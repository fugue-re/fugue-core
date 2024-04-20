//! A concrete context module
//! 
//! an evaluator context with a segmented memory map

use fugue::bv::BitVec;
use fugue::bytes::Endian;
use fugue::ir::{
    Address,
    AddressSpace,
    Translator,
    VarnodeData,
};
use fugue::high::{
    ir::Location,
    lifter::Lifter,
    eval::{
        fixed_state::FixedState,
        EvaluatorError,
        EvaluatorContext,
    },
};
use std::collections::HashMap;

// todo
// implement a MemoryMap structure that contains more info from svd

/// ConcreteContext
/// 
/// Implements memory as a segmented memory map of FixedStates.
/// Assumes a 32-bit Address.
/// Memory is segmented in minimum 0x1000 byte (4 KB) segments.
pub struct ConcreteContext {
    base: Address,
    endian: Endian,
    memory_map: HashMap<u32, FixedState>,
    registers: FixedState,
    temporaries: FixedState,
}

impl ConcreteContext {
    /// instantiate a new concrete context
    pub fn new(
        lifter: &Lifter, 
        base: impl Into<Address>, 
        map_sizes: Vec<(u32, usize)>
    ) -> Self {
        let t = lifter.translator();

        let mut memory_map = HashMap::new();

        // allocate memory map in 4kb chunks. 
        for &(addr, size) in map_sizes.iter() {
            if (size % 0x1000 != 0 || addr % 0x1000 != 0) {
                panic!("memory map not 4KB aligned.");
            }
            let mut addr_base = addr >> 12;
            let mut remaining = size;
            while remaining > 0 {
                memory_map.insert(addr_base, FixedState::new(0x1000));
                remaining -= 0x1000;
                addr_base += 1;
            }
        }

        Self {
            base: base.into(),
            endian: if t.is_big_endian() {
                Endian::Big
            } else {
                Endian::Little
            },
            memory_map: memory_map,
            registers: FixedState::new(t.register_space_size()),
            temporaries: FixedState::new(t.unique_space_size()),
        }
    }

    /// utility function to translate an absolute address to be 
    /// relative to the context base address
    fn translate(&self, addr: u64) -> Result<usize, EvaluatorError> {
        let addr = addr
            .checked_sub(self.base.into())
            .ok_or(EvaluatorError::state_with("address translation out-of-bounds"))?;
        Ok(addr as usize)
    }
}

impl EvaluatorContext for ConcreteContext {
    fn read_vnd(&mut self, var: &VarnodeData) -> Result<BitVec, EvaluatorError> {
        let spc = var.space();
        if spc.is_constant() {
            Ok(BitVec::from_u64(var.offset(), var.size() * 8))
        } else if spc.is_register() {
            self.registers
                .read_val_with(var.offset() as usize, var.size(), self.endian)
                .map_err(EvaluatorError::state)
        } else if spc.is_unique() {
            self.temporaries
                .read_val_with(var.offset() as usize, var.size(), self.endian)
                .map_err(EvaluatorError::state)
        } else {
            let addr = self.translate(var.offset())?;
            // translate addr to access memory hashmap
            let addr_base = (addr >> 12) as u32;
            let offset = addr & 0xFFF;
            let memory_chunk = self.memory_map.get_mut(&addr_base)
                .ok_or(EvaluatorError::state_with(format!("address not mapped 0x{:x}", addr)))?;
            memory_chunk
                .read_val_with(offset, var.size(), self.endian)
                .map_err(EvaluatorError::state)
        }
    }

    fn write_vnd(&mut self, var: &VarnodeData, val: &BitVec) -> Result<(), EvaluatorError> {
        let spc = var.space();
        if spc.is_constant() {
            panic!("cannot write to constant Varnode")
        } else if spc.is_register() {
            self.registers
                .write_val_with(var.offset() as usize, val, self.endian)
                .map_err(EvaluatorError::state)
        } else if spc.is_unique() {
            self.temporaries
                .write_val_with(var.offset() as usize, val, self.endian)
                .map_err(EvaluatorError::state)
        } else {
            let addr = self.translate(var.offset())?;
            let addr_base = (addr >> 12) as u32;
            let offset = addr & 0xFFF;
            let memory_chunk = self.memory_map.get_mut(&addr_base)
                .ok_or(EvaluatorError::state_with(format!("address not mapped 0x{:x}", addr)))?;
            memory_chunk
                .write_val_with(offset, val, self.endian)
                .map_err(EvaluatorError::state)
        } 
    }
}