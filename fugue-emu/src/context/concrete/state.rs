//! concrete states
//! 
//! various state backings for the concrete context

use std::sync::Arc;
use nohash_hasher::IntMap;
use iset::IntervalMap;

use fugue_ir::{ 
    Address, Translator, VarnodeData, 
    register::RegisterNames, 
    space::AddressSpaceId 
};
use fugue_bv::BitVec;
use fugue_bytes::Endian;
use fugue_core::eval::fixed_state::FixedState;

use crate::context;
use crate::context::traits::{
    VarnodeContext,
    MemoryMapContext,
    MappedContext,
    RegisterContext,
    UniqueContext,
};
use crate::peripheral::traits::MappedPeripheralState;



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

    /// read register from varnode without exclusive reference
    pub fn read_reg_by_vnd(&self, var: &VarnodeData) -> Result<BitVec, context::Error> {
        if var.space() != self.spaceid {
            return Err(context::Error::Unexpected(
                format!{"register space id mismatch: {:?} expected {:?}", var.space(), self.spaceid}))
        }
        self.inner.read_val_with(var.offset() as usize, var.size(), self.endian)
            .map_err(context::Error::from)
    }
}

impl VarnodeContext<BitVec> for ConcreteRegisters {

    fn read_vnd(&mut self, var: &VarnodeData) -> Result<BitVec, context::Error> {
        self.read_reg_by_vnd(var)
    }

    fn write_vnd(&mut self, var: &VarnodeData, val: &BitVec) -> Result<(), context::Error> {
        if var.space() != self.spaceid {
            return Err(context::Error::Unexpected(
                format!{"register space id mismatch: {:?} expected {:?}", var.space(), self.spaceid}))
        }
        self.inner.write_val_with(var.offset() as usize, val, self.endian)
            .map_err(context::Error::from)
    }
}

impl RegisterContext<BitVec> for ConcreteRegisters {

    fn read_reg(&self, name: &str) -> Result<BitVec, context::Error> {
        let (_, offset, size) = self.reg_names
            .get_by_name(name)
            .ok_or(context::Error::InvalidRegister(String::from(name)))?;
        self.inner.read_val_with(offset as usize, size, self.endian)
            .map_err(context::Error::from)
    }

    fn write_reg(&mut self, name: &str, data: &BitVec) -> Result<(), context::Error> {
        println!("write_reg(name: {}, val: {} ", name, data);
        let (_, offset, _size) = self.reg_names
            .get_by_name(name)
            .ok_or(context::Error::InvalidRegister(String::from(name)))?;
        self.inner.write_val_with(offset as usize, data, self.endian)
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

impl VarnodeContext<BitVec> for ConcreteTemps {

    fn read_vnd(&mut self, var: &VarnodeData) -> Result<BitVec, context::Error> {
        if var.space() != self.spaceid {
            return Err(context::Error::Unexpected(
                format!{"unique space id mismatch: {:?} expected {:?}", var.space(), self.spaceid}))
        }
        self.inner.read_val_with(var.offset() as usize, var.size(), self.endian)
            .map_err(context::Error::from)
    }

    fn write_vnd(&mut self, var: &VarnodeData, val: &BitVec) -> Result<(), context::Error> {
        if var.space() != self.spaceid {
            return Err(context::Error::Unexpected(
                format!{"unique space id mismatch: {:?} expected {:?}", var.space(), self.spaceid}))
        }
        self.inner.write_val_with(var.offset() as usize, val, self.endian)
            .map_err(context::Error::from)
    }
}

impl UniqueContext<BitVec> for ConcreteTemps { }

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
    endian: Endian,
    map: IntMap<u64, MapIx>,
    segments: IntervalMap<Address, MapIx>,
    mem: Vec<ConcreteState>,
    mmio: Vec<MappedConcretePeripheral>,
}

impl ConcreteMemoryMap {

