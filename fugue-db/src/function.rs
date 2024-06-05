use fugue_ir::Address;
use rkyv::{Archive, Deserialize, Serialize};

use crate::project::{ProjectDBError, ProjectDBReader, ProjectDBStorable, ProjectDBWriter};
use crate::types::key::Key;

#[derive(Archive, Deserialize, Serialize)]
pub struct Function {
    #[with(crate::types::address::Address)]
    addr: Address,
    name: String, // NOTE: we could use SmolStr
}

impl ProjectDBStorable for Function {
    fn key(&self) -> Key {
        Key::function(self.addr)
    }

    fn fetch<'a>(_db: &ProjectDBReader<'a>) -> Result<&'a ArchivedFunction, ProjectDBError> {
        todo!()
    }

    fn store<'a>(&self, db: &mut ProjectDBWriter<'a>) -> Result<(), ProjectDBError> {
        db.write::<_, 0>(&self.key(), self)
    }
}
