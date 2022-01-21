use crate::Id;
use crate::Function;

use crate::error::Error;
use crate::schema;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InterRef<'db> {
    address: u64,
    source_id: Id<Function<'db>>,
    target_id: Id<Function<'db>>,
    call: bool,
}

impl<'db> InterRef<'db> {
    pub fn address(&self) -> u64 {
        self.address
    }

    pub fn source_id(&self) -> Id<Function<'db>> {
        self.source_id.clone()
    }

    pub fn target_id(&self) -> Id<Function<'db>> {
        self.target_id.clone()
    }

    pub fn is_call(&self) -> bool {
        self.call
    }

    pub fn is_jump(&self) -> bool {
        !self.call
    }

    pub(crate) fn from_reader(reader: schema::InterRef) -> Result<Self, Error> {
        Ok(Self {
            address: reader.address(),
            source_id: reader.source().into(),
            target_id: reader.target().into(),
            call: reader.call(),
        })
    }

    pub(crate) fn to_builder<'a: 'b, 'b>(
        &self,
        builder: &'b mut flatbuffers::FlatBufferBuilder<'a>
    ) -> Result<flatbuffers::WIPOffset<schema::InterRef<'a>>, Error> {
        let mut ibuilder = schema::InterRefBuilder::new(builder);

        ibuilder.add_address(self.address());
        ibuilder.add_source(self.source_id().index() as u32);
        ibuilder.add_target(self.target_id().index() as u32);
        ibuilder.add_call(self.is_call());

        Ok(ibuilder.finish())
    }
}

