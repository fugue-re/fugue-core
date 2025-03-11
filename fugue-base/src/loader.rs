use std::borrow::Cow;

use bitflags::bitflags;
use fugue_lifter::{Language, Lifter};

use crate::types::{Address, AttributeMap};

pub mod object;
pub mod shellcode;

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct LoadedRegionProperties: u8 {
        const PERM_READ     = 0b0000_0001;
        const PERM_WRITE    = 0b0000_0010;
        const PERM_EXECUTE  = 0b0000_0100;

        const PERM_ALL      = Self::PERM_READ.bits() | Self::PERM_WRITE.bits() | Self::PERM_EXECUTE.bits();

        const UNINITIALISED = 0b0001_0000;
    }
}

impl LoadedRegionProperties {
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
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoadedRegion<'a> {
    name: Cow<'a, str>,
    address: Address,
    properties: LoadedRegionProperties,
    bytes: Cow<'a, [u8]>,
}

pub trait LoadedBinary {
    fn attributes(&self) -> &AttributeMap;
    fn attributes_mut(&mut self) -> &mut AttributeMap;

    fn entry_address(&self) -> Option<Address>;

    fn language(&self) -> &'static Language {
        self.lifter().language()
    }

    fn lifter(&self) -> &Lifter;
    fn lifter_mut(&mut self) -> &mut Lifter;

    fn regions<'a>(&'a self) -> impl Iterator<Item = LoadedRegion<'a>> + 'a;
}
