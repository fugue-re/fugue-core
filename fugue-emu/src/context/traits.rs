//! context traits
//! 
//! defines various traits related to emulation contexts


use std::sync::Arc;

use fugue_ir::{ Address, disassembly::PCodeData };
use fugue_core::ir::Location;

use crate::eval::traits::EvaluatorContext;

use crate::context;

/// emulation context trait
/// 
/// this trait defines the high-level
pub trait Context: EvaluatorContext {
    // Self::Data should be inherited from EvaluatorContext::Data

    /// read a slice of bytes from memory at specified address
    fn read_bytes(&self, address: impl Into<Address>, size: usize) -> Result<&[u8], context::Error>;

    /// write bytes to memory at specified address
    fn write_bytes(&mut self, address: impl Into<Address>, bytes: &[u8]) -> Result<(), context::Error>;

    /// read register data (returns cloned data)
    fn read_reg(&self, name: &str) -> Result<Self::Data, context::Error>;

    /// write register with data
    fn write_reg(&mut self, name: &str, data: Self::Data) -> Result<(), context::Error>;

    /// read data from memory at specified address
    fn read_mem(&self, address: impl Into<Address>, size: usize) -> Result<Self::Data, context::Error>;

    /// write data to memory at specified address
    fn write_mem(&mut self, address: impl Into<Address>, data: Self::Data) -> Result<(), context::Error>;

    /// fetch reference to single pcode operation to run
    fn fetch(&mut self, location: impl Into<Location>) -> Result<&PCodeData, context::Error>;

    /// return an iterator that fetches the pcode operations
    /// of the instruction at the specified address and stops
    /// at the end of the current architectural instruction
    fn fetch_iter(&mut self, address: impl Into<Address>) -> Result<impl Iterator<Item=&PCodeData>, context::Error>;
}


// pub trait MemoryContext: EvaluatorContext {

// }


// pub trait MemoryMappedContext: EvaluatorContext {

// }


// pub trait RegisterContext: EvaluatorContext {

// }


// pub trait TemporariesContext: EvaluatorContext {

// }