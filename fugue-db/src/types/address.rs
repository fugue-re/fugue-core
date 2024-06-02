use rkyv::with::{ArchiveWith, DeserializeWith};
use rkyv::{Archive, Archived, Deserialize, Fallible, Serialize};

use rkyv_with::ArchiveWith;

#[derive(Archive, ArchiveWith, Deserialize, Serialize)]
#[archive_with(from(fugue_ir::Address))]
pub struct Address(#[archive_with(getter = "fugue_ir::Address::offset")] u64);

impl<D> DeserializeWith<Archived<Address>, fugue_ir::Address, D> for Address
where
    D: Fallible,
{
    fn deserialize_with(
        field: &Archived<Address>,
        _deserializer: &mut D,
    ) -> Result<fugue_ir::Address, <D as Fallible>::Error> {
        Ok(fugue_ir::Address::from(field.0))
    }
}
