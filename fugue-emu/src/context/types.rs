//! context types
//! 
//! various struct and type definitions for contexts in general
use std::sync::Arc;
use bitflags::bitflags;

use fugue_ir::Address;
use fugue_core::ir::PCode;

use crate::context;

/// the default memory block alignment size
pub const DEFAULT_ALIGNMENT_SIZE: u64 = 0x1000u64;

/// a wrapper for lifter results
pub type LiftResult<'irb> = Result<Arc<PCode<'irb>>, context::Error>;

bitflags! {
    #[derive(Clone, Hash, PartialEq, Eq, Debug)]
    pub struct AccessType: u8 {
        const R = 0b0100;    // read
        const W = 0b0010;    // write
        const X = 0b0001;    // fetch/execute
        // const Z = 0b1000;   // no permissions (bitflags crate doesn't like 0-bit flags)
    }
}

/// translation block
/// 
/// a minimal translation block to keep track of lifted blocks
/// does not actually contain the lifted instructions
/// 
// TODO: change bytes to be backed by a bump allocator
#[derive(Clone)]
pub struct TranslationBlock {
    pub base: Address,
    pub insn_offsets: Vec<usize>,
    pub bytes: Vec<u8>,
}

impl TranslationBlock {

    /// create a new block with the given base address
    pub fn new_with(base: Address, insn_offsets: Vec<usize>, bytes: &[u8]) -> Self {
        let bytes = Vec::from(bytes);
        Self { base, insn_offsets, bytes }
    }
}

