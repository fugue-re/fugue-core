//! emu traits
//! 
//! defines various traits related to managing simulation
//! 
//! defines some transction-level modeling taits for
//! communication between simulation components

use std::sync::mpsc;

use fugue_core::eval::EvaluatorContext;
use fugue_ir::disassembly::lift::IRBuilderArena;

use crate::eval::traits::Evaluator;
use crate::context::traits::Context;
use crate::sim;

/// simulation trait
/// 
/// it should define a virtual time resolution to increment at each step
/// of the simulation (see Renode's time framework)
pub trait Simulation {

    /// run the simulation until halt condition detected
    fn run(
        &mut self,
    ) -> Result<(), sim::Error>;
}

/// clocked trait
/// 
/// implementation implies that actions must be taken
/// at each step of the simulation clock, independent of the engine.
/// this gives the user flexibility in dictating how clocked components
/// (aside from the simulation engine) should behave
pub trait Clocked {
    
    /// the step method is invoked at each simulation clock cycle
    /// an simulation clock cycle is defined by the execution of 
    /// a single architectural instruction
    fn step(&mut self) -> Result<(), sim::Error> {
        Ok(())
    }
}
