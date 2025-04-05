use std::borrow::Cow;

use fallible_iterator::FallibleIterator;

use object::elf::{
    FileHeader32, FileHeader64, PF_R, PF_W, PF_X, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, STB_GLOBAL,
    STB_WEAK, STT_NOTYPE,
};
use object::read::elf::{
    self, ElfFile, ElfSection, ElfSectionIterator, ElfSegment, ElfSegmentIterator, FileHeader,
};
use object::{
    Architecture, Endianness, FileKind, Object, ObjectKind, ObjectSection, ObjectSegment,
    ObjectSymbol, ReadRef, Relocation, RelocationFlags, RelocationKind, SectionFlags, SegmentFlags,
    SymbolFlags,
};

use range_set_blaze::{IntoRangesIter, RangeSetBlaze};

use crate::lifter::{Language, Lifter};
use crate::loader::externs::ExternSymbols;
use crate::loader::object::object_lifter;
use crate::loader::{Loadable, LoadableSegment, LoadableSegmentProperties, LoaderError};
use crate::types::{Address, AttributeMap, BytesOrMapping};

pub mod relocations;
pub mod symbols;

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
    externs: ExternSymbols,
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
        let externs = with_elf!(view, elf | elf_externs(elf, &lifter));

        Ok(Self {
            object,
            lifter,
            externs,
            attributes: attributes.into(),
        })
    }
}

pub fn elf_externs<'a>(elf: &'a impl Object<'a>, lifter: &Lifter) -> ExternSymbols {
    // TODO:
    // - base address should be configurable.
    // - template should be obtained from the lifter based on the architecture.

    let is_object = elf.kind() == ObjectKind::Relocatable;
    let addr_size = lifter.address_size();

    let base = if is_object {
        elf.sections().fold(0, |acc, sect| {
            if sect.size() == 0 {
                return acc;
            }

            let SectionFlags::Elf { sh_flags } = sect.flags() else {
                return acc;
            };

            if (sh_flags as u32 & SHF_ALLOC) == SHF_ALLOC {
                let aligned_start =
                    (acc + sect.align().wrapping_sub(1)) & !sect.align().wrapping_sub(1);
                let aligned_end = aligned_start + sect.size();
                aligned_end
            } else {
                acc
            }
        })
    } else {
        elf.sections()
            .map(|sect| sect.address() + sect.size())
            .chain(elf.segments().map(|segm| segm.address() + segm.size()))
            .max()
            .unwrap_or(0)
            + addr_size as u64
    };

    let syms = if is_object {
        elf.symbols()
    } else {
        elf.dynamic_symbols()
    };

    let mut externs = ExternSymbols::new(base, &[0u8; 16][..]);
    let template_size = externs.template().len();

    for (index, addr, sym) in syms
        .enumerate()
        .filter(|(_, sym)| {
            let SymbolFlags::Elf { st_info, .. } = sym.flags() else {
                return false;
            };

            let st_bind = st_info >> 4;
            let st_type = st_info & 0x0f;

            let is_import = (st_bind == STB_GLOBAL || st_bind == STB_WEAK) && sym.address() == 0;

            (is_import && !is_object) || (is_object && is_import && st_type == STT_NOTYPE)
        })
        .enumerate()
        .map(|(idx, (oidx, sym))| (oidx, base + (idx * template_size) as u64, sym))
    {
        let sym = sym.name().ok().map(ustr::ustr);
        externs.add_symbol(index, addr, sym);
    }

    externs
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
    elf: &'file ElfFile<'data, Elf, R>,
    segms: ElfSegmentIterator<'data, 'file, Elf, R>,
    sects: ElfSectionIterator<'data, 'file, Elf, R>,
    // this represents the ranges already covered
    covered: RangeSetBlaze<u64>,
    // this is used to split segments that span multiple unmapped ranges
    segms_split: Option<(IntoRangesIter<u64>, ElfSegment<'data, 'file, Elf, R>)>,
    // this is used to track the current base address
    current_base: Address,
    // this represents the virtual segment containing external symbols
    externs: Option<&'file ExternSymbols>,
    // this is used to track if we're working with an object file or not
    is_object: bool,
}