    /// creates a new empty ConcreteMemoryMap
    pub fn new_with(translator: &Translator) -> Self {
        Self {
            endian: if translator.is_big_endian() {
                Endian::Big
            } else {
                Endian::Little
            },
            map: IntMap::default(),
            segments: IntervalMap::new(),
            mem: Vec::new(),
            mmio: Vec::new(),
        }
    }

    /// utility for getting exclusive reference to a mapped context
    pub fn get_mut_context_at(
        &mut self,
        address: impl AsRef<Address>,
    ) -> Result<&mut dyn MappedContext<BitVec>, context::Error> {
        let addr = address.as_ref();
        let align = addr.offset() & !0xFFFu64;
        let idx = self.map.get(&align)
            .ok_or(context::Error::Unmapped(addr.clone()))?;
        match idx {
            MapIx::MEM(i) => Ok(self.mem.get_mut(*i).unwrap()),
            MapIx::MMIO(i) => Ok(self.mmio.get_mut(*i).unwrap()),
        }
    }

    /// utility for getting shared reference to a mapped context
    pub fn get_context_at(
        &self,
        address: impl AsRef<Address>,
    ) -> Result<& dyn MappedContext<BitVec>, context::Error> {
        let addr = address.as_ref();
        let align = addr.offset() & !0xFFFu64;
        let idx = self.map.get(&align)
            .ok_or(context::Error::Unmapped(addr.clone()))?;
        match idx {
            MapIx::MEM(i) => Ok(self.mem.get(*i).unwrap()),
            MapIx::MMIO(i) => Ok(self.mmio.get(*i).unwrap()),
        }
    }

}

impl VarnodeContext<BitVec> for ConcreteMemoryMap {

    fn read_vnd(&mut self, var: &VarnodeData) -> Result<BitVec, context::Error> {
        let address = Address::from(var.offset());
        self.read_mem(&address, var.size())
    }

    fn write_vnd(&mut self, var: &VarnodeData, val: &BitVec) -> Result<(), context::Error> {
        let address = Address::from(var.offset());
        self.write_mem(&address, val)
    }
}

impl MemoryMapContext<BitVec> for ConcreteMemoryMap {

    fn map_mem(
        &mut self,
        base: impl Into<Address>,
        size: usize,
    ) -> Result<(), context::Error> {
        let base_address = base.into();

        // check for alignment problems
        if base_address.offset() % Self::ALIGNMENT_SIZE != 0 {
            return Err(context::Error::UnalignedAddress(base_address.clone()));
        }
        if size % Self::ALIGNMENT_SIZE as usize != 0 {
            return Err(context::Error::UnalignedSize(size, Self::ALIGNMENT_SIZE as usize));
        }

        // check for collision with existing mapped contexts
        // this is pretty much the only thing segments is used for as
        // accesses should be much faster using nohash lookup method
        let range = base_address..base_address + size;
        if let Some(std::ops::Range { start, .. }) = self.segments.intervals(range.clone()).next() {
            return Err(context::Error::MapConflict(base_address, start));
        }

        // add concrete memory to memory map
        let mem = ConcreteState::new_with(base_address.clone(), size);
        self.mem.push(mem);
        let idx = MapIx::MEM(self.mem.len() - 1);
        self.segments.insert(range, idx);
        self.map.insert(base_address.offset(), idx);
        let mut addr_alias = base_address + Self::ALIGNMENT_SIZE;
        while addr_alias < base_address + size {
            // create aliases for all 0x1000-aligned addresses
            // mapped regions must have contiguous 0x1000-aligned keys
            self.map.insert(addr_alias.offset(), idx);
            addr_alias += Self::ALIGNMENT_SIZE;
        }

        Ok(())
    }

