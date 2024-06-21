//! peripheral errors

use thiserror::Error;
use anyhow;

use fugue_ir::Address;
use fugue_bv::BitVec;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid peripheral write of size {1} @ {0}: {2}")]
    InvalidWrite(Address, usize, BitVec),
    #[error("invalid peripheral read of size {1} @ {0}")]
    InvalidRead(Address, usize),
}