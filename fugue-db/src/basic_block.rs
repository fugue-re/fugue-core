use fugue_ir::disassembly::{ContextDatabase, IRBuilderArena};
use fugue_ir::PCode;
use fugue_ir::Translator;
use fugue_ir::il::Instruction;
use iset::IntervalMap;

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
    #[doc(hidden)]
    pub fn architecture(&self) -> usize {
        self.architecture_id.index()
    }

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

    pub fn disassemble_with<'z>(&self, context: &mut ContextDatabase, arena: &'z IRBuilderArena) -> Result<Vec<Instruction<'z>>, Error> {
        self.translate_with(context, |translator, context, address, bytes| {
            let insn = translator.disassemble(context, arena, translator.address(address), bytes)
                .map_err(|source| Error::Lifting { address, source })?;
            let length = insn.length();
            Ok((insn, length))
        })
    }

    pub fn disassemble<'z>(&self, arena: &'z IRBuilderArena) -> Result<Vec<Instruction<'z>>, Error> {
        let mut context = self.translator.context_database();
        self.disassemble_with(&mut context, arena)
    }


    pub fn lift_with<'z>(&self, context: &mut ContextDatabase) -> Result<Vec<PCode>, Error> {
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
        reader: schema::BasicBlock,
        segments: &'db IntervalMap<u64, Segment>,
        translators: &'db [Translator],
    ) -> Result<Self, Error> {
        let architecture_id = Id::<ArchitectureDef>::from(reader.architecture());
        let arch_index = architecture_id.index();
        let address = reader.address();
        Ok(Self {
            address,
            length: reader.size_() as usize,
            architecture_id,
            segment: segments
                .iter(address..address + reader.size_() as u64)
                .find_map(|(_, s)| {
                    if s.is_code() || s.is_external() || s.is_executable() || s.is_readable() {
                        Some(s)
                    } else {
                        None
                    }
                })
                .ok_or_else(|| Error::NoBlockSegment(reader.address()))?,
            predecessors: reader
                .predecessors()
                .ok_or(Error::DeserialiseField("predecessors"))?
                .into_iter()
                .map(IntraRef::from_reader)
                .collect::<Result<Vec<_>, _>>()?,
            successors: reader
                .successors()
                .ok_or(Error::DeserialiseField("successors"))?
                .into_iter()
                .map(IntraRef::from_reader)
                .collect::<Result<Vec<_>, _>>()?,
            translator: &translators[arch_index],
        })
    }

    pub(crate) fn to_builder<'a: 'b, 'b>(
        &self,
        builder: &'b mut flatbuffers::FlatBufferBuilder<'a>,
    ) -> Result<flatbuffers::WIPOffset<schema::BasicBlock<'a>>, Error> {
        let pvec = self.predecessors
            .iter()
            .map(|r| r.to_builder(builder))
            .collect::<Result<Vec<_>, _>>()?;

        let predecessors = builder.create_vector_from_iter(pvec.into_iter());

        let svec = self.successors
            .iter()
            .map(|r| r.to_builder(builder))
            .collect::<Result<Vec<_>, _>>()?;

        let successors = builder.create_vector_from_iter(svec.into_iter());

        let mut bbuilder = schema::BasicBlockBuilder::new(builder);

        bbuilder.add_address(self.address());
        bbuilder.add_size_(self.len() as u32);
        bbuilder.add_architecture(self.architecture_id.index() as u32);
        bbuilder.add_predecessors(predecessors);
        bbuilder.add_successors(successors);

        Ok(bbuilder.finish())
    }
}
