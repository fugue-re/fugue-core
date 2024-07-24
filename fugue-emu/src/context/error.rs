//! context errors

use thiserror::Error;

use fugue_ir::{ Address, disassembly::VarnodeData };
use fugue_bv::BitVec;
use fugue_core::eval::fixed_state::FixedStateError;

use crate::peripheral;

#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("lift error: {0}")]
    Lift(String),
    #[error("access to unmapped address {0}")]
    Unmapped(Address),
    #[error("new context at {0} conflicts with context at {1}")]
    MapConflict(Address, Address),
    #[error("address unaligned: {0}")]
    UnalignedAddress(Address),
    #[error("size unaligned: {0:#x}, expected {1:#x}-aligned")]
    UnalignedSize(usize, usize),
    #[error("unexpected error: {0}")]
    Unexpected(String),
    #[error("address {0} out of bounds")]
    OutOfBounds(Address),
    #[error("invalid register {0}")]
    InvalidRegister(String),
    #[error("invalid varnode {0:?}")]
    InvalidVarnode(VarnodeData),
    #[error("could not convert bitvec {0} to type {1}")]
    BitVecConversion(BitVec, &'static str),
    #[error("peripheral error: {0}")]
    Peripheral(String),
    #[error("{0}")]
    State(String),
    #[error("observer error: {0}")]
    Observer(String),
}

// enable coercion from disassembly/lifting errors
impl From<fugue_ir::error::Error> for Error {
    fn from(value: fugue_ir::error::Error) -> Self {
        Self::Lift(format!("{:?}", value))
    }
}

// enable coercion from peripheral errors
impl From<peripheral::Error> for Error {
    fn from(value: peripheral::Error) -> Self {
        Self::Peripheral(format!("{:?}", value))
    }
}

// enable coercion from fixed state errors
impl From<FixedStateError> for Error {
    fn from(value: FixedStateError) -> Self {
        Self::State(format!("{:?}", value))
    }
}