use std::borrow::Cow;
use std::fmt::{Debug, Display};

use bitflags::bitflags;

use fallible_iterator::FallibleIterator;

use fugue_bytes::traits::ByteCast;
use fugue_bytes::{BE, LE};

use thiserror::Error;

use crate::arch::Arch;
use crate::lifter::{Language, Lifter, LifterBuilderError};
use crate::types::{Address, AttributeMap, BytesOrMapping};

pub mod elf;
pub use elf::Elf;

// pub mod macho

pub mod object;

// pub mod pe;

pub mod shellcode;
pub use shellcode::Shellcode;

pub mod symbols;
pub use symbols::{ExternSymbols, LocalSymbols};

#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("cannot load object: {0}")]
    Format(anyhow::Error),
    #[error("cannot read object: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Lifter(#[from] LifterBuilderError),
    #[error("cannot load object; unsupported architecture")]
    UnsupportedArch,
}

impl LoaderError {
    pub fn format<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Format(e.into())
    }

    pub fn format_with<M>(m: M) -> Self
    where
        M: Debug + Display + Send + Sync + 'static,
    {
        Self::Format(anyhow::Error::msg(m))
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct LoadableSegmentProperties: u8 {
        const PERM_READ     = 0b0000_0001;
        const PERM_WRITE    = 0b0000_0010;
        const PERM_EXECUTE  = 0b0000_0100;

        const PERM_ALL      = Self::PERM_READ.bits() | Self::PERM_WRITE.bits() | Self::PERM_EXECUTE.bits();

        const UNINITIALISED = 0b0001_0000;
        const LITTLE_ENDIAN = 0b0010_0000;

        const EXTERNAL      = 0b0100_0000;
    }
}

impl LoadableSegmentProperties {
    pub fn is_readable(&self) -> bool {
        self.contains(Self::PERM_READ)
    }

    pub fn is_writable(&self) -> bool {
        self.contains(Self::PERM_WRITE)
    }

    pub fn is_executable(&self) -> bool {
        self.contains(Self::PERM_EXECUTE)
    }

    pub fn is_initialised(&self) -> bool {
        !self.is_uninitialised()
    }

    pub fn is_uninitialised(&self) -> bool {
        self.contains(Self::UNINITIALISED)
    }

    pub fn is_little_endian(&self) -> bool {
        self.contains(Self::LITTLE_ENDIAN)
    }

    pub fn is_external(&self) -> bool {
        self.contains(Self::EXTERNAL)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoadableSegment<'a> {
    name: Cow<'a, str>,
    address: Address,
    properties: LoadableSegmentProperties,
    bytes: Cow<'a, [u8]>,
}

impl LoadableSegment<'_> {
    pub fn address(&self) -> Address {
        self.address
    }

