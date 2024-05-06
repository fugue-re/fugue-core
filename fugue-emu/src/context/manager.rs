//! context manager module
#![allow(unused_imports)]

use std::fmt;
// todo: reimplement without multikeymap
// multikeymap uses 2 hashmaps, we only really need one and a vector
// which might give us performance improvement.
use multi_key_map::MultiKeyMap;

use fugue::bv::BitVec;
use fugue::bytes::Endian;
use fugue::ir::{
    translator, Address, VarnodeData
};
use fugue::high::{
    language::Language,
    lifter::Lifter,
    eval::{
        fixed_state::FixedState,
        EvaluatorError,
        EvaluatorContext,
    },
};

use crate::context::{
    ContextType,
    MappedContext,
    ContextError,
};

use super::concrete::ConcreteMemory;

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
    memory_map: MultiKeyMap<Address, Box<dyn MappedContext>>,
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
    ) -> Self {
        // let lifter = lang.lifter();
        let t = lifter.translator();
        let endian = if t.is_big_endian() { 
            Endian::Big 
        } else { 
            Endian::Little 
        };
        let memory_map = MultiKeyMap::new();
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

        // check for collision with existing mapped contexts
        // performs better with few, large mapped contexts
        for context in self.memory_map.values() {
            let context_lbound = context.base();
            let context_ubound = context.base() + context.size();
            // check for context overlap
            if base_address < context_ubound 
                    && base_address + size > context_lbound {
                return Err(ContextError::MapConflict(base_address, context_lbound))
            }
        }

        let context = match context_type {
            Some(ContextType::Concrete) | None => {
                ConcreteMemory::new(Address::from(base_address), self.endian, size)
            }
            // for additional future memory types
        };

        // add memory to memory map
        self.memory_map.insert(base_address, Box::new(context));
        let mut addr_alias = base_address + 0x1000u64;
        while addr_alias < base_address + size {
            // must create aliases for 0x1000-aligned addresses
            // context manager relies on contiguous 0x1000-aligned keys for 
            // mapped regions, so we should panic if we fail to create one. 
            if let Err(alias) = self.memory_map.alias(&base_address, addr_alias) {
                panic!("failed to create address alias: {:?}", alias);
            }
            addr_alias += 0x1000u64;
        }

        Ok(self)
    }

    /// utility for mutably borrowing memory structs
    #[inline]
    pub fn get_mut_context_at(
        &mut self, 
        address: impl Into<Address>
    ) -> Result<&mut Box<dyn MappedContext>, ContextError> {
        let addr = u64::from(address.into());
        let align = Address::from(addr & !0xFFFu64);
        self.memory_map.get_mut(&align)
            .ok_or(ContextError::Unmapped(addr.into()))
    }

    /// utility for immmutably borrowing memory structs
    #[inline]
    pub fn get_context_at(
        &self,
        address: impl Into<Address>
    ) -> Result<&Box<dyn MappedContext>, ContextError> {
        let addr = u64::from(address.into());
        let align = Address::from(addr & !0xFFFu64);
        self.memory_map.get(&align)
            .ok_or(ContextError::Unmapped(addr.into()))
    }

    /// returns a vector of bytes
    pub fn read_bytes(
        &self, 
        address: Address,
        size: usize
    ) -> Result<Vec<u8>, ContextError> {
        let context = self
            .get_context_at(address)
            .map_err(ContextError::state)?;
        context.read_bytes(address, size)
    }

    /// write bytes to context
    pub fn write_bytes(
        &mut self,
        address: Address,
        values: &[u8],
    ) -> Result<(), ContextError> {
        let context = self
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
            Ok(BitVec::from_u64(var.offset(), var.size() * 8))
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
            let context = self
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
            let context = self
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