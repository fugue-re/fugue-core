use fugue_ir::Translator;
use intervals::collections::IntervalTree;

use crate::Id;
use crate::BasicBlock;
use crate::InterRef;
use crate::Segment;

use crate::error::Error;
use crate::schema;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Function<'db> {
    symbol: String,
    entry: Id<BasicBlock<'db>>,
    address: u64,
    segment: Id<Segment>,
    blocks: Vec<BasicBlock<'db>>,
    references: Vec<InterRef<'db>>,
}

impl<'db> Function<'db> {
    pub fn name(&self) -> &str {
        &self.symbol
    }

    pub fn address(&self) -> u64 {
        self.address
    }

    pub fn entry(&self) -> Option<&BasicBlock> {
        self.blocks.get(self.entry.index())
    }

    pub fn blocks(&self) -> &[BasicBlock] {
        &self.blocks
    }

    pub fn segment_id(&self) -> Id<Segment> {
        self.segment.clone()
    }

    pub fn references(&self) -> &[InterRef] {
        &self.references
    }

    pub(crate) fn from_reader(reader: schema::Function, segments: &'db IntervalTree<u64, Segment>, translators: &'db [Translator]) -> Result<Self, Error> {
        Ok(Self {
            symbol: reader.symbol().ok_or(Error::DeserialiseField("symbol"))?.to_string(),
            entry: Id::from(reader.entry()),
            address: reader.address(),
            segment: segments.find(&reader.address())
                .ok_or_else(|| Error::NoFunctionSegment(reader.address()))?.index().into(),
            blocks: reader.blocks().ok_or(Error::DeserialiseField("blocks"))?
                .into_iter()
                .map(|b| BasicBlock::from_reader(b, segments, translators))
                .collect::<Result<Vec<_>, _>>()?,
            references: reader.references().ok_or(Error::DeserialiseField("references"))?
                .into_iter()
                .map(InterRef::from_reader)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    pub(crate) fn to_builder<'a: 'b, 'b>(
        &self,
        builder: &'b mut flatbuffers::FlatBufferBuilder<'a>
    ) -> Result<flatbuffers::WIPOffset<schema::Function<'a>>, Error> {
        let symbol = builder.create_string(self.name());

        let bvec = self.blocks
            .iter()
            .map(|r| r.to_builder(builder))
            .collect::<Result<Vec<_>, _>>()?;

        let blocks = builder.create_vector_from_iter(bvec.into_iter());

        let rvec = self.references
            .iter()
            .map(|r| r.to_builder(builder))
            .collect::<Result<Vec<_>, _>>()?;

        let references = builder.create_vector_from_iter(rvec.into_iter());

        let mut fbuilder = schema::FunctionBuilder::new(builder);

        fbuilder.add_symbol(symbol);
        fbuilder.add_address(self.address());
        fbuilder.add_entry(self.entry.value());
        fbuilder.add_blocks(blocks);
        fbuilder.add_references(references);

        Ok(fbuilder.finish())
    }
}

