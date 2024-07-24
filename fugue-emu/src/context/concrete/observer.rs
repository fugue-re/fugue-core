//! concrete observers
//! 
//! generic observer implementations for concrete contexts

use crate::context::AccessType;
use crate::context::traits::observer::*;

/// mem access logging observer
#[derive(Clone)]
pub struct MemAccessLogger;

impl MemAccessLogger {
    /// return a new boxed instance of MemAccessLogger
    pub fn new_boxed() -> Box<Self> {
        Box::new(MemAccessLogger)
    }
}

impl MemObserver for MemAccessLogger {
    
    fn update<'c>(
        &self,
        address: &fugue_ir::Address,
        val: &fugue_bv::BitVec,
        access: AccessType,
    ) -> Result<(), crate::context::Error> {
        let access_str = match access {
            AccessType::R => { "read" }
            AccessType::W => { "write" }
            AccessType::X => { "fetch" }
            _ => { panic!("invalid access type passed to MemAccessLogger") }
        };
        println!("{} @ 0x{:08x}: {}", access_str, address.offset(), val);
        Ok(())
    }
}

/// reg access logging observer
#[derive(Clone)]
pub struct RegAccessLogger;

impl RegAccessLogger {
    /// return a new boxed instance of RegAccessLogger
    pub fn new_boxed() -> Box<Self> {
        Box::new(RegAccessLogger)
    }
}

impl RegObserver for RegAccessLogger {

    fn update(
        &self,
        name: &str,
        offset: u64,
        size: usize,
        value: &fugue_bv::BitVec,
        access: crate::context::AccessType,
    ) -> Result<(), crate::context::Error> {
        let access_str = match access {
            AccessType::R => { "read" }
            AccessType::W => { "write" }
            _ => { panic!("invalid access type passed to RegAccessLogger") }
        };
        println!("{} register {}: {}", access_str, name, value);
        Ok(())
    }
}