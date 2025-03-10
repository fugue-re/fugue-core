use std::borrow::Cow;

use crate::loader::{LoadedBinary, LoadedRegion, LoadedRegionProperties};
use crate::types::Address;

#[derive(Debug, Clone)]
pub struct Shellcode {
    address: Address,
    bytes: Vec<u8>,
}

impl Shellcode {
    pub fn new(address: impl Into<Address>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            address: address.into(),
            bytes: bytes.into(),
        }
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
}
