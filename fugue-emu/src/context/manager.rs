//! context manager module
#![allow(unused_imports)]

use std::fmt;

use fugue_bv::BitVec;
use fugue_bytes::Endian;
use fugue_ir::{
    translator, Address, VarnodeData
};
use fugue_core::{
    language::Language,
    lifter::Lifter,
    eval::{
        fixed_state::FixedState,
        EvaluatorError,
        EvaluatorContext,
    },
    ir::{
        // Insn,
        PCode,
        Location,
    },
};

use crate::context::{
    ContextType,
    MappedContext,
    ContextError,
};

use super::concrete::ConcreteMemory;
use super::memory_map::MemoryMap;

/// A context manager
/// 
/// Takes in multiple contexts that implement EvaluatorContext
/// and implements EvaluatorContext to call indirectly.
/// 
/// The ContextManager allows multiple contexts to be declared
/// and modified by the Evaluator, and facilitates access for
/// the user.
pub struct ContextManager<'a> {
    // primary data
    memory_map: MemoryMap,
    regs: FixedState,
    tmps: FixedState,
    endian: Endian,
    
    // other useful things to have
    lifter: Lifter<'a>,
}

impl<'a> ContextManager<'a> {
    /// instantiate a new context manager
    /// 
    /// note that the context manager needs its own lifter, it is not a borrow!
    /// this is to make explicit that the lifter created for the context manager
    /// cannot be used for anything else.
    pub fn new(
        // lang: &'a Language,
        lifter: Lifter<'a>,
        irb_size: Option<usize>,
    ) -> Self {
        // let lifter = lang.lifter();
        let t = lifter.translator();
        let irb = lifter.irb(irb_size.unwrap_or(0x1000usize));
        let endian = if t.is_big_endian() { 
            Endian::Big 
        } else { 
            Endian::Little 
        };
        let memory_map = MemoryMap::new();
        Self {
            memory_map: memory_map,
            regs: FixedState::new(t.register_space_size()),
            tmps: FixedState::new(t.unique_space_size()),
            endian: endian,
            lifter: lifter,
        }
    }
    
    /// add a memory region to the context manager
    /// 
    /// memory base address and allocation size will be aligned to 0x1000
    /// endianness is inferred from the lifter
    pub fn map_memory(
        &mut self, 
        base: impl Into<Address>,
        size: usize,
        context_type: Option<ContextType>,
    ) -> Result<&mut Self, ContextError> {

        // check arguments
        let base_address = base.into();
        if u64::from(base_address) & 0xFFFu64 != 0 {
            return Err(ContextError::UnalignedAddress(base_address))
        }
        if size & 0xFFFusize != 0 {
            return Err(ContextError::UnalignedSize(size, 0x1000usize))
        }

        let context = match context_type {
            Some(ContextType::Concrete) | None => {
                ConcreteMemory::new(Address::from(base_address), self.endian, size)
            }
            // for additional future memory types
        };

        // add memory to memory map
        self.memory_map.map_context(Box::new(context))?;
        Ok(self)
    }

    /// returns a slice of bytes starting from the given address
    pub fn read_mem_slice(
        &self, 
        address: Address,
        size: usize
    ) -> Result<&[u8], ContextError> {
        self.memory_map.read_bytes_slice(address, size)
    }

    /// return a vector of bytes read from given address
    pub fn read_mem(
        &self,
        address: Address,
        size: usize,
    ) -> Result<Vec<u8>, ContextError> {
        self.memory_map.read_bytes(address, size)
    }

    /// write bytes to context at given address
    pub fn write_mem(
        &mut self,
        address: Address,
        values: &[u8],
    ) -> Result<(), ContextError> {
        let context = self.memory_map
            .get_mut_context_at(address)
            .map_err(ContextError::state)?;
        context.write_bytes(address, values)
    }

    /// read register by name
    pub fn read_reg<S>(&mut self, name: S) -> Result<BitVec, ContextError> 
    where
        S: AsRef<str>
    {
        let translator = self.lifter.translator();
        if let Some(reg) = translator.register_by_name(name) {
            self.regs
                .read_val_with(reg.offset() as usize, reg.size(), self.endian)
                .map_err(ContextError::state)
        } else {
            Err(ContextError::state_with("register does not exist"))
        }
    }

    /// write register by name
    pub fn write_reg<S>(&mut self, name: S, val: &BitVec) -> Result<(), ContextError>
    where
        S: AsRef<str>
    {
        let translator = self.lifter.translator();
        if let Some(reg) = translator.register_by_name(name) {
            self.regs
                .write_val_with(reg.offset() as usize, val, self.endian)
                .map_err(ContextError::state)
        } else {
            Err(ContextError::state_with("register does not exist"))
        }
    }
}

impl<'a> EvaluatorContext for ContextManager<'a> {
    
    fn read_vnd(
        &mut self, 
        var: &VarnodeData
    ) -> Result<BitVec, EvaluatorError> {
        let spc = var.space();
        if spc.is_constant() {
            Ok(BitVec::from_u64(var.offset(), var.bits()))
        } else if spc.is_register() {
            self.regs
                .read_val_with(var.offset() as usize, var.size(), self.endian)
                .map_err(EvaluatorError::state)
        } else if  spc.is_unique() {
            self.tmps
                .read_val_with(var.offset() as usize, var.size(), self.endian)
                .map_err(EvaluatorError::state)
        } else if spc.is_default() {
            let addr = var.offset();
            let context = self.memory_map
                .get_mut_context_at(addr)?;
            context.read_vnd(&var)
        } else {
            panic!("[ContextManager::read_vnd]: invalid space type: {:?}", spc)
        }
    }

    fn write_vnd(
        &mut self, 
        var: &VarnodeData, 
        val: &BitVec
    ) -> Result<(), EvaluatorError> {
        let spc = var.space();
        if spc.is_constant() {
            panic!("[ContextManager::write_vnd]: cannot write to constant varnode")
        } else if spc.is_register() {
            self.regs
                .write_val_with(var.offset() as usize, val, self.endian)
                .map_err(EvaluatorError::state)
        } else if spc.is_unique() {
            self.tmps
                .write_val_with(var.offset() as usize, val, self.endian)
                .map_err(EvaluatorError::state)
        } else if spc.is_default() {
            let addr = var.offset();
            let context = self.memory_map
                .get_mut_context_at(addr)?;
            context.write_vnd(&var, val)
        } else {
            panic!("[ContextManager::write_vnd]]: invalid space type: {:?}", spc)
        }
    }
}

impl<'a> fmt::Debug for ContextManager<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let t = self.lifter.translator();
        write!(f, "ContextManager[{}]", t.architecture())
    }
}