//! icache module
//! 
//! implements translation block caching

use thiserror::Error;

use super::{
    EngineError,
    EngineType,
};
use fugue::high::{
    lifter::Lifter,
    ir::{
        // Insn,
        PCode,
        Location,
    },
};
use fugue::ir::disassembly::IRBuilderArena;
use crate::context::manager::ContextManager;

/// cache for lifted instructions
/// 
/// todo: implement actual caching behavior
pub(crate) struct ICache<'a> {
    irb: IRBuilderArena,
    pcode: Option<PCode<'a>>,
}

impl<'a> ICache<'a> {
    pub fn new(irb: IRBuilderArena) -> Self {
        Self {
            irb: irb,
            pcode: None,
        }
    }

    /// fetch and lift instruction at location
    pub(crate) fn fetch<'b>(
        &mut self,
        lifter: &Lifter,
        location: &Location,
        context: &mut ContextManager<'b>,
        engine_type: EngineType,
    ) -> Result<PCode, EngineError> {
        match engine_type {
            EngineType::Concrete => { // concrete fetch behavior
                let insn_bytes = context
                    .read_bytes(location.address, 4usize)
                    .map_err(EngineError::state)?;
                let mut lifter = lifter.clone();
                lifter.lift(&mut self.irb, location.address, &insn_bytes)
                    .map_err(EngineError::state)
            },
        }
    }
}