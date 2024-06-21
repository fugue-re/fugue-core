//! peripheral traits
//! 
//! defines various traits for the peripheral module
use dyn_clone::{ DynClone, clone_trait_object };

use fugue_ir::Address;
use fugue_bytes::Endian;
use fugue_bv::BitVec;

use crate::emu::traits::Clocked;
use crate::peripheral;
// use crate::context::traits::MappedContext;

/// memory-mapped peripheral state trait
/// 
/// note that by default, memory mapped peripherals should expose BitVec
/// data on read/write
/// 
/// if an evaluator deals with a different data variation, the corresponding
/// peripheral trait for that evaluator type should be implemented as an
/// observable translation layer between BitVec and the associated 
/// evaluator data type.
/// 
/// internally, an implementation of an emulator type (e.g. Concrete) must be
/// able to clone a peripheral's state for deterministic backup and restore.
/// DynClone is necessary to get around the inability implement clone for
/// trait objects.
pub trait MappedPeripheralState: DynClone + Clocked {

    /// return the base address of the mapped context
    fn base(&self) -> Address;

    /// return the memory region size of the mapped context
    fn size(&self) -> usize;

    /// read a slice of bytes from memory at specified address
    fn read_bytes(&self, address: &Address, size: usize) -> Result<&[u8], peripheral::Error>;

    /// write bytes to memory at specified address
    fn write_bytes(&mut self, address: &Address, bytes: &[u8]) -> Result<(), peripheral::Error>;

    /// read data from memory at specified address
    fn read_mem(&self, address: &Address, size: usize, endian: Endian) -> Result<BitVec, peripheral::Error>;

    /// write data to memory at specified address
    fn write_mem(&mut self, address: &Address, data: &BitVec, endian: Endian) -> Result<(), peripheral::Error>;
}

clone_trait_object!(MappedPeripheralState);