use std::borrow::Cow;
use std::marker::PhantomData;

use fugue_bv::BitVec;

use fugue_ir::disassembly::{IRBuilderArena, Opcode, PCodeData};
use fugue_ir::il::Location;
use fugue_ir::{Address, VarnodeData};

use thiserror::Error;

use crate::lifter::{Lifter, PCode};

#[derive(Debug, Error)]
pub enum EvaluatorError {
    #[error("{0}")]
    Lift(fugue_ir::error::Error),
}

pub trait EvaluatorContext<'ir> {
    fn read_vnd(&mut self, var: &VarnodeData) -> Result<BitVec, EvaluatorError>;
    fn write_vnd(&mut self, var: &VarnodeData, val: &BitVec) -> Result<BitVec, EvaluatorError>;

    fn fetch(&mut self, addr: Address) -> Result<Vec<PCodeData<'ir>>, EvaluatorError>;
}

pub struct DummyContext<'a, 'ir> {
    lifter: Lifter<'a>,
    irb: &'ir IRBuilderArena,
}

impl<'a, 'ir> EvaluatorContext<'ir> for DummyContext<'a, 'ir> {
    fn read_vnd(&mut self, var: &VarnodeData) -> Result<BitVec, EvaluatorError> {
        todo!()
    }

    fn write_vnd(&mut self, var: &VarnodeData, val: &BitVec) -> Result<BitVec, EvaluatorError> {
        todo!()
    }

    fn fetch(&mut self, addr: Address) -> Result<Vec<PCodeData<'ir>>, EvaluatorError> {
        self.lifter
            .lift(self.irb, addr, &[])
            .map_err(EvaluatorError::Lift)
    }
}

pub struct Evaluator<'a, 'ir, C>
where
    C: EvaluatorContext<'ir>,
{
    context: &'a mut C,
    step_state: Option<StepState>,
    _marker: PhantomData<fn() -> &'ir [PCodeData<'ir>]>,
}

struct StepState {
    location: Location,
}

impl<'a, 'ir, C> Evaluator<'a, 'ir, C>
where
    C: EvaluatorContext<'ir>,
{
    pub fn new(context: &'a mut C) -> Self {
        Self {
            context,
            step_state: None,
        }
    }

    fn fetch_next(data: PCodeData) {}

    fn fetch_operation(
        context: &mut C,
        step_state: &mut Option<StepState>,
        location: Location,
    ) -> Result<Opcode, EvaluatorError> {
        todo!()
    }

    pub fn step_from(&mut self, location: impl Into<Location>) -> Result<(), EvaluatorError> {
        let location = location.into();

        // 1. obtain next operation

        // 2. evaluate the operation

        Ok(())
    }
}
