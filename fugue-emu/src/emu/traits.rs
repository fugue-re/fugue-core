//! emu traits
//! 
//! defines various traits related to managing emulation


use fugue_core::eval::EvaluatorContext;

use crate::eval::traits::Evaluator;
use crate::context::traits::Context;
use crate::emu;

/// emulation trait
/// 
/// the emulation should not take ownership fo the evaluator or context
/// the struct defined for the emulator should only really hold variables
/// relevent to the overall emulation state
/// 
/// it should define a virtual time resolution to increment at each step
/// of the emulation (see Renode's time framework)
pub trait Emulation<'irb> {
    type E: Evaluator<'irb>;
    type C: Context<'irb>;

    /// run the simulation until halt condition detected
    fn run(
        &mut self,
        evaluator: &mut Self::E,
        context: &mut Self::C,
    ) -> Result<(), emu::Error>;
}

/// clocked trait
/// 
/// implementation implies that actions must be taken
/// at each step of the emulation clock, independent of the engine.
/// this gives the user flexibility in dictating how clocked components
/// (aside from the emulation engine) should behave
pub trait Clocked {
    
    /// the step method is invoked at each emulation clock cycle
    /// an emulation clock cycle is defined by the execution of 
    /// a single architectural instruction
    fn step(&mut self) -> Result<(), emu::Error> {
        Ok(())
    }
}

pub trait EmulationHook<'irb, E: Evaluator<'irb>> {

}