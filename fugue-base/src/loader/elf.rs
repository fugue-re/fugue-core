use std::borrow::Cow;

use object::elf::{
    FileHeader32, FileHeader64, PF_R, PF_W, PF_X, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE,
};
use object::read::elf::{
    self, ElfFile, ElfSectionIterator, ElfSegment, ElfSegmentIterator, FileHeader,
};
use object::{
    Endianness, FileKind, Object, ObjectSection, ObjectSegment, ReadRef, SectionFlags, SegmentFlags,
};
use range_set_blaze::{IntoRangesIter, RangeSetBlaze};

use crate::lifter::{Language, Lifter};
use crate::loader::object::object_lifter;
use crate::loader::{Loadable, LoadableSegment, LoadableSegmentProperties, LoaderError};
use crate::types::{Address, AttributeMap, BytesOrMapping};

#[ouroboros::self_referencing]
struct ElfInner<'a> {
    data: BytesOrMapping<'a>,
    #[borrows(data)]
    #[covariant]
    view: ElfFileRepr<'this, 'a>,
}

enum ElfFileRepr<'this, 'data> {
    Elf32(ElfFile<'this, FileHeader32<Endianness>, &'this BytesOrMapping<'data>>),
    Elf64(ElfFile<'this, FileHeader64<Endianness>, &'this BytesOrMapping<'data>>),
}

macro_rules! with_elf {
    ($inner:expr, $var:ident | $body:expr) => {
        match $inner {
            ElfFileRepr::Elf32(ref $var) => $body,
            ElfFileRepr::Elf64(ref $var) => $body,
        }
    };
}

impl<'this, 'data> ElfFileRepr<'this, 'data> {
    fn parse(data: &'this BytesOrMapping<'data>) -> Result<Self, LoaderError> {
        let elf = match FileKind::parse(data).map_err(LoaderError::format)? {
            FileKind::Elf32 => {
                Self::Elf32(elf::ElfFile32::parse(data).map_err(LoaderError::format)?)
            }
            FileKind::Elf64 => {
                Self::Elf64(elf::ElfFile64::parse(data).map_err(LoaderError::format)?)
            }
            _ => {
                return Err(LoaderError::format_with("input is not an ELF"));
            }
        };
        Ok(elf)
    }
}

pub struct Elf<'a> {
    object: ElfInner<'a>,
    lifter: Lifter,
    attributes: AttributeMap,
}

impl<'a> Elf<'a> {
    pub fn new(data: impl Into<BytesOrMapping<'a>>) -> Result<Self, LoaderError> {
        Self::new_with(data, AttributeMap::new())
    }

    pub fn new_with(
        data: impl Into<BytesOrMapping<'a>>,
        attributes: impl Into<AttributeMap>,
    ) -> Result<Self, LoaderError> {
        let object = ElfInner::try_new(data.into(), |data| ElfFileRepr::parse(data))?;

        let view = object.borrow_view();
        let lifter = with_elf!(view, elf | object_lifter(elf))?;

        Ok(Self {
            object,
            lifter,
            attributes: attributes.into(),
        })
    }
}

pub fn elf_section_properties<'a>(sect: &impl ObjectSection<'a>) -> LoadableSegmentProperties {
    let SectionFlags::Elf { sh_flags } = sect.flags() else {
        // NOTE: we could probably panic here
        return LoadableSegmentProperties::empty();
    };

    let sh_flags = sh_flags as u32;

    let mut props = LoadableSegmentProperties::PERM_READ;

    if sh_flags & SHF_WRITE == SHF_WRITE {
        props.insert(LoadableSegmentProperties::PERM_WRITE);
    }

    if sh_flags & SHF_EXECINSTR == SHF_EXECINSTR {
        props.insert(LoadableSegmentProperties::PERM_EXECUTE);
    }

    if matches!(sect.file_range(), None | Some((_, 0))) {
        props.insert(LoadableSegmentProperties::UNINITIALISED);
    }

    props
}

pub fn elf_segment_properties<'a>(segm: &impl ObjectSegment<'a>) -> LoadableSegmentProperties {
    let SegmentFlags::Elf { p_flags } = segm.flags() else {
        // NOTE: we could probably panic here
        return LoadableSegmentProperties::empty();
    };

    let mut props = LoadableSegmentProperties::empty();

    if p_flags & PF_R == PF_R {
        props.insert(LoadableSegmentProperties::PERM_READ);
    }

    if p_flags & PF_W == PF_W {
        props.insert(LoadableSegmentProperties::PERM_WRITE);
    }

    if p_flags & PF_X == PF_X {
        props.insert(LoadableSegmentProperties::PERM_EXECUTE);
    }

    if segm.file_range().1 == 0 {
        props.insert(LoadableSegmentProperties::UNINITIALISED);
    }

    props
}

