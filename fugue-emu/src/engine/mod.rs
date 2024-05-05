//! the cpu module
//! 
//! contains evaluator, registers, instruction fetch and cache
#![allow(unused_imports)]

pub mod icache;

use thiserror::Error;

use fugue::high::{
    lifter::Lifter,
    ir::{
        Insn,
        PCode,
        Location,
    },
    eval::{
        Evaluator,
        EvaluatorContext,
        EvaluatorTarget,
        EvaluatorError,
    }
};
use fugue::bv::BitVec;
use fugue::ir::{
    Address,
    VarnodeData,
    disassembly::IRBuilderArena,
};
use crate::context::manager::ContextManager;
use crate::emu::{
    EmulationError,
    Clocked,
};

// EngineError
#[derive(Debug, Error)]
pub enum EngineError {
    #[error("{0}")]
    State(anyhow::Error),
    #[error("Runtime Error: {0}")]
    Runtime(EvaluatorError),
    #[error("Fetch Error: {0}")]
    Fetch(anyhow::Error),
}

impl EngineError {

    /// used to generate a generic State EngineError
    pub fn state<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::State(anyhow::Error::new(e))
    }

    /// used to generate a generic State EngineError with custom message
    pub fn state_with<M>(msg: M) -> Self
    where
        M: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static,
    {
        Self::State(anyhow::Error::msg(msg))
    }

    /// used to generate a Fetch Error
    pub fn fetch<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Fetch(anyhow::Error::new(e))
    }
}

/// implemented engine types
pub enum EngineType {
    Concrete,
}

/// a concrete emulation engine
/// 
/// manages instruction fetches and execution
pub struct Engine<'a> 
{
    lifter: Lifter<'a>,
    evaluator: Evaluator<'a>,
    engine_type: EngineType,
    // icache: icache::Icache, // instruction cache
    pc_varnode: VarnodeData,
    irb: IRBuilderArena,
}

impl<'a> Engine<'a> {
    /// instantiate a new concrete emulation engine
    /// 
    /// note: lifter, evaluator, and context are consumed, not borrowed
    pub fn new(
        lifter: &'a mut Lifter,
        engine_type: EngineType,
        irb_size: Option<usize>,
    ) -> Self {
        let t = lifter.translator();
        let program_counter_vnd = t.program_counter();

        let evaluator = match engine_type {
            EngineType::Concrete => Evaluator::new(lifter),
        };
        Self {
            lifter: lifter.clone(),
            evaluator: evaluator,
            engine_type: engine_type,
            pc_varnode: program_counter_vnd.clone(),
            irb: lifter.irb(irb_size.unwrap_or(1024)),
        }
    }

    #[inline(always)]
    pub fn get_pc_loc<'b>(
        &mut self,
        context: &mut ContextManager<'b>,
    ) -> Location {
        // read the pc val in the varnode (expect always exists)
        // if it doesn't, panic.
        let read_val = context
            .read_vnd(&self.pc_varnode)
            .unwrap()
            .to_u64().unwrap_or(0u64);

        Location::new(read_val, 0u32)
    }

    /// get reference to engine lifter
    pub fn lifter(&self) -> &Lifter<'a> {
        &self.lifter
    }

    /// get engine type
    pub fn engine_type(&self) -> &EngineType {
        &self.engine_type
    }

    #[inline(always)]
    /// set program counter to address
    pub fn set_pc<'b>(
        &mut self, 
        address: impl Into<Address>,
        context: &mut ContextManager<'b>,
    ) -> Result<(), EngineError> {
        let addr = u64::from(address.into());
        let val = &BitVec::from_u64(addr, self.pc_varnode.size() * 8);
        context
            .write_vnd(&self.pc_varnode, val)
            .map_err(EngineError::state)
    }

    /// fetch and lift instruction at location
    pub(crate) fn fetch<'b>(
        &self,
        location: &Location,
        context: &mut ContextManager<'b>
    ) -> Result<PCode, EngineError> {
        match self.engine_type {
            EngineType::Concrete => { // concrete fetch behavior
                let insn_bytes = context
                    .read_bytes(location.address, 4usize)
                    .map_err(EngineError::state)?;
                let mut lifter = self.lifter.clone();
                lifter.lift(&self.irb, location.address, &insn_bytes)
                    .map_err(EngineError::state)
            },
        }
    }

}

impl<'a> Clocked<'a> for Engine<'a> {
    /// in a single simulation step we should do the following:
    /// - [ ] check for pending interrupts and do context switch if necessary
    /// - [ ] instruction fetch and lift
    ///     - [ ] cache lifted instructions
    /// - [ ] execute instruction pcode
    /// - [ ] update program counter if necessary
    fn step<'b>(&mut self, context: &mut ContextManager<'b>) -> Result<(), EmulationError> {
        // todo: implement interrupts

        // fetch and lift
        let pc_loc = self.get_pc_loc(context);
        let pcode = self.fetch(&pc_loc, context)
            .map_err(EmulationError::state)?;
        let insn_length = pcode.length;

        // evaluate lifted pcode
        // right now assumes everything but last is fall-through
        let mut next_pc_addr = pc_loc.address + (insn_length as usize);
        for (i, op) in pcode.operations().iter().enumerate() {
            let target = self.evaluator
                .step(pc_loc, op, context)
                .map_err(EmulationError::state)?;
            match target {
                EvaluatorTarget::Branch(loc) |
                EvaluatorTarget::Call(loc) |
                EvaluatorTarget::Return(loc) => {
                    next_pc_addr = loc.address
                },
                EvaluatorTarget::Fall => { },
            };
        }

        self.set_pc(next_pc_addr, context);
        
        Ok(())
    }
}

