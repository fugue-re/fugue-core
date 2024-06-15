//! context errors

use thiserror::Error;

use fugue_ir::Address;


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
    #[error("{0}")]
    State(String),
}

// enable coercion from disassembly/lifting errors
impl From<fugue_ir::error::Error> for Error {
    fn from(value: fugue_ir::error::Error) -> Self {
        Self::Lift(format!("{:?}", value))
    }
}