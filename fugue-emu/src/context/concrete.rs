//! A concrete context module
//! 
//! single-block memory meant for use with context manager
#![allow(unused_imports)]
use std::fmt;
use fugue_bv::BitVec;
use fugue_bytes::Endian;
use fugue_ir::{
    Address,
    AddressSpace,
    Translator,
    VarnodeData,
};
use fugue_core::{
    ir::Location,
    lifter::Lifter,
    eval::{
        fixed_state::FixedState,
        fixed_state::FixedStateError,
        EvaluatorError,
        EvaluatorContext,
    },
};
use std::collections::HashMap;
use crate::context::{
    MappedContext,
    ContextError,
};


/// ConcreteMemory
/// 
/// Implements memory as a single Fixed State.
/// Assumes a 32-bit Address and that all accesses are by absolute address.
pub struct ConcreteMemory {
    base: Address,
    endian: Endian,
    memory: FixedState,
}

impl ConcreteMemory {
    /// instantiate a new concrete context
    pub fn new(
        base: impl Into<Address>, 
        endian: Endian, 
        size: usize
    ) -> Self {
        Self {
            base: base.into(),
            endian: endian,
            memory: FixedState::new(size),
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

// note: these will break if attempt to access a varnode with size larger than 0x1000
impl EvaluatorContext for ConcreteMemory {

    fn read_vnd(&mut self, var: &VarnodeData) -> Result<BitVec, EvaluatorError> {
        let spc = var.space();
        if spc.is_default() {
            let addr = self.translate(var.offset())?;
            self.memory
                .read_val_with(addr, var.size(), self.endian)
                .map_err(EvaluatorError::state)
        } else {
            Err(EvaluatorError::state_with("expected default address space id "))
        }
    }

    fn write_vnd(&mut self, var: &VarnodeData, val: &BitVec) -> Result<(), EvaluatorError> {
        let spc = var.space();
        if spc.is_default() {
            let addr = self.translate(var.offset())?;
            self.memory
                .write_val_with(addr, val, self.endian)
                .map_err(EvaluatorError::state)
        } else {
            Err(EvaluatorError::state_with("expected default address space id "))
        }
    }
}

impl MappedContext for ConcreteMemory {

    fn base(&self) -> Address {
        self.base.clone()
    }

    fn size(&self) -> usize {
        self.memory.len()
    }

    /// read bytes from memory
    /// returns a vector of bytes
    fn read_bytes(
        &self, 
        address: Address, 
        size: usize
    ) -> Result<Vec<u8>, ContextError> {
        let offset = self.translate(u64::from(address))?;
        self.memory.view_bytes(offset, size)
            .map_err(ContextError::state)
            .map(Vec::from)
    }

    /// write bytes to memory
    fn write_bytes(
        &mut self,
        address: Address,
        values: &[u8],
    ) -> Result<(), ContextError> {
        let offset = self.translate(u64::from(address))?;
        self.memory.write_bytes(offset, values)
            .map_err(ContextError::state)
    }
}