pub fn elf_section<'a>(sect: &impl ObjectSection<'a>) -> Option<LoadableSegment<'a>> {
    let SectionFlags::Elf { sh_flags } = sect.flags() else {
        return None;
    };

    if sect.size() == 0 || (sh_flags as u32 & SHF_ALLOC) != SHF_ALLOC {
        return None;
    }

    let address = Address::from(sect.address());
    let data = sect.data().unwrap_or_default();

    let bytes = if data.len() as u64 != sect.size() {
        let mut data = data.to_owned();
        data.resize(sect.size() as _, 0);

        Cow::Owned(data)
    } else {
        Cow::Borrowed(data)
    };

    Some(LoadableSegment {
        name: sect
            .name()
            .ok()
            .map_or_else(|| Cow::Borrowed("LOAD"), |name| Cow::Borrowed(name)),
        address,
        properties: elf_section_properties(sect),
        bytes,
    })
}

pub fn elf_segment<'a>(segm: &impl ObjectSegment<'a>) -> Option<LoadableSegment<'a>> {
    if segm.size() == 0 {
        return None;
    }

    let address = Address::from(segm.address());
    let data = segm.data().unwrap_or_default();

    let bytes = if data.len() as u64 != segm.size() {
        let mut data = data.to_owned();
        data.resize(segm.size() as _, 0);

        Cow::Owned(data)
    } else {
        Cow::Borrowed(data)
    };

    Some(LoadableSegment {
        name: segm
            .name()
            .ok()
            .flatten()
            .map_or_else(|| Cow::Borrowed("LOAD"), |name| Cow::Owned(name.to_owned())),
        address,
        properties: elf_segment_properties(segm),
        bytes,
    })
}

pub fn elf_sections<'a>(
    elf: &'a impl Object<'a>,
) -> impl Iterator<Item = LoadableSegment<'a>> + 'a {
    elf.sections()
        .into_iter()
        .filter_map(|sect| elf_section(&sect))
}

pub fn elf_segments<'a>(
    elf: &'a impl Object<'a>,
) -> impl Iterator<Item = LoadableSegment<'a>> + 'a {
    elf.segments()
        .into_iter()
        .filter_map(|segm| elf_segment(&segm))
}

pub(crate) struct ElfLoadableSegments<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
    'file: 'data,
{
    segms: ElfSegmentIterator<'data, 'file, Elf, R>,
    sects: ElfSectionIterator<'data, 'file, Elf, R>,
    covered: RangeSetBlaze<u64>,
    segms_split: Option<(IntoRangesIter<u64>, ElfSegment<'data, 'file, Elf, R>)>,
}