impl<'data, 'file, Elf, R> ElfLoadableSegments<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
    'file: 'data,
{
    pub(crate) fn new(elf: &'file ElfFile<'data, Elf, R>, externs: &'file ExternSymbols) -> Self {
        Self {
            elf,
            sects: elf.sections(),
            segms: elf.segments(),
            covered: RangeSetBlaze::new(),
            segms_split: None,
            current_base: Address::zero(),
            externs: Some(externs),
            is_object: elf.kind() == ObjectKind::Relocatable,
        }
    }

    pub(crate) fn extern_segment(&mut self) -> Result<Option<LoadableSegment<'data>>, LoaderError> {
        let Some(externs) = self.externs.take().filter(|e| e.len() > 0) else {
            return Ok(None);
        };
        let extern_size = externs.size() as usize;

        let address = externs.base();
        let last_address = externs.base() + (extern_size as u64 - 1);

        let mut bytes = Vec::with_capacity(extern_size);

        for _ in 0..externs.len() {
            bytes.extend_from_slice(externs.template());
        }

        self.covered
            .ranges_insert(address.offset()..=last_address.offset());

        let lsegm = LoadableSegment {
            name: Cow::Borrowed("EXTERN"),
            address: externs.base(),
            properties: LoadableSegmentProperties::EXTERNAL
                | LoadableSegmentProperties::PERM_READ
                | LoadableSegmentProperties::PERM_EXECUTE,
            bytes: Cow::Owned(bytes),
        };

        Ok(Some(lsegm))
    }

    pub(crate) fn next_unlinked(&mut self) -> Result<Option<LoadableSegment<'data>>, LoaderError> {
        while let Some(sect) = self.sects.next() {
            let SectionFlags::Elf { sh_flags } = sect.flags() else {
                continue;
            };

            let size = sect.size();

            if size == 0 || (sh_flags as u32 & SHF_ALLOC) != SHF_ALLOC {
                continue;
            }

            let alignment_mask = sect.align().wrapping_sub(1);
            let address = Address::from(
                self.current_base.offset().wrapping_add(alignment_mask) & !alignment_mask,
            );
            let last_address = address + size - 1usize;

            if last_address < address {
                tracing::debug!("section bounds {address}-{last_address} overflow; skipping");
                continue;
            }

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

            self.current_base = last_address + 1usize;

            let bytes = if data.len() as u64 != sect.size() {
                let mut data = data.to_owned();
                data.resize(sect.size() as _, 0);

                Cow::Owned(data)
            } else {
                Cow::Borrowed(data)
            };

            let mut lsegm = LoadableSegment {
                name: sect
                    .name()
                    .ok()
                    .map_or_else(|| Cow::Borrowed("LOAD"), |name| Cow::Borrowed(name)),
                address,
                properties: elf_section_properties(&sect),
                bytes,
            };

            self.covered.ranges_insert(vrange);

            self.apply_relocations(&mut lsegm, &sect)?;
            self.apply_dynamic_relocations(&mut lsegm)?;

            return Ok(Some(lsegm));
        }

        self.extern_segment()
    }

    pub(crate) fn next_linked_split(
        &mut self,
    ) -> Result<Option<LoadableSegment<'data>>, LoaderError> {
        let Some((covered, segm)) = self.segms_split.as_mut() else {
            return Ok(None);
        };
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

            let mut lsegm = LoadableSegment {
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
            self.apply_dynamic_relocations(&mut lsegm)?;

            return Ok(Some(lsegm));
        }

        self.segms_split = None;

        Ok(None)
    }

    pub(crate) fn next_linked_section(
        &mut self,
    ) -> Result<Option<LoadableSegment<'data>>, LoaderError> {
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

            if last_address < address {
                tracing::debug!("section bounds {address}-{last_address} overflow; skipping");
                continue;
            }

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

            let bytes = if data.len() as u64 != sect.size() {
                let mut data = data.to_owned();
                data.resize(sect.size() as _, 0);

                Cow::Owned(data)
            } else {
                Cow::Borrowed(data)
            };

            let mut lsegm = LoadableSegment {
                name: sect
                    .name()
                    .ok()
                    .map_or_else(|| Cow::Borrowed("LOAD"), |name| Cow::Borrowed(name)),
                address,
                properties: elf_section_properties(&sect),
                bytes,
            };

            self.covered.ranges_insert(vrange);
            self.apply_relocations(&mut lsegm, &sect)?;
            self.apply_dynamic_relocations(&mut lsegm)?;

            return Ok(Some(lsegm));
        }

        Ok(None)
    }

    pub(crate) fn next_linked_segment(
        &mut self,
    ) -> Result<Option<LoadableSegment<'data>>, LoaderError> {
        while let Some(segm) = self.segms.next() {
            let size = segm.size();

            if segm.size() == 0 {
                continue;
            }

            let address = Address::from(segm.address());
            let last_address = Address::from(segm.address() + size - 1);

            if last_address < address {
                tracing::debug!("segment bounds {address}-{last_address} overflow; skipping");
                continue;
            }

            tracing::trace!("processing segment {address}-{last_address}");

            let data = segm.data().unwrap_or_default();

            let vrange = address.offset()..=last_address.offset();

            let covered = RangeSetBlaze::from_iter([vrange.clone()]) - &self.covered;

            if covered.is_empty() {
                tracing::trace!("segment range {address}-{last_address} already covered");
                continue;
            }

            let should_split = covered.ranges_len() > 1;

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

            tracing::trace!("loading segment {address}-{last_address}");

            let mut lsegm = LoadableSegment {
                name: segm
                    .name()
                    .ok()
                    .flatten()
                    .map_or_else(|| Cow::Borrowed("LOAD"), |name| Cow::Owned(name.to_owned())),
                address,
                properties: elf_segment_properties(&segm),
                bytes,
            };

            if should_split {
                self.segms_split = Some((ranges, segm));
            }

            self.covered.ranges_insert(range);
            self.apply_dynamic_relocations(&mut lsegm)?;

            return Ok(Some(lsegm));
        }

        Ok(None)
    }

    pub(crate) fn next_linked(&mut self) -> Result<Option<LoadableSegment<'data>>, LoaderError> {
        if let Some(v) = self.next_linked_split()? {
            return Ok(Some(v));
        }

        if let Some(v) = self.next_linked_section()? {
            return Ok(Some(v));
        }

        if let Some(v) = self.next_linked_segment()? {
            return Ok(Some(v));
        }

        self.extern_segment()
    }

    pub(crate) fn apply_relocations(
        &self,
        lsegm: &mut LoadableSegment<'data>,
        sect: &ElfSection<'data, 'file, Elf, R>,
    ) -> Result<(), LoaderError> {
        /*
        let relocations = self
            .elf
            .relocations()
            .filter(|rel| rel.address() >= lsegm.address().offset())
            .filter(|rel| rel.address() <= lsegm.last_address().offset());

        for reloc in relocations {
            let offset = reloc.address() - lsegm.address().offset();
            let size = reloc.size() as usize;

            if size > lsegm.bytes.len() {
                return Err(LoaderError::format_with(
                    "relocation size is larger than segment size",
                ));
            }

            let bytes = &mut lsegm.bytes[offset..offset + size];
            bytes.copy_from_slice(reloc.data());
        }
        */

        for (off, rel) in sect.relocations() {
            tracing::trace!(
                "applying relocation {}+{off:#x} {:?}",
                lsegm.address(),
                rel.kind()
            );
        }

        Ok(())
    }

    pub(crate) fn apply_dynamic_relocations(
        &self,
        lsegm: &mut LoadableSegment<'data>,
    ) -> Result<(), LoaderError> {
        let Some(drels) = self.elf.dynamic_relocations() else {
            return Ok(());
        };

        let offset = lsegm.address().offset();
        let last_offset = lsegm.last_address().offset();

        // TODO: add base address to dynamic relocations offset
        for (off, rel) in drels.filter(|(off, _)| *off >= offset && *off <= last_offset) {
            tracing::trace!("applying dynamic relocation at {}", Address::from(off));

            match rel.kind() {
                RelocationKind::Unknown => {
                    let RelocationFlags::Elf { r_type } = rel.flags() else {
                        // NOTE: we could probably panic here
                        continue;
                    };

                    match self.elf.architecture() {
                        Architecture::X86_64 => {
                            self.apply_x86_64_relocation(lsegm, off, &rel, r_type);
                        }
                        arch => {
                            tracing::warn!(
                                "unsupported architecture {arch:?} for relocation {:?}",
                                rel.kind()
                            );
                        }
                    }
                }
                _ => {
                    tracing::warn!("unsupported relocation kind {:?}", rel.kind());
                }
            }
        }

        Ok(())
    }

    pub(crate) fn apply_x86_64_relocation(
        &self,
        lsegm: &mut LoadableSegment<'data>,
        offset: u64,
        reloc: &Relocation,
        reloc_type: u32,
    ) {
        use object::elf::{R_X86_64_GLOB_DAT, R_X86_64_JUMP_SLOT, R_X86_64_RELATIVE};

        // TODO: allow configurable base address
        let base = 0u64;

        match reloc_type {
            R_X86_64_RELATIVE => {
                let offset = offset as usize;
                let value = base.wrapping_add_signed(reloc.addend());

                tracing::trace!("applying relocation {reloc_type:#x} at {offset:#x}: {value:#x}",);

                lsegm.write_value(offset, value);
            }
            R_X86_64_GLOB_DAT | R_X86_64_JUMP_SLOT => {
                let offset = offset as usize;

                tracing::trace!("applying relocation {reloc_type:#x} at {offset:#x}: <TODO>",);
            }
            _ => {
                tracing::warn!("unsupported relocation type {reloc:?}");
            }
        }
    }
}

