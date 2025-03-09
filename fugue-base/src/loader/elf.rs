use object::read::elf::{ElfFile, FileHeader};
use object::Object;

use crate::types::Address;
use crate::loader::{LoadedBinary, LoadedRegion, LoadedRegionProperties};

impl<Elf> LoadedBinary for ElfFile<'_, Elf> where Elf: FileHeader {
    fn entry_address(&self) -> Option<Address> {
        Some(self.entry().into())
    }

    fn regions<'a>(&'a self) -> impl Iterator<Item = LoadedRegion<'a>> + 'a {
        std::iter::empty()
    }
}
