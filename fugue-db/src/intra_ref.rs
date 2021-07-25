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

    pub(crate) fn from_reader(reader: schema::IntraRef) -> Result<Self, Error> {
        Ok(Self {
            source_id: reader.source().into(),
            target_id: reader.target().into(),
            function_id: reader.function().into(),
        })
    }

    pub(crate) fn to_builder<'a: 'b, 'b>(
        &self,
        builder: &'b mut flatbuffers::FlatBufferBuilder<'a>
    ) -> Result<flatbuffers::WIPOffset<schema::IntraRef<'a>>, Error> {
        let mut ibuilder = schema::IntraRefBuilder::new(builder);

        ibuilder.add_source(self.source_id().value());
        ibuilder.add_target(self.target_id().value());
        ibuilder.add_function(self.function_id().index() as u32);

        Ok(ibuilder.finish())
    }
}
