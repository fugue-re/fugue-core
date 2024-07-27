//! dummy peripheral
//! 
//! a dummy memory-mapped peripheral implementation that basically
//! acts just like concrete memory

use fugue_ir::Address;
use fugue_bv::BitVec;
use fugue_bytes::Endian;
use fugue_core::eval::fixed_state::FixedState;

use crate::sim::traits::Clocked;

use crate::peripheral;
use crate::peripheral::traits::MappedPeripheralState;

#[derive(Clone)]
pub struct DummyPeripheral {
    base: Address,
    backing: FixedState,
}

impl DummyPeripheral {

    pub fn new_with(base: impl Into<Address>, size: usize) -> Self {
        let base = base.into();
        let backing = FixedState::new(size);
        Self { base, backing }
    }

    /// utility to translate given address into offset relative to
    /// peripheral's base address
    /// returns None if given address is out of bounds
    pub fn offset(&self, address: &Address) -> Option<u64> {
        let offset = address.offset().wrapping_sub(self.base.offset());
        if offset as usize > self.backing.len() {
            None
        } else {
            Some(offset)
        }
    }
}

impl Clocked for DummyPeripheral { }

impl MappedPeripheralState for DummyPeripheral {
    
    fn base(&self) -> Address {
        self.base.clone()
    }

    fn size(&self) -> usize {
        self.backing.len()
    }

    fn read_bytes(&self, address: &Address, size: usize) -> Result<&[u8], peripheral::Error> {
        let offset = self.offset(address)
            .ok_or(peripheral::Error::InvalidRead(address.clone(), size))?;
        self.backing.view_bytes(offset as usize, size)
            .map_err(|_err| {
                peripheral::Error::InvalidRead(address.clone(), size)
            })
    }

    fn write_bytes(&mut self, address: &Address, bytes: &[u8]) -> Result<(), peripheral::Error> {
        let offset = self.offset(address)
            .ok_or(peripheral::Error::InvalidWrite(address.clone(), BitVec::from_le_bytes(bytes)))?;
        self.backing.write_bytes(offset as usize, bytes)
            .map_err(|_err| {
                peripheral::Error::InvalidWrite(address.clone(), BitVec::from_le_bytes(bytes))
            })
    }

    fn read_mem(&self, address: &Address, size: usize, endian: Endian) -> Result<BitVec, peripheral::Error> {
        let offset = self.offset(address)
            .ok_or(peripheral::Error::InvalidRead(address.clone(), size))?;
        self.backing.read_val_with(offset as usize, size, endian)
            .map_err(|_err| {
                peripheral::Error::InvalidRead(address.clone(), size)
            })
    }

    fn write_mem(&mut self, address: &Address, data: &BitVec, endian: Endian) -> Result<(), peripheral::Error> {
        let offset = self.offset(address)
            .ok_or(peripheral::Error::InvalidWrite(address.clone(), data.clone()))?;
        self.backing.write_val_with(offset as usize, data, endian)
            .map_err(|_err| {
                peripheral::Error::InvalidWrite(address.clone(), data.clone())
            })
    }
}