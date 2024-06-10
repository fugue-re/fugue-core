//! the cpu module
//! 
//! contains evaluator, registers, instruction fetch and cache
use std::sync::Arc;

// pub mod icache;
// pub(crate) mod fetcher;
pub mod tblock;
pub mod tgraph;

// use fetcher::Fetcher;
use tblock::{TranslatedInsn, TranslationBlock, TranslationError};
use tgraph::TranslationGraph;
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
    disassembly::IRBuilderArena, Address, Translator, VarnodeData
};
use crate::context::manager::ContextManager;
use crate::emu::{
    EmulationError,
    EmulationType,
    Clocked,
};
// use icache::ICache;

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

    // pub(crate) icache: ICache<'a>, // instruction cache
    pub tgraph: TranslationGraph<'a>,
    pub(crate) current_base: Option<Address>,
    // pub(crate) fetcher: Fetcher<'a>,
    // pub irb: &'a mut IRBuilderArena,
}

impl<'a> Engine<'a> {
    /// instantiate a new concrete emulation engine
    /// 
    /// note: lifter continues to be borrowed by evaluator
    pub fn new<'z: 'a>(
        lifter: &'z Lifter<'z>,
        engine_type: EngineType,
        tgraph: TranslationGraph<'z>,
        // irb_size: Option<usize>,
    ) -> Self {
        let translator = lifter.translator();
        let program_counter_vnd = translator.program_counter();
        let evaluator = match engine_type {
            EngineType::Concrete => Evaluator::new(translator),
        };
        Self {
            lifter: Lifter::new(&translator),
            evaluator: evaluator,
            engine_type: engine_type,
            pc: ProgramCounter::new(program_counter_vnd),
            // icache: ICache::new(Lifter::new(&translator).irb(irb_size.unwrap_or(1024))),
            tgraph,
            current_base: None,
            // fetcher: Fetcher::new(),
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

    // /// get shared translation graph reference
    // pub fn tgraph(&self) -> &TranslationGraph<'a> {
    //     &self.tgraph
    // }

    /// execute single pcode instruction and return new current location
    pub fn substep(&mut self) -> Result<(EvaluatorTarget, Location), EmulationError> {
        // refactor step() to call substep until Location has different address
        todo!()
    }
}

impl<'a> Clocked for Engine<'a> {
    /// in a single simulation step we should do the following:
    /// - [ ] check for pending interrupts and do context switch if necessary
    /// - [ ] instruction fetch and lift
    ///     - [ ] cache lifted instructions
    /// - [ ] execute instruction pcode
    /// - [ ] update program counter if necessary
    fn step(&mut self, context: &mut ContextManager<'_>) -> Result<(), EmulationError> {
        // todo: implement interrupts
        
        // fetch and lift
        let pc_loc = self.pc.get_pc_loc(context);
        let fetch_result = self.tgraph.fetch(&mut self.lifter, pc_loc.address, context);
        if fetch_result.is_err() {
            let err = fetch_result.as_ref().unwrap_err();
            return Err(EmulationError::from(EngineError::fetch(err.clone())))
        }
        let translated_insn = fetch_result.as_ref().unwrap();
        let insn_length = translated_insn.pcode.length;

        // evaluate lifted pcode
        // right now assumes everything but last is fall-through
        let mut next_pc_addr = pc_loc.address + (insn_length as usize);
        for (_i, op) in translated_insn.pcode.operations().iter().enumerate() {
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

