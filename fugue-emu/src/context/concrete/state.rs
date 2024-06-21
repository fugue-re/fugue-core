//! concrete states
//! 
//! various state backings for the concrete context

use std::collections::HashMap;
use std::sync::Arc;
use nohash_hasher::IntMap;
use iset::IntervalMap;
use ustr::Ustr;

use fugue_ir::{ 
    Address, Translator, VarnodeData, 
    register::RegisterNames, 
    space::AddressSpaceId 
};
use fugue_bv::BitVec;
use fugue_bytes::Endian;
use fugue_core::eval::fixed_state::{ FixedState, FixedStateError };

use crate::context;
use crate::context::traits::{MappedContext, RegisterContext, UniqueContext};
use crate::peripheral::traits::MappedPeripheralState;
use crate::eval::traits::EvaluatorContext;

use super::ALIGNMENT_SIZE;



/// concrete register context
/// 
/// a arch-specific wrapper for FixedState that facilitates access by register name
#[derive(Clone)]
pub struct ConcreteRegisters {
    reg_names: Arc<RegisterNames>,
    spaceid: AddressSpaceId,
    endian: Endian,
    inner: FixedState,
}

impl ConcreteRegisters {

    /// creates a new register context from an architecture's translator
    pub fn new_with(translator: &Translator) -> Self {
        Self {
            reg_names: translator.registers().clone(),
            spaceid: translator.manager().register_space_id(),
            endian: if translator.is_big_endian() { Endian::Big } else { Endian::Little },
            inner: FixedState::new(translator.register_space_size()),
        }
    }
}

impl RegisterContext for ConcreteRegisters {
    type Data=BitVec;

    fn read_reg(&self, name: &str) -> Result<Self::Data, context::Error> {
        let (_, offset, size) = self.reg_names
            .get_by_name(name)
            .ok_or(context::Error::InvalidRegisterName(String::from(name)))?;
        self.inner.read_val_with(offset as usize, size, self.endian)
            .map_err(context::Error::from)
    }

    fn write_reg(&mut self, name: &str, data: &Self::Data) -> Result<(), context::Error> {
        let (_, offset, size) = self.reg_names
            .get_by_name(name)
            .ok_or(context::Error::InvalidRegisterName(String::from(name)))?;
        self.inner.write_val_with(offset as usize, data, self.endian)
            .map_err(context::Error::from)
    }

    fn read_vnd(&self, var: &VarnodeData) -> Result<Self::Data, context::Error> {
        if var.space() != self.spaceid {
            return Err(context::Error::Unexpected(
                format!{"register space id mismatch: {:?} expected {:?}", var.space(), self.spaceid}))
        }
        self.inner.read_val_with(var.offset() as usize, var.size(), self.endian)
            .map_err(context::Error::from)
    }

    fn write_vnd(&mut self, var: &VarnodeData, val: &Self::Data) -> Result<(), context::Error> {
        if var.space() != self.spaceid {
            return Err(context::Error::Unexpected(
                format!{"register space id mismatch: {:?} expected {:?}", var.space(), self.spaceid}))
        }
        self.inner.write_val_with(var.offset() as usize, val, self.endian)
            .map_err(context::Error::from)
    }
}

/// concrete context for pcode temporaries
/// 
/// a arch-specific wrapper for FixedState
#[derive(Clone)]
pub struct ConcreteTemps {
    spaceid: AddressSpaceId,
    endian: Endian,
    inner: FixedState,
}

impl ConcreteTemps {

    /// creates a new concrete context for a given architecture's translator
    pub fn new_with(translator: &Translator) -> Self {
        Self {
            spaceid: translator.manager().unique_space_id(),
            endian: if translator.is_big_endian() { Endian::Big } else { Endian::Little },
            inner: FixedState::new(translator.unique_space_size()),
        }
    }
}

impl UniqueContext for ConcreteTemps {
    type Data = BitVec;

    fn read_vnd(&self, var: &VarnodeData) -> Result<Self::Data, context::Error> {
        if var.space() != self.spaceid {
            return Err(context::Error::Unexpected(
                format!{"unique space id mismatch: {:?} expected {:?}", var.space(), self.spaceid}))
        }
        self.inner.read_val_with(var.offset() as usize, var.size(), self.endian)
            .map_err(context::Error::from)
    }

    fn write_vnd(&mut self, var: &VarnodeData, val: &Self::Data) -> Result<(), context::Error> {
        if var.space() != self.spaceid {
            return Err(context::Error::Unexpected(
                format!{"unique space id mismatch: {:?} expected {:?}", var.space(), self.spaceid}))
        }
        self.inner.write_val_with(var.offset() as usize, val, self.endian)
            .map_err(context::Error::from)
    }
}

