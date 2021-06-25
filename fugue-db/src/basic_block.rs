use fugue_ir::disassembly::ContextDatabase;
use fugue_ir::PCode;
use fugue_ir::Translator;
use fugue_ir::il::Instruction;
use interval_tree::IntervalTree;

use crate::ArchitectureDef;
use crate::Id;
use crate::IntraRef;
use crate::Segment;

use crate::error::Error;
use crate::schema;

use educe::Educe;

#[derive(Educe)]
#[educe(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BasicBlock<'db> {
    address: u64,
    length: usize,
    architecture_id: Id<ArchitectureDef>,
    predecessors: Vec<IntraRef<'db>>,
    successors: Vec<IntraRef<'db>>,
    segment: &'db Segment,
    #[educe(
        Debug(ignore),
        PartialEq(ignore),
        PartialOrd(ignore),
        Ord(ignore),
        Hash(ignore)
    )]
    translator: &'db Translator,
}

impl<'db> BasicBlock<'db> {
    pub fn address(&self) -> u64 {
        self.address
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn segment(&self) -> &'db Segment {
        self.segment
    }

    pub fn successors(&self) -> &[IntraRef] {
        &self.successors
    }

    pub fn predecessors(&self) -> &[IntraRef] {
        &self.predecessors
    }

    pub fn bytes(&self) -> &'db [u8] {
        let offset = (self.address() - self.segment.address()) as usize;
        &self.segment.bytes()[offset..offset + self.len()]
    }

    pub fn translate_with<F, O>(&self, context: &mut ContextDatabase, mut f: F) -> Result<Vec<O>, Error>
    where F: FnMut(&'db Translator, &mut ContextDatabase, u64, &[u8]) -> Result<(O, usize), Error> {
        let mut offset = 0;
        let mut outputs = Vec::with_capacity(16);

        let block_addr = self.address();
        let bytes = self.bytes();

        while offset < self.bytes().len() {
            let address = block_addr + offset as u64;
            let (output, length) = f(self.translator, context, address, &bytes[offset..])?;

            offset += length;
            outputs.push(output);
        }

        Ok(outputs)
    }

    pub fn disassemble_with(&self, context: &mut ContextDatabase) -> Result<Vec<Instruction>, Error> {
        self.translate_with(context, |translator, context, address, bytes| {
            let insn = translator.disassemble(context, translator.address(address), bytes)
                .map_err(|source| Error::Lifting { address, source })?;
            let length = insn.length();
            Ok((insn, length))
        })
    }

    pub fn disassemble(&self) -> Result<Vec<Instruction>, Error> {
        let mut context = self.translator.context_database();
        self.disassemble_with(&mut context)
    }


    pub fn lift_with(&self, context: &mut ContextDatabase) -> Result<Vec<PCode>, Error> {
        self.translate_with(context, |translator, context, address, bytes| {
            let pcode = translator.lift_pcode(context, translator.address(address), bytes)
                .map_err(|source| Error::Lifting { address, source })?;
            let length = pcode.length();
            Ok((pcode, length))
        })
    }

    pub fn lift(&self) -> Result<Vec<PCode>, Error> {
        let mut context = self.translator.context_database();
        self.lift_with(&mut context)
    }

    pub(crate) fn from_reader(
        reader: schema::basic_block::Reader,
        segments: &'db IntervalTree<u64, Segment>,
        translators: &'db [Translator],
    ) -> Result<Self, Error> {
        let architecture_id = Id::<ArchitectureDef>::from(reader.get_architecture());
        let arch_index = architecture_id.index();
        Ok(Self {
            address: reader.get_address(),
            length: reader.get_length() as usize,
            architecture_id,
            segment: segments
                .find(&reader.get_address())
                .ok_or_else(|| Error::NoBlockSegment(reader.get_address()))?
                .value(),
            predecessors: reader
                .get_predecessors()
                .map_err(Error::Deserialisation)?
                .into_iter()
                .map(IntraRef::from_reader)
                .collect::<Result<Vec<_>, _>>()?,
            successors: reader
                .get_successors()
                .map_err(Error::Deserialisation)?
                .into_iter()
                .map(IntraRef::from_reader)
                .collect::<Result<Vec<_>, _>>()?,
            translator: &translators[arch_index],
        })
    }

    pub(crate) fn to_builder(
        &self,
        builder: &mut schema::basic_block::Builder,
    ) -> Result<(), Error> {
        builder.set_address(self.address());
        builder.set_length(self.len() as u32);
        builder.set_architecture(self.architecture_id.index() as u32);
        let mut predecessors = builder
            .reborrow()
            .init_predecessors(self.predecessors.len() as u32);
        self.predecessors
            .iter()
            .enumerate()
            .try_for_each(|(i, r)| {
                let mut builder = predecessors.reborrow().get(i as u32);
                r.to_builder(&mut builder)
            })?;
        let mut successors = builder
            .reborrow()
            .init_successors(self.successors.len() as u32);
        self.successors.iter().enumerate().try_for_each(|(i, r)| {
            let mut builder = successors.reborrow().get(i as u32);
            r.to_builder(&mut builder)
        })?;
        Ok(())
    }
}
