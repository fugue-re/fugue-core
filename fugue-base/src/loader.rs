use std::borrow::Cow;

use bitflags::bitflags;
use object::Object;

use crate::types::Address;

pub mod elf;
pub mod pe;

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct LoadedRegionProperties: u8 {
        const READ    = 0b0001;
        const WRITE   = 0b0010;
        const EXECUTE = 0b0100;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoadedRegion<'a> {
    name: Cow<'a, str>,
    address: Address,
    properties: LoadedRegionProperties,
    bytes: Cow<'a, [u8]>,
}

pub trait LoadedBinary {
    fn regions<'a>(&'a self) -> impl Iterator<Item = LoadedRegion<'a>> + 'a;
    fn entry_address(&self) -> Option<Address>;
}