/// an index type to distinguish between mapped memory
/// versus mapped peripherals
#[derive(Debug, Clone, Copy)]
enum MapIx {
    MEM(usize),
    MMIO(usize),
}

/// concrete memory map context
/// 
/// for a concrete context, the memory map should simply enable access
/// to segmented memory regions and potentially peripherals,
/// all of which should implement the MappedContext trait for BitVec
#[derive(Clone)]
pub struct ConcreteMemoryMap {
    map: IntMap<u64, MapIx>,
    segments: IntervalMap<Address, MapIx>,
    mem: Vec<ConcreteState>,
    mmio: Vec<MappedConcretePeripheral>,
}

impl ConcreteMemoryMap {

    /// creates a new empty ConcreteMemoryMap
    pub fn new() -> Self {
        Self {
            map: IntMap::default(),
            segments: IntervalMap::new(),
            mem: Vec::new(),
            mmio: Vec::new(),
        }
    }

    /// add a new context that implements the mapped context trait to the memory map
    pub fn map_mem(
        &mut self,
        base: impl Into<Address>,
        size: usize,
    ) -> Result<(), context::Error> {
        let base_address = base.into();

        // check for collision with existing mapped contexts
        // this is pretty much the only thing segments is used for as
        // accesses should be much faster using nohash lookup method
        let range = base_address..base_address + size;
        for std::ops::Range { start, .. } in self.segments.intervals(range.clone()) {
            return Err(context::Error::MapConflict(base_address, start));
        }

        // add concrete memory to memory map
        let mem = ConcreteState::new_with(base_address.clone(), size);
        self.mem.push(mem);
        let idx = MapIx::MEM(self.mem.len() - 1);
        self.segments.insert(range, idx);
        self.map.insert(base_address.offset(), idx);
        let mut addr_alias = base_address + ALIGNMENT_SIZE;
        while addr_alias < base_address + size {
            // create aliases for all 0x1000-aligned addresses
            // mapped regions must have contiguous 0x1000-aligned keys
            self.map.insert(addr_alias.offset(), idx);
            addr_alias += ALIGNMENT_SIZE;
        }

        Ok(())
    }

    pub fn map_mmio(
        &mut self,
        base: impl Into<Address>,
        peripheral: MappedConcretePeripheral,
    ) -> Result<(), context::Error> {
        let base_address = base.into();
        let size = peripheral.size();

        // check for collision with existing mapped contexts
        let range = base_address..base_address + size;
        for std::ops::Range { start, .. } in self.segments.intervals(range.clone()) {
            return Err(context::Error::MapConflict(base_address, start));
        }

        // add peripheral to memory map
        self.mmio.push(peripheral);
        let idx = MapIx::MMIO(self.mmio.len() - 1);
        self.segments.insert(range, idx);
        self.map.insert(base_address.offset(), idx);
        let mut addr_alias = base_address + ALIGNMENT_SIZE;
        while addr_alias < base_address + size {
            // create aliases for all 0x1000-aligned addresses
            // mapped regions must have contiguous 0x1000-aligned keys
            self.map.insert(addr_alias.offset(), idx);
            addr_alias += ALIGNMENT_SIZE;
        }

        Ok(())
    }

    /// utility for getting exclusive reference to a mapped context
    pub fn get_mut_context_at(
        &mut self,
        address: impl Into<Address>,
    ) -> Result<&mut dyn MappedContext<Data=BitVec>, context::Error> {
        let addr = address.into();
        let align = addr.offset() & !0xFFFu64;
        let idx = self.map.get(&align)
            .ok_or(context::Error::Unmapped(addr))?;
        match idx {
            MapIx::MEM(i) => Ok(self.mem.get_mut(*i).unwrap()),
            MapIx::MMIO(i) => Ok(self.mmio.get_mut(*i).unwrap()),
        }
    }

    /// utility for getting shared reference to a mapped context
    pub fn get_context_at(
        &self,
        address: impl Into<Address>,
    ) -> Result<& dyn MappedContext<Data=BitVec>, context::Error> {
        let addr = address.into();
        let align = addr.offset() & !0xFFFu64;
        let idx = self.map.get(&align)
            .ok_or(context::Error::Unmapped(addr))?;
        match idx {
            MapIx::MEM(i) => Ok(self.mem.get(*i).unwrap()),
            MapIx::MMIO(i) => Ok(self.mmio.get(*i).unwrap()),
        }
    }

