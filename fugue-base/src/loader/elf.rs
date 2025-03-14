use std::borrow::Cow;

use object::elf::{FileHeader32, FileHeader64};
use object::read::elf::{self, ElfFile};
use object::{Endianness, FileKind, Object, ObjectSegment};

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

pub fn elf_segments<'a>(
    elf: &'a impl Object<'a>,
) -> impl Iterator<Item = LoadableSegment<'a>> + 'a {
    elf.segments().into_iter().filter_map(|segm| {
        if segm.size() == 0 {
            return None;
        }

        let address = Address::from(segm.address());
        let data = segm.data().unwrap_or_default();

        let bytes = if data.len() as u64 != segm.size() {
            // we have some partial or fully uninitialised segment?

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
            properties: LoadableSegmentProperties::all(),
            bytes,
        })
    })
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
            elf | Box::new(elf_segments(elf)) as Box<dyn Iterator<Item = LoadableSegment>>
        )
    }
}

#[cfg(test)]
mod test {
    use crate::types::BytesOrMapping;

    use super::Elf;

    #[test]
    #[ignore]
    fn test_elf() -> Result<(), Box<dyn std::error::Error>> {
        let _elf = Elf::new(BytesOrMapping::from_file("tests/ls.elf")?)?;
        Ok(())
    }
}