    fn map_mmio(
        &mut self,
        base: impl Into<Address>,
        peripheral: Box<dyn MappedPeripheralState>,
    ) -> Result<(), context::Error> {
        let base_address = base.into();
        let size = peripheral.size();

        // check for collision with existing mapped contexts
        let range = base_address..base_address + size;
        for std::ops::Range { start, .. } in self.segments.intervals(range.clone()) {
            return Err(context::Error::MapConflict(base_address, start));
        }

        // add peripheral to memory map
        self.mmio.push(MappedConcretePeripheral(peripheral));
        let idx = MapIx::MMIO(self.mmio.len() - 1);
        self.segments.insert(range, idx);
        self.map.insert(base_address.offset(), idx);
        let mut addr_alias = base_address + Self::ALIGNMENT_SIZE;
        while addr_alias < base_address + size {
            // create aliases for all 0x1000-aligned addresses
            // mapped regions must have contiguous 0x1000-aligned keys
            self.map.insert(addr_alias.offset(), idx);
            addr_alias += Self::ALIGNMENT_SIZE;
        }

        Ok(())
    }

    /// read a slice of bytes from memory at specified address
    fn read_bytes(
        &self,
        address: impl AsRef<Address>,
        size: usize
    ) -> Result<&[u8], context::Error> {
        let address = address.as_ref();
        let context = self.get_context_at(address)?;
        context.read_bytes(address, size)
    }

    /// write bytes to memory at specified address
    fn write_bytes(&mut self, address: impl AsRef<Address>, bytes: &[u8]) -> Result<(), context::Error> {
        let address = address.as_ref();
        let context = self.get_mut_context_at(address)?;
        context.write_bytes(address, bytes)
    }

    /// read data from memory at specified address
    fn read_mem(&self, address: impl AsRef<Address>, size: usize) -> Result<BitVec, context::Error> {
        let address = address.as_ref();
        let endian = self.endian.clone();
        let context = self.get_context_at(address)?;
        context.read_mem(address, size, endian)
    }

    /// write data to memory at specified address
    fn write_mem(&mut self, address: impl AsRef<Address>, data: &BitVec) -> Result<(), context::Error> {
        let address = address.as_ref();
        let endian  = self.endian.clone();
        let context = self.get_mut_context_at(address)?;
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

impl MappedContext<BitVec> for ConcreteState {

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

    fn read_mem(&self, address: &Address, size: usize, endian: fugue_bytes::Endian) -> Result<BitVec, context::Error> {
        self.inner.read_val_with(self.offset(address)? as usize, size, endian)
            .map_err(context::Error::from)
    }

    fn write_mem(&mut self, address: &Address, data: &BitVec, endian: fugue_bytes::Endian) -> Result<(), context::Error> {
        self.inner.write_val_with(self.offset(address)? as usize, data, endian)
            .map_err(context::Error::from)
    }
}

/// a memory mapped peripheral implementation for concrete evaluation
/// 
/// since peripheral state has the same data type for the concrete
/// evaluator already, we can just wrap it in a box
#[derive(Clone)]
pub struct MappedConcretePeripheral(Box<dyn MappedPeripheralState>);

impl MappedContext<BitVec> for MappedConcretePeripheral {

    fn base(&self) -> Address {
        MappedPeripheralState::base(self.0.as_ref())
    }

    fn size(&self) -> usize {
        MappedPeripheralState::size(self.0.as_ref())
    }

    fn read_bytes(&self, address: &Address, size: usize) -> Result<&[u8], context::Error> {
        MappedPeripheralState::read_bytes(self.0.as_ref(), address, size)
            .map_err(context::Error::from)
    }

    fn read_mem(&self, address: &Address, size: usize, endian: Endian) -> Result<BitVec, context::Error> {
        MappedPeripheralState::read_mem(self.0.as_ref(), address, size, endian)
            .map_err(context::Error::from)
    }

    fn write_bytes(&mut self, address: &Address, bytes: &[u8]) -> Result<(), context::Error> {
        MappedPeripheralState::write_bytes(self.0.as_mut(), address, bytes)
            .map_err(context::Error::from)
    }

    fn write_mem(&mut self, address: &Address, data: &BitVec, endian: Endian) -> Result<(), context::Error> {
        MappedPeripheralState::write_mem(self.0.as_mut(), address, data, endian)
            .map_err(context::Error::from)
    }
}