impl<'data, 'file, Elf, R> ElfLoadableSegments<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
    'file: 'data,
{
    pub(crate) fn new(elf: &'file ElfFile<'data, Elf, R>) -> Self {
        Self {
            sects: elf.sections(),
            segms: elf.segments(),
            covered: RangeSetBlaze::new(),
            segms_split: None,
        }
    }

    pub(crate) fn next_linked_split(&mut self) -> Option<LoadableSegment<'data>> {
        let (covered, segm) = self.segms_split.as_mut()?;
        while let Some(range) = covered.next() {
            let data = segm.data().unwrap_or_default();

            let rvsize = (*range.end() - *range.start() + 1) as usize;
            let rvstart = (*range.start() - segm.address()) as usize;
            let rvend = rvsize + rvstart;

            let bytes = if data.len() < rvend {
                let mut bytes = Vec::with_capacity(rvsize);

                if rvstart < data.len() {
                    bytes.extend_from_slice(&data[rvstart..]);
                }

                bytes.resize(rvsize, 0u8);

                Cow::Owned(bytes)
            } else {
                Cow::Borrowed(&data[rvstart..rvend])
            };

            let address = Address::from(*range.start());
            let last_address = address + bytes.len();

            tracing::trace!("loading segment {address}-{last_address}");

            // TODO: relocations!

            let lsegm = LoadableSegment {
                name: segm
                    .name()
                    .ok()
                    .flatten()
                    .map_or_else(|| Cow::Borrowed("LOAD"), |name| Cow::Owned(name.to_owned())),
                address,
                properties: elf_segment_properties(&*segm),
                bytes,
            };

            self.covered.ranges_insert(range);

            return Some(lsegm);
        }

        self.segms_split = None;
        None
    }

    pub(crate) fn next_linked_section(&mut self) -> Option<LoadableSegment<'data>> {
        while let Some(sect) = self.sects.next() {
            let SectionFlags::Elf { sh_flags } = sect.flags() else {
                continue;
            };

            let size = sect.size();

            if size == 0 || (sh_flags as u32 & SHF_ALLOC) != SHF_ALLOC {
                continue;
            }

            let address = Address::from(sect.address());
            let last_address = Address::from(sect.address() + size - 1);

            tracing::trace!("processing section {address}-{last_address}");

            let data = sect.data().unwrap_or_default();

            let vrange = address.offset()..=last_address.offset();
            if !self
                .covered
                .is_disjoint(&RangeSetBlaze::from_iter([vrange.clone()]))
            {
                tracing::debug!("overlapping section {address}-{last_address}; skipping");
                continue;
            }

            tracing::trace!("loading section {address}-{last_address}");

            self.covered.ranges_insert(vrange);

            let bytes = if data.len() as u64 != sect.size() {
                let mut data = data.to_owned();
                data.resize(sect.size() as _, 0);

                Cow::Owned(data)
            } else {
                Cow::Borrowed(data)
            };

            // TODO: relocations!

            return Some(LoadableSegment {
                name: sect
                    .name()
                    .ok()
                    .map_or_else(|| Cow::Borrowed("LOAD"), |name| Cow::Borrowed(name)),
                address,
                properties: elf_section_properties(&sect),
                bytes,
            });
        }

        None
    }

    pub(crate) fn next_linked_segment(&mut self) -> Option<LoadableSegment<'data>> {
        while let Some(segm) = self.segms.next() {
            let size = segm.size();

            if segm.size() == 0 {
                continue;
            }

            let address = Address::from(segm.address());
            let last_address = Address::from(segm.address() + size - 1);

            tracing::trace!("processing segment {address}-{last_address}");

            let data = segm.data().unwrap_or_default();

            let vrange = address.offset()..=last_address.offset();

            let covered = RangeSetBlaze::from_iter([vrange.clone()]) - &self.covered;

            if covered.is_empty() {
                tracing::trace!("segment range {address}-{last_address} already covered");
                continue;
            }

            let should_split = covered.len() > 1;

            if should_split {
                tracing::trace!(
                    "segment range {address}-{last_address} spans multiple unmapped ranges; splitting"
                );
            }

            let mut ranges = covered.into_ranges();
            let range = ranges.next().expect("not empty");

            let rvsize = (*range.end() - *range.start() + 1) as usize;
            let rvstart = (*range.start() - *vrange.start()) as usize;
            let rvend = rvsize + rvstart;

            let bytes = if data.len() < rvend {
                let mut bytes = Vec::with_capacity(rvsize);

                if rvstart < data.len() {
                    bytes.extend_from_slice(&data[rvstart..]);
                }

                bytes.resize(rvsize, 0u8);

                Cow::Owned(bytes)
            } else {
                Cow::Borrowed(&data[rvstart..rvend])
            };

            let address = Address::from(*range.start());
            let last_address = address + bytes.len();

            // TODO: relocations!

            tracing::trace!("loading segment {address}-{last_address}");

            let lsegm = LoadableSegment {
                name: segm
                    .name()
                    .ok()
                    .flatten()
                    .map_or_else(|| Cow::Borrowed("LOAD"), |name| Cow::Owned(name.to_owned())),
                address,
                properties: elf_segment_properties(&segm),
                bytes,
            };

            self.covered.ranges_insert(range);

            if should_split {
                self.segms_split = Some((ranges, segm));
            }

            return Some(lsegm);
        }

        None
    }

    pub(crate) fn next_linked(&mut self) -> Option<LoadableSegment<'data>> {
        self.next_linked_split()
            .or_else(|| self.next_linked_section())
            .or_else(|| self.next_linked_segment())
    }
}

impl<'data, 'file, Elf, R> Iterator for ElfLoadableSegments<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
    'file: 'data,
{
    type Item = LoadableSegment<'data>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_linked()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let sects_bound = self.sects.size_hint().0;
        let segms_bound = self.segms.size_hint().0;
        let splits = self
            .segms_split
            .as_ref()
            .map(|(it, _)| it.size_hint().0)
            .unwrap_or(0);
        (sects_bound + segms_bound + splits, None)
    }
}

impl Loadable for Elf<'_> {
    fn entry_address(&self) -> Option<Address> {
        Some(with_elf!(
            self.object.borrow_view(),
            elf | elf.entry().into()
        ))
    }

    fn attributes(&self) -> &AttributeMap {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut AttributeMap {
        &mut self.attributes
    }

    fn language(&self) -> &'static Language {
        self.lifter.language()
    }

    fn lifter(&self) -> Lifter {
        self.lifter.clone()
    }

    fn segments<'a>(&'a self) -> impl Iterator<Item = LoadableSegment<'a>> + 'a {
        let view = self.object.borrow_view();

        with_elf!(
            view,
            elf | Box::new(ElfLoadableSegments::new(elf))
                as Box<dyn Iterator<Item = LoadableSegment>>
        )
    }
}

#[cfg(test)]
mod test {
    use crate::loader::Loadable;
    use crate::types::BytesOrMapping;

    use super::Elf;

    #[test]
    fn test_elf() -> Result<(), Box<dyn std::error::Error>> {
        let elf = Elf::new(BytesOrMapping::from_file("tests/ls.elf")?)?;
        for segm in elf.segments() {
            println!("{}-{}", segm.address(), segm.last_address());
        }
        Ok(())
    }
}
