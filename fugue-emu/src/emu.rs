//! emulator module
//! 
//! should act as the execution manager and tie together 
//! evaluator, context, and hooks
#![allow(unused_imports)]
use thiserror::Error;

use fugue::bv::BitVec;
use fugue::bytes::Endian;
use fugue::ir::{
    Address,
    AddressSpace,
    Translator,
    VarnodeData,
};
use fugue::high::{
    ir::Location,
    lifter::Lifter,
    eval::{
        fixed_state::FixedState,
        EvaluatorError,
        EvaluatorContext,
    },
};

use crate::context;
use crate::engine;

// EngineError
#[derive(Debug, Error)]
pub enum EmulationError {
    #[error("Engine Error: {0}")]
    Engine(engine::EngineError),
    #[error("Context Error: {0}")]
    Context(context::ContextError),
    #[error("Evaluator Error: {0}")]
    Evaluator(EvaluatorError),
}

impl From<context::ContextError> for EmulationError {
    fn from(err: context::ContextError) -> Self {
        Self::Context(err)
    }
}

impl From<engine::EngineError> for EmulationError {
    fn from(err: engine::EngineError) -> Self {
        Self::Engine(err)
    }
}

impl From<EvaluatorError> for EmulationError {
    fn from(err: EvaluatorError) -> Self {
        Self::Evaluator(err)
    }
}

/// Clocked trait
/// 
/// implementation implies that actions must be taken
/// at each step of the emulation clock, independent of the engine.
/// this gives the user flexibility in dictating how clocked components
/// (aside from the emulation engine) should behave
pub trait Clocked<'a> {
    /// defines actions the object takes in a single step of the simulation clock
    /// these are resolved in order that the objects were registered.
    fn step(
        &mut self, 
        context: &mut context::manager::ContextManager<'a>
    ) -> Result<(), EmulationError>;
}

/// Timed trait
/// 
/// implementation implies that action must be taken at a certain 
/// time has elapsed.
/// enable/disable effects must be user-defined
/// 
/// at the end or every simulation step (after step is called), the `countdown` 
/// function will be called on all registered `Timed` objects
/// at the beginning of every simulation step (before step is called),
/// the `timed_out` function will be called, and if it returns true,
/// then the `timeout_handler` will be called.
pub trait Timed {
    /// called after every simulation step for user to decrement internal countdown
    fn countdown(&mut self) -> ();
    /// called before every simulation step to check for timeout
    fn timed_out(&self) -> bool;
    /// called if `timed_out` returned true
    fn timeout_handler(&mut self) -> Result<(), EmulationError>;
}





