use fugue_ir::Translator;
use interval_tree::IntervalTree;

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

    pub(crate) fn from_reader(reader: schema::function::Reader, segments: &'db IntervalTree<u64, Segment>, translators: &'db [Translator]) -> Result<Self, Error> {
        Ok(Self {
            symbol: reader.get_symbol().map_err(Error::Deserialisation)?.to_string(),
            entry: Id::from(reader.get_entry()),
            address: reader.get_address(),
            segment: segments.find(&reader.get_address())
                .ok_or_else(|| Error::NoFunctionSegment(reader.get_address()))?.index().into(),
            blocks: reader.get_blocks().map_err(Error::Deserialisation)?
                .into_iter()
                .map(|b| BasicBlock::from_reader(b, segments, translators))
                .collect::<Result<Vec<_>, _>>()?,
            references: reader.get_references().map_err(Error::Deserialisation)?
                .into_iter()
                .map(InterRef::from_reader)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    pub(crate) fn to_builder(&self, builder: &mut schema::function::Builder) -> Result<(), Error> {
        builder.set_symbol(self.name());
        builder.set_address(self.address());
        builder.set_entry(self.entry.value());
        let mut blocks = builder.reborrow().init_blocks(self.blocks.len() as u32);
        self.blocks.iter().enumerate().try_for_each(|(i, b)| {
            let mut builder = blocks.reborrow().get(i as u32);
            b.to_builder(&mut builder)
        })?;
        let mut references = builder.reborrow().init_references(self.references.len() as u32);
        self.references.iter().enumerate().try_for_each(|(i, b)| {
            let mut builder = references.reborrow().get(i as u32);
            b.to_builder(&mut builder)
        })?;
        Ok(())
    }
}

