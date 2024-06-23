//! peripheral errors

use thiserror::Error;
use anyhow;

use fugue_ir::Address;
use fugue_bv::BitVec;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid peripheral write @ {0}: {1}")]
    InvalidWrite(Address, BitVec),
    #[error("invalid peripheral read of size {1} @ {0}")]
    InvalidRead(Address, usize),
}