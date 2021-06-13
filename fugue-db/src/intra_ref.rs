use crate::Id;
use crate::BasicBlock;
use crate::Function;

use crate::error::Error;
use crate::schema;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IntraRef<'db> {
    source_id: Id<BasicBlock<'db>>,
    target_id: Id<BasicBlock<'db>>,
    function_id: Id<Function<'db>>,
}

impl<'db> IntraRef<'db> {
    pub fn source_id(&self) -> Id<BasicBlock<'db>> {
        self.source_id.clone()
    }

    pub fn target_id(&self) -> Id<BasicBlock<'db>> {
        self.target_id.clone()
    }

    pub fn function_id(&self) -> Id<Function<'db>> {
        self.function_id.clone()
    }

    pub(crate) fn from_reader(reader: schema::intra_ref::Reader) -> Result<Self, Error> {
        Ok(Self {
            source_id: reader.get_source().into(),
            target_id: reader.get_target().into(),
            function_id: reader.get_function().into(),
        })
    }

    pub(crate) fn to_builder(&self, builder: &mut schema::intra_ref::Builder) -> Result<(), Error> {
        builder.set_source(self.source_id().value());
        builder.set_target(self.target_id().value());
        builder.set_function(self.function_id().index() as u32);
        Ok(())
    }
}