    pub fn last_address(&self) -> Address {
        self.address + self.bytes.len() - 1usize
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn properties(&self) -> LoadableSegmentProperties {
        self.properties
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn read_value<T: ByteCast>(&self, offset: usize) -> Option<T> {
        let range = self.view_bytes(offset, T::SIZEOF)?;
        Some(if self.properties.is_little_endian() {
            T::from_bytes::<LE>(range)
        } else {
            T::from_bytes::<BE>(range)
        })
    }

    pub fn update_value<T: ByteCast>(
        &mut self,
        offset: usize,
        f: impl FnOnce(T) -> T,
    ) -> Option<()> {
        let is_le = self.properties.is_little_endian();
        let range = self.view_bytes_mut(offset, T::SIZEOF)?;
        Some(if is_le {
            f(T::from_bytes::<LE>(range)).into_bytes::<LE>(range)
        } else {
            f(T::from_bytes::<BE>(range)).into_bytes::<BE>(range)
        })
    }

    pub fn write_value<T: ByteCast>(&mut self, offset: usize, value: T) -> Option<()> {
        let is_le = self.properties.is_little_endian();
        let range = self.view_bytes_mut(offset, T::SIZEOF)?;
        Some(if is_le {
            value.into_bytes::<LE>(range)
        } else {
            value.into_bytes::<BE>(range)
        })
    }

    pub fn view_bytes(&self, offset: usize, count: usize) -> Option<&[u8]> {
        let len = self.bytes.len();
        if offset >= len {
            return None;
        }

        if let Some(last_offset) = offset.checked_add(count) {
            if last_offset > len {
                None
            } else {
                Some(&self.bytes[offset..last_offset])
            }
        } else {
            None
        }
    }

    pub fn view_bytes_mut(&mut self, offset: usize, count: usize) -> Option<&mut [u8]> {
        let len = self.bytes.len();
        if offset >= len {
            return None;
        }

        if let Some(last_offset) = offset.checked_add(count) {
            if last_offset > len {
                None
            } else {
                Some(&mut self.bytes.to_mut()[offset..last_offset])
            }
        } else {
            None
        }
    }
}

pub trait Loadable {
    fn attributes(&self) -> &AttributeMap;

    fn attributes_mut(&mut self) -> &mut AttributeMap;

    fn entry_address(&self) -> Option<Address>;

    fn architecture(&self) -> Arch {
        Arch::new(self.language())
    }

    fn language(&self) -> &'static Language;

    fn lifter(&self) -> Lifter;

    fn local_symbols(&self) -> Option<&LocalSymbols> {
        None
    }

    fn extern_symbols(&self) -> Option<&ExternSymbols> {
        None
    }

    fn segments<'a>(
        &'a self,
    ) -> impl FallibleIterator<Item = LoadableSegment<'a>, Error = LoaderError> + 'a;

    fn segment_range(&self) -> (Address, Address);
}

pub enum Loader<'a> {
    Elf(elf::Elf<'a>),
    Object(object::Object<'a>),
}

impl<'a> Loader<'a> {
    pub fn new(data: impl Into<BytesOrMapping<'a>>) -> Result<Self, LoaderError> {
        Self::new_with(data, AttributeMap::new())
    }

    pub fn new_with(
        data: impl Into<BytesOrMapping<'a>>,
        attributes: impl Into<AttributeMap>,
    ) -> Result<Self, LoaderError> {
        use ::object::FileKind;

        let data = data.into();
        let loaded = match FileKind::parse(data.as_ref()).map_err(LoaderError::format)? {
            FileKind::Elf32 | FileKind::Elf64 => {
                let elf = Elf::new_with(data, attributes)?;
                Self::Elf(elf)
            }
            _ => {
                let object = object::Object::new_with(data, attributes)?;
                Self::Object(object)
            }
        };
        Ok(loaded)
    }

    pub fn from_file_with(
        path: impl AsRef<std::path::Path>,
        attributes: impl Into<AttributeMap>,
    ) -> Result<Self, LoaderError> {
        let data = BytesOrMapping::from_file(path)?;
        Self::new_with(data, attributes)
    }

    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, LoaderError> {
        Self::from_file_with(path, AttributeMap::new())
    }
}

impl Loadable for Loader<'_> {
    fn entry_address(&self) -> Option<Address> {
        match self {
            Self::Elf(elf) => elf.entry_address(),
            Self::Object(object) => object.entry_address(),
        }
    }

    fn architecture(&self) -> Arch {
        match self {
            Self::Elf(elf) => elf.architecture(),
            Self::Object(object) => object.architecture(),
        }
    }

    fn language(&self) -> &'static Language {
        match self {
            Self::Elf(elf) => elf.language(),
            Self::Object(object) => object.language(),
        }
    }

    fn lifter(&self) -> Lifter {
        match self {
            Self::Elf(elf) => elf.lifter(),
            Self::Object(object) => object.lifter(),
        }
    }

    fn local_symbols(&self) -> Option<&LocalSymbols> {
        match self {
            Self::Elf(elf) => elf.local_symbols(),
            Self::Object(object) => object.local_symbols(),
        }
    }

    fn extern_symbols(&self) -> Option<&ExternSymbols> {
        match self {
            Self::Elf(elf) => elf.extern_symbols(),
            Self::Object(object) => object.extern_symbols(),
        }
    }

    fn attributes(&self) -> &AttributeMap {
        match self {
            Self::Elf(elf) => elf.attributes(),
            Self::Object(object) => object.attributes(),
        }
    }

    fn attributes_mut(&mut self) -> &mut AttributeMap {
        match self {
            Self::Elf(elf) => elf.attributes_mut(),
            Self::Object(object) => object.attributes_mut(),
        }
    }

    fn segments<'a>(
        &'a self,
    ) -> impl FallibleIterator<Item = LoadableSegment<'a>, Error = LoaderError> + 'a {
        match self {
            Self::Elf(elf) => {
                Box::new(elf.segments()) as Box<dyn FallibleIterator<Item = _, Error = _>>
            }
            Self::Object(object) => {
                Box::new(object.segments()) as Box<dyn FallibleIterator<Item = _, Error = _>>
            }
        }
    }

    fn segment_range(&self) -> (Address, Address) {
        match self {
            Self::Elf(elf) => elf.segment_range(),
            Self::Object(object) => object.segment_range(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::attributes;

    use super::*;

    #[test]
    fn test_loader() -> Result<(), LoaderError> {
        let loaded = Loader::from_file_with(
            "tests/ls.elf",
            attributes![
                "test" => "test",
                "test2" => 2u32,
                "test3" => 3u64,
            ],
        )?;

        assert_eq!(loaded.architecture().language().id(), "x86:LE:64:default");
        assert_eq!(
            loaded.attributes().get_attr::<String>("test"),
            Some("test".to_owned())
        );
        assert_eq!(loaded.attributes().get_attr::<u32>("test2"), Some(2));
        assert_eq!(loaded.attributes().get_attr::<u64>("test3"), Some(3));

        let (start, end) = loaded.segment_range();

        println!("segment range: {start:#x} - {end:#x}");

        Ok(())
    }
}
