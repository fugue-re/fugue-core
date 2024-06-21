//! concrete context
//! 
//! the concrete context is designed for use with the concrete evaluator and
//! as a basis for primarily model-based firmware rehosting.
//! 
//! it is responsible for tracking memory/register/temporary state data,
//! some processor state data (such as program counter edges), caching lifted
//! instructions, managing memory mapped access hooks, peripherals, etc.
use std::sync::Arc;

use nohash_hasher::IntMap;
use parking_lot::{ RwLock, RwLockReadGuard };

use fugue_ir::{ 
    Address, Translator, VarnodeData,
    disassembly::{ 
        PCodeData, Opcode,
        lift::IRBuilderArena 
    },
};
use fugue_bv::BitVec;
use fugue_bytes::Endian;
use fugue_core::lifter::Lifter;
use fugue_core::language::Language;
use fugue_core::ir::{ Location, PCode };

use crate::context;
use crate::context::traits::*;
use crate::context::types::*;
use crate::eval;
use crate::eval::traits::{ EvaluatorContext, observer::BlockObserver };

pub mod state;
pub use state::*;

/// concrete context
/// 
/// a context for a concrete evaluator that holds all state information
#[derive(Clone)]
pub struct ConcreteContext<'irb> {
    // state data
    memory_map: ConcreteMemoryMap,
    regs: ConcreteRegisters,
    tmps: ConcreteTemps,

    // meta
    pc: VarnodeData,
    endian: Endian,
    lang: Language,
    translation_cache: Arc<RwLock<IntMap< u64, LiftResult<'irb> >>>,
}

impl<'irb> ConcreteContext<'irb> {

    /// creates a new concrete context
    pub fn new_with(lang: Language) -> Self {
        Self {
            memory_map: ConcreteMemoryMap::new_with(lang.translator()),
            regs: ConcreteRegisters::new_with(lang.translator()),
            tmps: ConcreteTemps::new_with(lang.translator()),

            pc: lang.translator().program_counter().clone(),
            endian: if lang.translator().is_big_endian() { Endian::Big } else { Endian::Little },
            lang,
            translation_cache: Arc::new(RwLock::new(IntMap::default())),
        }
    }
}

impl<'irb> VarnodeContext<BitVec> for ConcreteContext<'irb> {
    fn read_vnd(&self, var: &VarnodeData) -> Result<BitVec, context::Error> {
        let spc = var.space();
        if spc.is_constant() {
            Ok(BitVec::from_u64(var.offset(), var.bits()))
        } else if spc.is_register() {
            self.regs.read_vnd(var)
        } else if spc.is_unique() {
            self.tmps.read_vnd(var)
        } else if spc.is_default() {
            self.memory_map.read_vnd(var)
        } else {
            Err(context::Error::InvalidVarnode(var.clone()))
        }
    }

    fn write_vnd(&mut self, var: &VarnodeData, val: &BitVec) -> Result<(), context::Error> {
        let spc = var.space();
        if spc.is_constant() {
            panic!("cannot write to constant Varnode!");
        } else if spc.is_register() {
            self.regs.write_vnd(var, val)
        } else if spc.is_unique() {
            self.tmps.write_vnd(var, val)
        } else if spc.is_default() {
            self.memory_map.write_vnd(var, val)
        } else {
            Err(context::Error::InvalidVarnode(var.clone()))
        }
    }
}