    /// read a slice of bytes from memory at specified address
    pub fn read_bytes(
        &self,
        address: &Address,
        size: usize
    ) -> Result<&[u8], context::Error> {
        let context = self.get_context_at(address.clone())?;
        context.read_bytes(address, size)
    }

    /// write bytes to memory at specified address
    pub fn write_bytes(&mut self, address: &Address, bytes: &[u8]) -> Result<(), context::Error> {
        let context = self.get_mut_context_at(address.clone())?;
        context.write_bytes(address, bytes)
    }

    /// read data from memory at specified address
    pub fn read_mem(&self, address: &Address, size: usize, endian: Endian) -> Result<BitVec, context::Error> {
        let context = self.get_context_at(address.clone())?;
        context.read_mem(address, size, endian)
    }

    /// write data to memory at specified address
    pub fn write_mem(&mut self, address: &Address, data: &BitVec, endian: Endian) -> Result<(), context::Error> {
        let context = self.get_mut_context_at(address.clone())?;
        context.write_mem(address, data, endian)
    }

}

/// concrete mapped context
/// 
/// a wrapper for FixedState that keeps the base address
/// for use as main memory regions in ConcreteMemoryMap
#[derive(Clone)]
pub struct ConcreteState {
    base: Address,
    inner: FixedState,
}

impl ConcreteState {
    pub fn new_with(base: Address, size: usize) -> Self {
        Self {
            base,
            inner: FixedState::new(size),
        }
    }

    /// a utility to translate given address to an offset relative to 
    /// the concrete state's base address
    pub fn offset(&self, address: &Address) -> Result<u64, context::Error> {
        let offset = address.offset().wrapping_sub(self.base.offset());
        if offset as usize > self.inner.len() {
            return Err(context::Error::OutOfBounds(address.clone()))
        }
        Ok(offset)
    }
}

impl MappedContext for ConcreteState {
    type Data = BitVec;

    fn base(&self) -> Address {
        self.base.clone()
    }

    fn size(&self) -> usize {
        self.inner.len()
    }

    fn read_bytes(&self, address: &Address, size: usize) -> Result<&[u8], context::Error> {
        self.inner.view_bytes(self.offset(address)? as usize, size)
            .map_err(context::Error::from)
    }

    fn write_bytes(&mut self, address: &Address, bytes: &[u8]) -> Result<(), context::Error> {
        self.inner.write_bytes(self.offset(address)? as usize, bytes)
            .map_err(context::Error::from)
    }

    fn read_mem(&self, address: &Address, size: usize, endian: fugue_bytes::Endian) -> Result<Self::Data, context::Error> {
        self.inner.read_val_with(self.offset(address)? as usize, size, endian)
            .map_err(context::Error::from)
    }

    fn write_mem(&mut self, address: &Address, data: &Self::Data, endian: fugue_bytes::Endian) -> Result<(), context::Error> {
        self.inner.write_val_with(self.offset(address)? as usize, data, endian)
            .map_err(context::Error::from)
    }
}

impl From<FixedStateError> for context::Error {
    fn from(value: FixedStateError) -> Self {
        Self::State(format!("{:?}", value))
    }
}

/// a memory mapped peripheral implementation for concrete evaluation
/// 
/// since peripheral state has the same data type for the concrete
/// evaluator already, we can just wrap it in a box
type MappedConcretePeripheral = Box<dyn MappedPeripheralState>;

impl MappedContext for MappedConcretePeripheral {
    type Data = BitVec;

    fn base(&self) -> Address {
        MappedPeripheralState::base(self.as_ref())
    }

    fn size(&self) -> usize {
        MappedPeripheralState::size(self.as_ref())
    }

    fn read_bytes(&self, address: &Address, size: usize) -> Result<&[u8], context::Error> {
        MappedPeripheralState::read_bytes(self.as_ref(), address, size)
            .map_err(context::Error::from)
    }

    fn read_mem(&self, address: &Address, size: usize, endian: Endian) -> Result<Self::Data, context::Error> {
        MappedPeripheralState::read_mem(self.as_ref(), address, size, endian)
            .map_err(context::Error::from)
    }

    fn write_bytes(&mut self, address: &Address, bytes: &[u8]) -> Result<(), context::Error> {
        MappedPeripheralState::write_bytes(self.as_mut(), address, bytes)
            .map_err(context::Error::from)
    }

    fn write_mem(&mut self, address: &Address, data: &Self::Data, endian: Endian) -> Result<(), context::Error> {
        MappedPeripheralState::write_mem(self.as_mut(), address, data, endian)
            .map_err(context::Error::from)
    }
}