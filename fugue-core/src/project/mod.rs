use std::cmp::Ordering;
use std::fmt::{Debug, Display};
use std::ops::Range;

use anyhow::anyhow;
use fugue_bytes::LE;
use fugue_ir::Address;
use heed::types::U64;
use iset::IntervalMap;
use thiserror::Error;

use crate::loader::Loadable;
use crate::util::table::{MmapTable, MmapTableReader};

pub trait ProjectRawView: Sized {
    type Reader<'a>: ProjectRawViewReader<'a>
    where
        Self: 'a;

    fn new<'a, L>(loadable: &L) -> Result<Self, ProjectRawViewError>
    where
        L: Loadable<'a>;

    fn reader<'a>(&'a self) -> Result<Self::Reader<'a>, ProjectRawViewError>;
}

pub trait ProjectRawViewReader<'a> {
    fn view_bytes(&self, address: Address) -> Result<&[u8], ProjectRawViewError>;
}

pub struct ProjectRawViewMmaped {
    backing: MmapTable<U64<LE>>,
    ranges: Vec<Range<Address>>,
}

pub struct ProjectRawViewMmapedReader<'a> {
    backing: MmapTableReader<'a, U64<LE>>,
    ranges: &'a [Range<Address>],
}

impl<'a> ProjectRawViewReader<'a> for ProjectRawViewMmapedReader<'a> {
    fn view_bytes(&self, address: Address) -> Result<&[u8], ProjectRawViewError> {
        // find the interval that contains this address
        let Ok(index) = self.ranges.binary_search_by(|iv| {
            if address < iv.start {
                Ordering::Greater
            } else if address >= iv.end {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        }) else {
            return Err(ProjectRawViewError::read_with(
                address,
                "address not mapped",
            ));
        };

        let range = &self.ranges[index];

        let data = self
            .backing
            .get(range.start)
            .map_err(ProjectRawViewError::backing)?
            .ok_or_else(|| {
                ProjectRawViewError::read_with(address, "interval not present in backing")
            })?;

        let offset = usize::from(address - range.start);

        Ok(&data[offset..])
    }
}

impl ProjectRawView for ProjectRawViewMmaped {
    type Reader<'a> = ProjectRawViewMmapedReader<'a>;

    fn new<'a, L>(loadable: &L) -> Result<Self, ProjectRawViewError>
    where
        L: Loadable<'a>,
    {
        let mut ranges = Vec::new();
        let mut backing =
            MmapTable::temporary("binary-view").map_err(ProjectRawViewError::backing)?;
        let mut tx = backing.writer().map_err(ProjectRawViewError::backing)?;

        // Load the segments into the backing store (and keep track of the ranges)
        //
        // NOTE: we assume that the segments are non-overlapping, and verify this is the case
        // prior to committing the backing store.
        //
        loadable.segments().try_for_each(|segm| {
            let (addr, data) = segm.into_parts();
            let size = data.len();

            println!("mapping: {} -> {}", addr, size);

            // TODO: can we avoid owning the data--it seems needless for what we want to do with
            // it here?
            //
            tx.set(addr, data.as_ref())
                .map_err(ProjectRawViewError::backing)?;

            ranges.push(addr..addr + size);

            Ok(())
        })?;

        // Ensure that the ranges are sorted and non-overlapping
        ranges.sort_by_key(|r| r.start);

        for i in 1..ranges.len() {
            if ranges[i].start < ranges[i - 1].end {
                return Err(ProjectRawViewError::OverlappingRanges);
            }
        }

        // Finally commit the mapping to the backing store
        tx.commit().map_err(ProjectRawViewError::backing)?;

        Ok(Self { backing, ranges })
    }

    fn reader<'a>(&'a self) -> Result<Self::Reader<'a>, ProjectRawViewError> {
        Ok(ProjectRawViewMmapedReader {
            backing: self
                .backing
                .reader()
                .map_err(ProjectRawViewError::backing)?,
            ranges: &self.ranges,
        })
    }
}

