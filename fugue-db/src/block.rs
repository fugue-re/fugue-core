use fugue_ir::Address;
use rkyv::{Archive, Deserialize, Serialize};

use crate::project::{ProjectDBError, ProjectDBReader, ProjectDBStorable, ProjectDBWriter};
use crate::types::key::Key;

#[derive(Archive, Deserialize, Serialize)]
pub struct CodeBlock {
    #[with(crate::types::address::Address)]
    addr: Address,
    size: u32,
}

impl ProjectDBStorable for CodeBlock {
    fn key(&self) -> Key {
        Key::basic_block(self.addr)
    }

    fn fetch<'a>(_db: &ProjectDBReader<'a>) -> Result<&'a ArchivedCodeBlock, ProjectDBError> {
        todo!()
    }

    fn store<'a>(&self, db: &mut ProjectDBWriter<'a>) -> Result<(), ProjectDBError> {
        db.write::<_, 0>(&self.key(), self)
    }
}
