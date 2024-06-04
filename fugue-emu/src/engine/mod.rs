//! the cpu module
//! 
//! contains evaluator, registers, instruction fetch and cache

pub mod icache;
pub(crate) mod fetcher;
pub mod tblock;
pub mod tgraph;

use thiserror::Error;

#[allow(unused_imports)]
use fugue_core::{
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
use fugue_bv::BitVec;
use fugue_ir::{
    Address,
    VarnodeData,
    Translator,
};
use crate::context::manager::ContextManager;
use crate::emu::{
    EmulationError,
    EmulationType,
    Clocked,
};
use icache::ICache;

pub type EngineType = EmulationType;

// EngineError
#[derive(Debug, Error)]
pub enum EngineError {
    #[error("Runtime Error: {0}")]
    Runtime(EvaluatorError),
    #[error("Fetch Error: {0}")]
    Fetch(anyhow::Error),
}

impl From<EvaluatorError> for EngineError {
    fn from(err: EvaluatorError) -> Self {
        EngineError::Runtime(err)
    }
}

impl EngineError {
    /// used to generate a Fetch Error
    pub fn fetch<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Fetch(anyhow::Error::new(e))
    }
}

#[derive(Copy, Clone)]
pub struct ProgramCounter {
    vnd: VarnodeData,
}

impl ProgramCounter {
    /// ProgramCounter constructor
    /// takes ownership of pc_varnode for itself
    pub fn new(pc_varnode: &VarnodeData) -> Self {
        Self { vnd: pc_varnode.clone() }
    }

    #[inline(always)]
    /// set program counter to address
    pub fn set_pc(
        &mut self, 
        address: impl Into<Address>,
        context: &mut impl EvaluatorContext,
    ) -> Result<(), EngineError> {
        let addr = u64::from(address.into());
        let val = &BitVec::from_u64(addr, self.vnd.bits());
        context
            .write_vnd(&self.vnd, val)
            .map_err(EngineError::from)
    }

    /// get program counter from context
    #[inline(always)]
    pub fn get_pc_loc(
        &mut self,
        context: &mut impl EvaluatorContext,
    ) -> Location {
        // read the pc val in the varnode (expect always exists)
        // if it doesn't, panic.
        let read_val = context
            .read_vnd(&self.vnd)
            .unwrap()
            .to_u64().unwrap_or(0u64);

        Location::new(read_val, 0u32)
    }
}

/// a concrete emulation engine
/// 
/// manages instruction fetches and execution
pub struct Engine<'a> 
{
    pub lifter: Lifter<'a>,
    pub evaluator: Evaluator<'a>,
    pub engine_type: EngineType,
    pub pc: ProgramCounter,

    pub(crate) icache: ICache<'a>, // instruction cache
}

impl<'a> Engine<'a> {
    /// instantiate a new concrete emulation engine
    /// 
    /// note: lifter continues to be borrowed by evaluator
    pub fn new(
        translator: &'a Translator,
        engine_type: EngineType,
        irb_size: Option<usize>,
    ) -> Self {
        let program_counter_vnd = translator.program_counter();

        let evaluator = match engine_type {
            EngineType::Concrete => Evaluator::new(translator),
        };
        Self {
            lifter: Lifter::new(&translator),
            evaluator: evaluator,
            engine_type: engine_type,
            pc: ProgramCounter::new(program_counter_vnd),
            icache: ICache::new(Lifter::new(&translator).irb(irb_size.unwrap_or(1024))),
        }
    }

    /// get reference to engine lifter
    pub fn lifter(&self) -> &Lifter<'a> {
        &self.lifter
    }

    /// get engine type
    pub fn engine_type(&self) -> &EngineType {
        &self.engine_type
    }

    /// execute single pcode instruction and return new current location
    pub fn substep(&mut self) -> Result<(EvaluatorTarget, Location), EmulationError> {
        // refactor step() to call substep until Location has different address
        todo!()
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
        let pc_loc = self.pc.get_pc_loc(context);
        let pcode = self.icache
            .fetch(&self.lifter, &pc_loc, context, &self.engine_type)?;
        let insn_length = pcode.length;

        // evaluate lifted pcode
        // right now assumes everything but last is fall-through
        let mut next_pc_addr = pc_loc.address + (insn_length as usize);
        for (_i, op) in pcode.operations().iter().enumerate() {
            let target = self.evaluator
                .step(pc_loc, op, context)?;
            match target {
                EvaluatorTarget::Branch(loc) |
                EvaluatorTarget::Call(loc) |
                EvaluatorTarget::Return(loc) => {
                    next_pc_addr = loc.address
                },
                EvaluatorTarget::Fall => { },
            };
        }
        self.pc.set_pc(next_pc_addr, context)?;
        
        Ok(())
    }
}

