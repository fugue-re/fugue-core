use crate::Id;
use crate::Function;

use crate::error::Error;
use crate::schema;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde_derive", derive(serde::Deserialize, serde::Serialize))]
pub struct InterRef<'db> {
    address: u64,
    source: Id<Function<'db>>,
    target: Id<Function<'db>>,
    call: bool,
}

impl<'db> InterRef<'db> {
    pub fn address(&self) -> u64 {
        self.address
    }

    pub fn source_id(&self) -> Id<Function> {
        self.source.clone()
    }

    pub fn target_id(&self) -> Id<Function> {
        self.target.clone()
    }

    pub fn is_call(&self) -> bool {
        self.call
    }

    pub fn is_jump(&self) -> bool {
        !self.call
    }

    pub(crate) fn from_reader(reader: schema::inter_ref::Reader) -> Result<Self, Error> {
        Ok(Self {
            address: reader.get_address(),
            source: reader.get_source().into(),
            target: reader.get_target().into(),
            call: reader.get_call(),
        })
    }

    pub(crate) fn to_builder(&self, builder: &mut schema::inter_ref::Builder) -> Result<(), Error> {
        builder.set_address(self.address());
        builder.set_source(self.source_id().index() as u32);
        builder.set_target(self.target_id().index() as u32);
        builder.set_call(self.is_call());
        Ok(())
    }
}

