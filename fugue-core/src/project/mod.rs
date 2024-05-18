use std::borrow::Cow;
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
    fn view_bytes(&self, address: impl Into<Address>) -> Result<&[u8], ProjectRawViewError>;
}

pub struct ProjectRawViewMmaped {
    backing: MmapTable<U64<LE>>,
    segments: Vec<LoadedSegment<'static>>,
}

pub struct ProjectRawViewMmapedReader<'a> {
    backing: MmapTableReader<'a, U64<LE>>,
    segments: &'a [LoadedSegment<'static>],
}

pub struct LoadedSegment<'a> {
    name: Cow<'a, str>,
    addr: Address,
    size: usize,
    data: Cow<'a, [u8]>,
}

impl<'a> LoadedSegment<'a> {
    pub fn new(name: impl Into<Cow<'a, str>>, addr: impl Into<Address>, data: impl Into<Cow<'a, [u8]>>) -> Self {
        let data = data.into();
        let size = data.len();

        Self {
            name: name.into(),
            addr: addr.into(),
            size,
            data,
        }
    }

    pub fn new_uninit(name: impl Into<Cow<'a, str>>, addr: impl Into<Address>, size: usize) -> Self {
        Self {
            name: name.into(),
            addr: addr.into(),
            size,
            data: Cow::Borrowed(&[]),
        }
    }

    pub fn borrowed(&self) -> LoadedSegment {
        LoadedSegment {
            name: Cow::Borrowed(self.name()),
            addr: self.addr,
            size: self.size,
            data: Cow::Borrowed(self.data()),
        }
    }

    pub fn address(&self) -> Address {
        self.addr
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_ref()
    }

    pub fn range(&self) -> Range<Address> {
        self.addr..self.addr+self.size
    }
}

impl<'a> ProjectRawViewReader<'a> for ProjectRawViewMmapedReader<'a> {
    fn view_bytes(&self, address: impl Into<Address>) -> Result<&[u8], ProjectRawViewError> {
        let address = address.into();
        // find the interval that contains this address
        let Ok(index) = self.segments.binary_search_by(|segm| {
            let iv = segm.range();
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

        let segm = &self.segments[index].range();
        let data = self
            .backing
            .get(segm.start)
            .map_err(ProjectRawViewError::backing)?
            .ok_or_else(|| {
                ProjectRawViewError::read_with(address, "interval not present in backing")
            })?;

        let offset = usize::from(address - segm.start);

        Ok(&data[offset..])
    }
}

impl ProjectRawView for ProjectRawViewMmaped {
    type Reader<'a> = ProjectRawViewMmapedReader<'a>;

    fn new<'a, L>(loadable: &L) -> Result<Self, ProjectRawViewError>
    where
        L: Loadable<'a>,
    {
        let mut segments = Vec::new();
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

            tx.set(addr, data.as_ref())
                .map_err(ProjectRawViewError::backing)?;

            segments.push(LoadedSegment::new_uninit("LOAD", addr, size));

            Ok(())
        })?;

        // Ensure that the ranges are sorted and non-overlapping
        segments.sort_by_key(|r| r.address());

        for i in 1..segments.len() {
            let ri = segments[i].range();
            let rj = segments[i-1].range();

            if ri.start < rj.end {
                return Err(ProjectRawViewError::OverlappingRanges);
            }
        }

        // Finally commit the mapping to the backing store
        tx.commit().map_err(ProjectRawViewError::backing)?;

        Ok(Self { backing, segments })
    }

    fn reader<'a>(&'a self) -> Result<Self::Reader<'a>, ProjectRawViewError> {
        Ok(ProjectRawViewMmapedReader {
            backing: self
                .backing
                .reader()
                .map_err(ProjectRawViewError::backing)?,
            segments: &self.segments,
        })
    }
}

pub struct ProjectRawViewInMemory {
    backing: IntervalMap<Address, LoadedSegment<'static>>,
}

pub struct ProjectRawViewInMemoryReader<'a> {
    backing: &'a IntervalMap<Address, LoadedSegment<'static>>,
}

impl<'a> ProjectRawViewReader<'a> for ProjectRawViewInMemoryReader<'a> {
    fn view_bytes(&self, address: impl Into<Address>) -> Result<&[u8], ProjectRawViewError> {
        let address = address.into();
        self.backing
            .overlap(address)
            .next()
            .map(|(range, segm)| {
                let offset = usize::from(address - range.start);
                &segm.data()[offset..]
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

                if mapping.has_overlap(addr..last_addr) {
                    return Err(ProjectRawViewError::OverlappingRanges);
                }

                mapping.insert(addr..last_addr, LoadedSegment::new("LOAD", addr, data.into_owned()));

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
    #[ignore]
    fn test_project() -> Result<(), Box<dyn std::error::Error>> {
        // Load the binary at tests/ls.elf into a mapping object
        let input = BytesOrMapping::from_file("tests/ls.elf")?;
        let object = Object::new(input)?;

        // Create the project from the mapping object
        let project1 = Project::<ProjectRawViewMmaped>::new(&object)?;
        let project2 = Project::<ProjectRawViewInMemory>::new(&object)?;

        // Let's test a read from a known address...
        let reader1 = project1.raw().reader()?;
        let segment1 = reader1.view_bytes(0x4060u32)?;

        assert!(segment1.len() > 4);
        assert!(&segment1[..4] == b"\xf3\x0f\x1e\xfa");

        let reader2 = project2.raw().reader()?;
        let segment2 = reader2.view_bytes(0x4060u32)?;

        assert!(segment2.len() > 4);
        assert_eq!(&segment2[..4], &segment1[..4]);

        Ok(())
    }
}
