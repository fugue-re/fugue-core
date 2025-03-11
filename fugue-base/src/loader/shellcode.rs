use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use fugue_lifter::{Language, Lifter, LifterBuilder, LifterBuilderError};
use thiserror::Error;

use crate::loader::{LoadedBinary, LoadedRegion, LoadedRegionProperties};
use crate::types::{Address, AttributeMap};

#[derive(Clone)]
pub struct Shellcode {
    address: Address,
    bytes: Vec<u8>,
    lifter: Lifter,
    attributes: AttributeMap,
}

impl fmt::Debug for Shellcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shellcode")
            .field("address", &self.address)
            .field("attributes", &self.attributes)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Error)]
pub enum ShellcodeError {
    #[error("mapping {1} bytes at {0} will overflow the default address space")]
    AddressOverflow(Address, usize),
    #[error("requested language `{0}` unsupported: {1}")]
    UnsupportedLanguage(String, LifterBuilderError),
    #[error("buffer to map must be not be empty")]
    ZeroSized,
}

impl Shellcode {
    pub fn new(
        language: impl AsRef<str>,
        address: impl Into<Address>,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, ShellcodeError> {
        Self::new_with(language, address, bytes, AttributeMap::default())
    }

    pub fn new_with(
        language: impl AsRef<str>,
        address: impl Into<Address>,
        bytes: impl Into<Vec<u8>>,
        attributes: impl Into<AttributeMap>,
    ) -> Result<Self, ShellcodeError> {
        let language = language.as_ref();
        let lifter = LifterBuilder::from_str(language)
            .and_then(|builder| builder.build())
            .map_err(|e| ShellcodeError::UnsupportedLanguage(language.to_owned(), e))?;

        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(ShellcodeError::ZeroSized);
        }

        let address = address.into();
        let size = bytes.len();

        let language = lifter.language();
        if !address.range_in_space_bounds(language, size) {
            return Err(ShellcodeError::AddressOverflow(address, size));
        }

        Ok(Self {
            address: address.into(),
            bytes: bytes.into(),
            lifter,
            attributes: attributes.into(),
        })
    }

    pub fn address(&self) -> Address {
        self.address
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl LoadedBinary for Shellcode {
    fn entry_address(&self) -> Option<Address> {
        Some(self.address())
    }

    fn regions<'a>(&'a self) -> impl Iterator<Item = LoadedRegion<'a>> + 'a {
        std::iter::once(LoadedRegion {
            name: Cow::Borrowed("LOAD"),
            address: self.address,
            properties: LoadedRegionProperties::PERM_ALL,
            bytes: Cow::Borrowed(self.bytes()),
        })
    }

    fn language(&self) -> &'static Language {
        self.lifter.language()
    }

    fn lifter(&self) -> &Lifter {
        &self.lifter
    }

    fn lifter_mut(&mut self) -> &mut Lifter {
        &mut self.lifter
    }

    fn attributes(&self) -> &AttributeMap {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut AttributeMap {
        &mut self.attributes
    }
}