/// the EvaluatorContext implementation for ConcreteContext will use the BitVec
/// as the associated Data type
impl<'irb> EvaluatorContext<'irb, BitVec> for ConcreteContext<'irb> {

    fn lift_block(
        &mut self,
        address: impl Into<Address>,
        lifter: &mut Lifter<'_>,
        irb: &'irb mut IRBuilderArena,
    ) -> TranslationBlock {
        
        let base = address.into();
        let mut offsets = vec![0usize];
        // the largest instruction in x86 is 15 bytes
        const MAX_INSN_SIZE: usize = 16;

        'lifting: loop {
            let offset = offsets.last().unwrap();
            let address = base + *offset as u64;

            let read_result = self.read_bytes(&address, MAX_INSN_SIZE);
            if let Err(err) = read_result {
                // read from memory failed
                self.translation_cache.write()
                    .insert(address.offset(), Err(err));
                break 'lifting;
            }
            let bytes = read_result.unwrap();
            let lift_result = lifter.lift(irb, address, bytes);
            if let Err(err) = lift_result {
                // lift failed
                self.translation_cache.write()
                    .insert(address.offset(), Err(err.into()));
                break 'lifting;
            } else {
                // lift succeeded
                let pcode = lift_result.unwrap();
                // update offsets for translation block
                offsets.push(offset + pcode.len());

                // check if the instruction is branching
                let mut is_branch = false;
                match pcode.operations.last().unwrap().opcode {
                    Opcode::Branch | Opcode::CBranch | Opcode::IBranch |
                    Opcode::Call | Opcode::ICall | Opcode::Return => {
                        // usually we can tell if the last opcode is branching
                        is_branch = true;
                    },
                    _ => {
                        // otherwise we need to check if the pc gets written to
                        // todo: there's probably a way to streamline this somehow, 
                        // maybe by only checking certain opcodes or having the PCode
                        // also keep track of the live varnodes
                        // that could be useful for checking liveness...
                        'inner: for pcodedata in pcode.operations.iter() {
                            if let Some(vnd) = pcodedata.output {
                                if vnd == self.pc {
                                    is_branch = true;
                                    break 'inner;
                                }
                            }
                        } // 'inner
                    },
                }

                self.translation_cache.write()
                    .insert(address.offset(), Ok(Arc::new(pcode)));

                if is_branch {
                    break 'lifting;
                }
            };
        } // 'lifting

        // after finished lifting, return a placeholder translation block
        // to represent what was lifted
        let size = offsets.pop().unwrap();
        let bytes = if size > 0 { 
            Vec::from(self.read_bytes(base, size).unwrap())
        } else {
            vec![]
        };
        
        TranslationBlock { base, insn_offsets: offsets, bytes }
    }

    fn fetch(&self, address: impl Into<Address>) -> Result<Arc<PCode<'irb>>, eval::Error> {
        let address = address.into();
        let lift_result = self.translation_cache.read()
            .get(&address.offset())
            .unwrap()
            .clone();

        lift_result.map_err(eval::Error::from)
    }

    fn fork(&self) -> Self {
        self.clone()
    }

}

impl<'irb> MemoryMapContext<BitVec> for ConcreteContext<'irb> {

    fn map_mem(
        &mut self,
        base: impl Into<Address>,
        size: usize,
    ) -> Result<(), context::Error> {
        self.memory_map.map_mem(base, size)
    }

    fn map_mmio(
        &mut self,
        base: impl Into<Address>,
        peripheral: Box<dyn crate::peripheral::traits::MappedPeripheralState>,
    ) -> Result<(), context::Error> {
        self.memory_map.map_mmio(base, peripheral)
    }

    fn read_bytes(&self, address: impl AsRef<Address>, size: usize) -> Result<&[u8], context::Error> {
        self.memory_map.read_bytes(address.as_ref(), size)
    }

    fn write_bytes(&mut self, address: impl AsRef<Address>, bytes: &[u8]) -> Result<(), context::Error> {
        self.memory_map.write_bytes(address.as_ref(), bytes)
    }

    fn read_mem(&self, address: impl AsRef<Address>, size: usize) -> Result<BitVec, context::Error> {
        self.memory_map.read_mem(address.as_ref(), size)
    }

    fn write_mem(&mut self, address: impl AsRef<Address>, data: &BitVec) -> Result<(), context::Error> {
        self.memory_map.write_mem(address.as_ref(), data)
    }
}

impl <'irb> RegisterContext<BitVec> for ConcreteContext<'irb> {

    fn read_reg(&self, name: &str) -> Result<BitVec, context::Error> {
        self.regs.read_reg(name.as_ref())
    }

    fn write_reg(&mut self, name: &str, data: &BitVec) -> Result<(), context::Error> {
        self.regs.write_reg(name.as_ref(), data)
    }
}





#[cfg(test)]
mod tests {
    use fugue_core::language::LanguageBuilder;
    use super::*;
    
    #[test]
    fn test_concrete_context_init() {
        let lang_builder = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");

        let context = ConcreteContext::new_with(lang);


    }

}