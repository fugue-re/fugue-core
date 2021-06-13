use crate::Id;
use crate::BasicBlock;
use crate::Function;

use crate::error::Error;
use crate::schema;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde_derive", derive(serde::Deserialize, serde::Serialize))]
pub struct IntraRef<'db> {
    source: Id<BasicBlock<'db>>,
    target: Id<BasicBlock<'db>>,
    function: Id<Function<'db>>,
}

impl<'db> IntraRef<'db> {
    pub fn source_id(&self) -> Id<BasicBlock> {
        self.source.clone()
    }

    pub fn target_id(&self) -> Id<BasicBlock> {
        self.target.clone()
    }

    pub fn function_id(&self) -> Id<Function> {
        self.function.clone()
    }

    pub(crate) fn from_reader(reader: schema::intra_ref::Reader) -> Result<Self, Error> {
        Ok(Self {
            source: reader.get_source().into(),
            target: reader.get_target().into(),
            function: reader.get_function().into(),
        })
    }

    pub(crate) fn to_builder(&self, builder: &mut schema::intra_ref::Builder) -> Result<(), Error> {
        builder.set_source(self.source_id().value());
        builder.set_target(self.target_id().value());
        builder.set_function(self.function_id().index() as u32);
        Ok(())
    }
}
