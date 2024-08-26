//! generic gpio peripheral backend
//! 
//! a generic backend for memory-mapped gpio

use fugue_ir::Address;
use fugue_bv::BitVec;
use fugue_bytes::Endian;
use fugue_core::eval::fixed_state::FixedState;

use crate::emu::traits::Clocked;

use crate::peripheral;
use crate::peripheral::traits::MappedPeripheralState;


#[derive(Clone)]
pub struct GPIOPortPeripheral {
    base: Address,
}