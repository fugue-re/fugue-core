//! context traits
//! 
//! defines various traits related to emulation contexts


use std::sync::Arc;

use fugue_ir::{ Address, VarnodeData, disassembly::PCodeData };
use fugue_bytes::Endian;
use fugue_core::ir::Location;

use crate::eval::traits::EvaluatorContext;

use crate::context;

/// emulation context trait
/// 
/// this trait defines the high-level interface that a full context should
/// implement.
pub trait Context<'irb>: EvaluatorContext<'irb> + Clone {
    // Self::Data should be inherited from EvaluatorContext::Data

    /// read a slice of bytes from memory at specified address
    fn read_bytes(&self, address: impl AsRef<Address>, size: usize) -> Result<&[u8], context::Error>;

    /// write bytes to memory at specified address
    fn write_bytes(&mut self, address: impl AsRef<Address>, bytes: &[u8]) -> Result<(), context::Error>;

    /// read register data (returns cloned data)
    fn read_reg(&self, name: impl AsRef<str>) -> Result<Self::Data, context::Error>;

    /// write register with data
    fn write_reg(&mut self, name: impl AsRef<str>, data: &Self::Data) -> Result<(), context::Error>;

    /// read data from memory at specified address
    fn read_mem(&self, address: impl AsRef<Address>, size: usize) -> Result<Self::Data, context::Error>;

    /// write data to memory at specified address
    fn write_mem(&mut self, address: impl AsRef<Address>, data: &Self::Data) -> Result<(), context::Error>;

    

    // /// return an iterator that fetches the pcode operations
    // /// of the instruction at the specified address and stops
    // /// at the end of the current architectural instruction
    // fn fetch_iter(&mut self, address: impl Into<Address>) -> Result<impl Iterator<Item=&PCodeData>, context::Error>;
}

/// mapped context trait
/// 
/// any state/context/peripheral that can be mapped into the emulationed
/// device's memory map must implement this trait
/// 
/// the offset parameters are expected to be translated prior to being passed.
/// translation should generally look like converting an Address into a usize,
/// then subtracting the base address of the corresponding context
/// 
/// note that while the mapped context trait does not explicitly declare that
/// an implementation must be cloneable, in practice they must be in order to 
/// enable backup and restore functionality
pub trait MappedContext {
    type Data;

    /// return the base address of the mapped context
    fn base(&self) -> Address;

    /// return the memory region size of the mapped context
    fn size(&self) -> usize;

    /// read a slice of bytes from memory at specified address
    fn read_bytes(&self, address: &Address, size: usize) -> Result<&[u8], context::Error>;

    /// write bytes to memory at specified address
    fn write_bytes(&mut self, address: &Address, bytes: &[u8]) -> Result<(), context::Error>;

    /// read data from memory at specified address
    fn read_mem(&self, address: &Address, size: usize, endian: Endian) -> Result<Self::Data, context::Error>;

    /// write data to memory at specified address
    fn write_mem(&mut self, address: &Address, data: &Self::Data, endian: Endian) -> Result<(), context::Error>;
}

/// register context trait
/// 
/// register states must implement accessibility based on register name
pub trait RegisterContext: Clone {
    type Data;

    /// read register data (returns cloned data)
    fn read_reg(&self, name: &str) -> Result<Self::Data, context::Error>;

    /// write register with data
    fn write_reg(&mut self, name: &str, data: &Self::Data) -> Result<(), context::Error>;

    /// read data at the location of the specified varnode
    fn read_vnd(&self, var: &VarnodeData) -> Result<Self::Data, context::Error>;

    /// write data to the location of the specified varnode
    fn write_vnd(&mut self, var: &VarnodeData, val: &Self::Data) -> Result<(), context::Error>;
}

/// pcode temporaries context trait
/// 
/// must be made accessible via varnode
pub trait UniqueContext: Clone {
    type Data;

    /// read data at the location of the specified varnode
    fn read_vnd(&self, var: &VarnodeData) -> Result<Self::Data, context::Error>;

    /// write data to the location of the specified varnode
    fn write_vnd(&mut self, var: &VarnodeData, val: &Self::Data) -> Result<(), context::Error>;
}

