use object::read::pe::{ImageNtHeaders, PeFile};
use object::Object;

use crate::loader::{LoadedBinary, LoadedRegion, LoadedRegionProperties};
use crate::types::Address;

impl<Pe> LoadedBinary for PeFile<'_, Pe>
where
    Pe: ImageNtHeaders,
{
    fn entry_address(&self) -> Option<Address> {
        Some(self.entry().into())
    }

    fn regions<'a>(&'a self) -> impl Iterator<Item = LoadedRegion<'a>> + 'a {
        std::iter::empty()
    }
}
