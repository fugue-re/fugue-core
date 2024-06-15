//! eval traits
//! 
//! defines various traits related to emulation evaluators

// use std::fmt;
// use std::clone;
// use std::hash;
// use std::ops::{
//     Add, AddAssign,
//     Sub, SubAssign,
//     Mul, MulAssign,
//     Div, DivAssign,
//     Shl, ShlAssign,
//     Shr, ShrAssign,
//     Rem, RemAssign,
//     BitAnd, BitAndAssign,
//     BitOr,  BitOrAssign,
//     BitXor, BitXorAssign,
//     Neg, Not,
// };

// use serde;

use fugue_core::ir::Location;
use fugue_ir::{Address, VarnodeData};

use crate::eval;

pub trait EvaluatorContext {
    /// evaluator datatype
    /// BitVec for concrete evaluator, can be modified for taint evaluator
    type Data;

    /// read data at the location of the specified varnode
    fn read_vnd(&self, var: &VarnodeData) -> Result<Self::Data, eval::Error>;

    /// write data to the location of the specified varnode
    fn write_vnd(&mut self, var: &VarnodeData, val: &Self::Data) -> Result<(), eval::Error>;
}

pub trait Evaluator {
    type Context: EvaluatorContext;
    /// the evaluator step function should be given a mutable reference to 
    /// a context and perform a single architectural step on it.
    fn step(
        &mut self,
        context: &mut Self::Context,
    ) -> Result<(), eval::Error>;

    fn substep(
        &mut self,
        context: &mut Self::Context,
    ) -> Result<(), eval::Error>;

}