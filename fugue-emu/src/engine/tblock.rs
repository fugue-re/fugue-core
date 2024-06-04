//! translation block
//! 
//! a translation block that holds lifted instructions
//! and their original bytes
//! (conceptually identical to QEMU translation blocks)
use std::sync::Arc;
use nohash_hasher::{ IntMap, IsEnabled };
use thiserror;

use fugue_ir::{
    Address,
    disassembly::lift::{ IRBuilderArena, ArenaVec },
    disassembly::error as disassembly_error,
    disassembly::Opcode,
    error as ir_error,
};
use fugue_core::{
    lifter::Lifter,
    ir::{ PCode, Location },
};
use crate::context;
use crate::context::ContextError;
use crate::context::manager::ContextManager;

#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error("Disassembly Error: {0}")]
    Disassembly(disassembly_error::Error),
    #[error("Context Error: {0}")]
    Context(context::ContextError),
}

impl From<context::ContextError> for TranslationError {
    fn from(err: context::ContextError) -> Self {
        TranslationError::Context(err)
    }
}

impl From<disassembly_error::Error> for TranslationError {
    fn from(err: disassembly_error::Error) -> Self {
        TranslationError::Disassembly(err)
    }
}

impl From<ir_error::Error>  for TranslationError {
    fn from(err: ir_error::Error) -> Self {
        let ir_error::Error::Disassembly(err) = err else {
            panic!("could not convert ir error to disassembly error!")
        };
        TranslationError::Disassembly(err)
    }
}

/// translation block
/// holds a list of lift results
/// designed for pre-fetching instructions and to defer
/// error propagation until instructions actually fetched
/// 
/// Note: never attempt to hash an empty TranslationBlock as
/// this will panic since a hash cannot be generated for from 
/// the empty addrs vector.
pub struct TranslationBlock<'a> {
    pub addrs: ArenaVec<'a, Address>,
    pub insns: IntMap<u64, Result<Arc<TranslatedInsn<'a>>, TranslationError>>,
    // predecessors: ArenaVec<'a, Address>,
    // successors: ArenaVec<'a, Address>,
}

impl<'a> TranslationBlock<'a> {
    pub fn new_with<'z, 'b, 'c>(
        lifter: &mut Lifter<'b>,
        irb: &'z IRBuilderArena,
        base: impl Into<Address>,
        context: &ContextManager<'c>,
    ) -> Self
        where 'z : 'a
    {
        let base_address = base.into();
        let mut offset = 0usize;
        let mut addrs: ArenaVec<'z, Address> = ArenaVec::new_in(irb.inner());
        let mut insns = IntMap::default();
        loop {
            let address = base_address + offset;
            addrs.push(address);
            let read_result = context.read_mem_slice(address, 4);
            if let Err(err) = read_result {
                insns.insert(address.offset(), Err(TranslationError::Context(err)));
                break;
            };
            let bytes = read_result.unwrap();
            let translation_result = TranslatedInsn::new_with(lifter, irb, address, bytes);
            match translation_result {
                Err(err) => {
                    insns.insert(address.offset(), Err(err));
                    break;
                },
                Ok(translated_insn) => {
                    offset += translated_insn.bytes.len();
                    let last_opcode = translated_insn.pcode.operations
                        .last()
                        .unwrap().opcode;
                    insns.insert(address.offset(), Ok(Arc::new(translated_insn)));
                    match last_opcode {
                        Opcode::Branch |
                        Opcode::CBranch |
                        Opcode::IBranch |
                        Opcode::Call |
                        Opcode::ICall |
                        Opcode::Return => {
                            break;
                        },
                        _ => { },
                    }
                }
            }
        }
        TranslationBlock {
            addrs,
            insns,
            // predecessors: ArenaVec::new_in(irb.inner()),
            // successors: ArenaVec::new_in(irb.inner()),
        }
    }

    pub fn base(&self) -> Option<&Address> {
        self.addrs.get(0)
    }

    pub fn get_translated_insn(
        &self,
        address: &Address,
    ) -> Option<&'a Result<Arc<TranslatedInsn>, TranslationError>> {
        self.insns.get(&address.offset())
    }

    pub fn contains_address(&self, address: &Address) -> bool {
        self.insns.contains_key(&address.offset())
    }
}

impl PartialEq for TranslationBlock<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.base() == other.base()
    }
}
impl Eq for TranslationBlock<'_> {}

impl std::hash::Hash for TranslationBlock<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.base().unwrap().offset())
    }
}

impl IsEnabled for TranslationBlock<'_> {}

use std::fmt;
impl fmt::Debug for TranslationBlock<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ops_str: Vec<String> = self.addrs.iter()
            .map(| addr | {
                format!("{:?}", self.insns.get(&addr.offset()).unwrap())
            }).collect();
        writeln!(f, "TranslationBlock {{\n\t{}\n}}", ops_str.join("\n\t"))
    }
}

/// translated instruction
/// holds the pcode that the instruction was lifted to, which is
/// backed by a bumpalo arena in an IRBuilderArena passed via dependency injection
/// the bytes that the pcode was lifted from will also cloned and stored in
/// the same arena
pub struct TranslatedInsn<'a> {
    pub pcode: PCode<'a>,
    pub bytes: ArenaVec<'a, u8>,
}