pub struct ProjectRawViewInMemory {
    backing: IntervalMap<Address, Vec<u8>>,
}

pub struct ProjectRawViewInMemoryReader<'a> {
    backing: &'a IntervalMap<Address, Vec<u8>>,
}

impl<'a> ProjectRawViewReader<'a> for ProjectRawViewInMemoryReader<'a> {
    fn view_bytes(&self, address: Address) -> Result<&[u8], ProjectRawViewError> {
        self.backing
            .overlap(address)
            .next()
            .map(|(range, data)| {
                let offset = usize::from(address - range.start);
                &data[offset..]
            })
            .ok_or_else(|| ProjectRawViewError::read_with(address, "address not mapped"))
    }
}

impl ProjectRawView for ProjectRawViewInMemory {
    type Reader<'a> = ProjectRawViewInMemoryReader<'a>;

    fn new<'a, L>(loadable: &L) -> Result<Self, ProjectRawViewError>
    where
        L: Loadable<'a>,
    {
        let backing = loadable
            .segments()
            .try_fold(IntervalMap::new(), |mut mapping, segm| {
                let (addr, data) = segm.into_parts();
                let size = data.len();
                let last_addr = addr + size;

                println!("mapping: {} -> {}", addr, size);

                if mapping.has_overlap(addr..last_addr) {
                    return Err(ProjectRawViewError::OverlappingRanges);
                }

                mapping.insert(addr..last_addr, data.into_owned());

                Ok(mapping)
            })?;

        Ok(Self { backing })
    }

    fn reader<'a>(&'a self) -> Result<Self::Reader<'a>, ProjectRawViewError> {
        Ok(ProjectRawViewInMemoryReader {
            backing: &self.backing,
        })
    }
}

#[derive(Debug, Error)]
pub enum ProjectRawViewError {
    #[error("cannot construct memory mapping: {0}")]
    Backing(anyhow::Error),
    #[error("cannot create a memory mapping with overlapping ranges")]
    OverlappingRanges,
    #[error("cannot view bytes at {0}: {1}")]
    Read(Address, anyhow::Error),
}

impl ProjectRawViewError {
    pub fn backing<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Backing(e.into())
    }

    pub fn backing_with<M>(m: M) -> Self
    where
        M: Debug + Display + Send + Sync + 'static,
    {
        Self::Backing(anyhow!(m))
    }

    pub fn read<E>(address: impl Into<Address>, e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Read(address.into(), e.into())
    }

    pub fn read_with<M>(address: impl Into<Address>, m: M) -> Self
    where
        M: Debug + Display + Send + Sync + 'static,
    {
        Self::Read(address.into(), anyhow!(m))
    }
}

pub struct Project<R>
where
    R: ProjectRawView,
{
    mapping: R,
}

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("cannot load project: {0}")]
    Load(anyhow::Error),
}

impl ProjectError {
    pub fn load<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Load(e.into())
    }
}

impl<R> Project<R>
where
    R: ProjectRawView,
{
    pub fn new<'a, L>(loadable: &L) -> Result<Self, ProjectError>
    where
        L: Loadable<'a>,
    {
        Ok(Self {
            mapping: R::new(loadable).map_err(ProjectError::load)?,
        })
    }

    pub fn raw(&self) -> &R {
        &self.mapping
    }
}

#[cfg(test)]
mod test {
    use crate::loader::Object;
    use crate::util::BytesOrMapping;

    use super::*;

    #[test]
    fn test_project() -> Result<(), Box<dyn std::error::Error>> {
        // Load the binary at tests/ls.elf into a mapping object
        let input = BytesOrMapping::from_file("tests/ls.elf")?;

        // Create the project from the mapping object
        let _prj = Project::<ProjectRawViewMmaped>::new(&Object::new(input)?)?;

        Ok(())
    }
}