impl<'data, 'file, Elf, R> FallibleIterator for ElfLoadableSegments<'data, 'file, Elf, R>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
    'file: 'data,
{
    type Item = LoadableSegment<'data>;
    type Error = LoaderError;

    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        if self.is_object {
            self.next_unlinked()
        } else {
            self.next_linked()
        }
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

    fn segments<'a>(
        &'a self,
    ) -> impl FallibleIterator<Item = LoadableSegment<'a>, Error = LoaderError> + 'a {
        let view = self.object.borrow_view();

        with_elf!(
            view,
            elf | Box::new(ElfLoadableSegments::new(elf, &self.externs))
                as Box<dyn FallibleIterator<Item = LoadableSegment, Error = LoaderError>>
        )
    }
}

#[cfg(test)]
mod test {
    use fallible_iterator::FallibleIterator;

    use crate::loader::Loadable;
    use crate::types::BytesOrMapping;

    use super::Elf;

    #[test]
    fn test_elf() -> Result<(), Box<dyn std::error::Error>> {
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
            .with_line_number(true)
            .with_file(true)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let elf = Elf::new(BytesOrMapping::from_file("tests/ls.elf")?)?;
            let mut segments = elf.segments();
            while let Some(segm) = segments.next()? {
                tracing::info!(
                    "{}-{} ({:?})",
                    segm.address(),
                    segm.last_address(),
                    segm.name()
                );
            }
            Ok(())
        })
    }

    #[test]
    #[ignore]
    fn test_elf_rel() -> Result<(), Box<dyn std::error::Error>> {
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
            .with_line_number(true)
            .with_file(true)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let elf = Elf::new(BytesOrMapping::from_file("tests/liblzma_la-crc64-fast.o")?)?;
            let mut segments = elf.segments();
            while let Some(segm) = segments.next()? {
                tracing::info!(
                    "{}-{} ({:?})",
                    segm.address(),
                    segm.address() + segm.len(),
                    segm.name()
                );
            }
            Ok(())
        })
    }
}