impl<'a> TranslatedInsn<'a> {
    pub fn new_with<'z, 'b>(
        lifter: &mut Lifter<'b>,
        irb: &'z IRBuilderArena,
        address: impl Into<Address>,
        bytes: impl AsRef<[u8]>,
    ) -> Result<Self, TranslationError>
        where 'z : 'a
    {
        let address = address.into();
        let bytes = bytes.as_ref();
        let pcode = lifter.lift(irb, address, bytes)?;
        let insn_bytes = &bytes[..pcode.len()];
        Ok(TranslatedInsn {
            pcode,
            bytes: ArenaVec::from_iter_in(insn_bytes.iter().cloned(), irb.inner()),
        })
    }
}

impl fmt::Debug for TranslatedInsn<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "< Address: 0x{:x?} | Bytes: {:x?} >", 
            self.pcode.address.offset(),
            &self.bytes[..]
        )
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use crate::context::{
        ContextType,
        manager::ContextManager,
    };
    use fugue_core::{ir::Insn, language::LanguageBuilder};

    #[test]
    fn test_new_translated_insn() {
        let lang_builder = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");

        let mut lifter = lang.lifter();
        let irb = lifter.irb(1024);

        let insn_bytes: &[u8] = &[
            0x70, 0x47,             // 00: bx lr
        ];

        let translated_insn = TranslatedInsn::new_with(
            &mut lifter,
            &irb,
            Address::from(0x1000u64),
            insn_bytes,
        ).expect("failed to translate instruction");

        assert!(&translated_insn.bytes[..] == insn_bytes);
        assert!(translated_insn.pcode.operations.len() > 0, "pcode: {:?}", translated_insn.pcode);
    }

    #[test]
    fn test_translate_block() {
        let lang_builder = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");

        let mut lifter = lang.lifter();
        let context_lifter = lang.lifter();
        let irb = context_lifter.irb(1024);

        // map concrete context memory
        let mem_size = 0x1000usize;
        let mut context_manager = ContextManager::new(&context_lifter);
        context_manager.map_memory(
            0x0u64,
            mem_size,
            Some(ContextType::Concrete)
        ).expect("failed to map memory");

        let program_mem: &[u8] = &[
            // 0000 <main>:
            0x80, 0xb5,             // 00: push     {r7, lr}
            0x82, 0xb0,             // 02: sub      sp, #8
            0x00, 0xaf,             // 04: add      r7, sp, #0
            0x03, 0x23,             // 06: movs     r3, #3
            0x7b, 0x60,             // 08: str      r3, [r7, #4]
            0x00, 0x23,             // 0a: movs     r3, #0
            0x3b, 0x60,             // 0c: str      r3, [r7, #0]
            0x06, 0xe0,             // 0e: b.n      1e <main+0x1e>
        ];

        // load program
        context_manager
            .write_mem(Address::from(0u64), program_mem)
            .expect("failed to write bytes");

        let block = TranslationBlock::new_with(
            &mut lifter,
            &irb,
            Address::from(0u64),
            &context_manager,
        );

        let addrs_len = block.addrs.len();
        assert!(addrs_len == 8, "{:?}", block);

        let last_addr = block.addrs.last().unwrap();
        let result = block.insns.get(&last_addr.offset());
        match result {
            None => panic!("Fetch Failed!"),
            Some(result) => {
                let result = result.as_ref();
                let insn = result
                    .as_ref()
                    .expect("Fetch Failed!");
                assert!(&insn.bytes[..] == &[0x06, 0xe0]);
            }
        };
    }

    #[test]
    fn test_translate_udf_instruction() {
        let lang_builder = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");

        let mut lifter = lang.lifter();
        let context_lifter = lang.lifter();
        let irb = context_lifter.irb(1024);

        // map concrete context memory
        let mem_size = 0x1000usize;
        let mut context_manager = ContextManager::new(&context_lifter);
        context_manager.map_memory(
            0x0u64,
            mem_size,
            Some(ContextType::Concrete)
        ).expect("failed to map memory");

        let program_mem: &[u8] = &[
            0x80, 0xb5,             // 00: push     {r7, lr}
            0x82, 0xb0,             // 02: sub      sp, #8
            0x00, 0xaf,             // 04: add      r7, sp, #0
            0x03, 0x23,             // 06: movs     r3, #3
            0x7b, 0x60,             // 08: str      r3, [r7, #4]
            0xad, 0xde,             // 0a: udf      #0xad
            0x00, 0x23,             // 0c: movs     r3, #0
            0x3b, 0x60,             // 0e: str      r3, [r7, #0]
            0x06, 0xe0,             // 10: b.n      1e <main+0x1e>
        ];

        // load program
        context_manager
            .write_mem(Address::from(0u64), program_mem)
            .expect("failed to write bytes");

        let block = TranslationBlock::new_with(
            &mut lifter,
            &irb,
            Address::from(0u64),
            &context_manager,
        );

        let addrs_len = block.addrs.len();
        assert!(addrs_len == 6, "{:?}", block);

        let last_addr = block.addrs.last().unwrap();
        let Some(result) = block.insns.get(&last_addr.offset()) else {
            panic!("Fetch Failed!")
        };
        let result = result.as_ref();
        let insn = result
            .as_ref()
            .expect("Fetch Failed!");
        let first_opcode = insn.pcode.operations
            .first().unwrap().opcode;
        assert!(first_opcode == Opcode::CallOther, "{:?}", first_opcode);
    }
}