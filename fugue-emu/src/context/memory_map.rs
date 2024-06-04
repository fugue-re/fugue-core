//! memory_map module
//! 
//! implements MemoryMap to manage access to mapped contexts

// todo: reimplement without multikeymap
// multikeymap uses 2 hashmaps, we only really need one and a vector
// which might give us performance improvement.
use std::collections::HashMap;

use fugue_ir::Address;
use crate::context::{
    MappedContext,
    ContextError,
};

/// MemoryMap
pub struct MemoryMap {
    map: HashMap<Address, usize>,
    contexts: Vec<Box<dyn MappedContext>>,
}

impl MemoryMap {

    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            contexts: Vec::new(),
        }
    }

    /// add a new context that implements the MappedContext trait to the 
    /// memory map
    pub fn map_context(
        &mut self,
        context: Box<dyn MappedContext>,
    ) -> Result<(), ContextError> {
        let base_address = context.base();
        let size = context.size();

        // check for collision with existing mapped contexts
        // better for few, large mapped contexts, eventually want to use
        // an interval tree or segment tree to do this, but this will
        // be ok for few, large mapped contexts for now.
        // it's also an init step for now, so it won't happen often
        for mapped_context in self.contexts.iter() {
            let lbound = mapped_context.base();
            let ubound = lbound + mapped_context.size();
            // check for context overlap
            if base_address < ubound && base_address + size > lbound {
                return Err(ContextError::MapConflict(base_address, lbound));
            }
        }

        // add context to memory map
        self.contexts.push(context);
        let idx = self.contexts.len() - 1;
        self.map.insert(base_address, idx);
        let mut addr_alias = base_address + 0x1000u64;
        while addr_alias < base_address + size {
            // create aliases for all 0x1000-aligned addresses
            // mapped regions must have contiguous 0x1000-aligned keys
            self.map.insert(addr_alias, idx);
            addr_alias += 0x1000u64;
        }

        Ok(())
    }

    /// utility for mutably borrowing memory structs
    #[inline]
    pub fn get_mut_context_at(
        &mut self, 
        address: impl Into<Address>
    ) -> Result<&mut Box<dyn MappedContext>, ContextError> {
        let addr = u64::from(address.into());
        let align = Address::from(addr & !0xFFFu64);
        let idx = self.map.get(&align)
            .ok_or(ContextError::Unmapped(addr.into()))?;
        Ok(self.contexts.get_mut(*idx).unwrap())
    }

    /// utility for immmutably borrowing memory structs
    #[inline]
    pub fn get_context_at(
        &self,
        address: impl Into<Address>
    ) -> Result<&Box<dyn MappedContext>, ContextError> {
        let addr = u64::from(address.into());
        let align = Address::from(addr & !0xFFFu64);
        let idx = self.map.get(&align)
            .ok_or(ContextError::Unmapped(addr.into()))?;
        Ok(self.contexts.get(*idx).unwrap())
    }

    /// returns a slice of bytes starting from the given address
    pub fn read_bytes_slice(
        &self, 
        address: Address,
        size: usize
    ) -> Result<&[u8], ContextError> {
        let context = self
            .get_context_at(address)?;
        context.read_bytes_slice(address, size)
    }

    pub fn read_bytes(
        &self,
        address: Address,
        size: usize,
    ) -> Result<Vec<u8>, ContextError> {
        self.read_bytes_slice(address, size)
            .map(Vec::from)
    }
}

