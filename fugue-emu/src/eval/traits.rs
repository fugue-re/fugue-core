//! eval traits
//! 
//! defines various traits related to emulation evaluators

use std::sync::Arc;
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

use fugue_core::ir::PCode;
use fugue_ir::{
    Address,
    disassembly::PCodeData,
    disassembly::lift::IRBuilderArena,
};

use crate::context::traits::VarnodeContext;
use crate::context::types::TranslationBlock;
use crate::eval;

/// evaluator context trait
/// 
/// any context that an evaluator operates on must implement this trait
/// at minimum and share the evaluator's datatype (D generic)
///
/// D is BitVec for concrete evaluator, can be modified for taint evaluator
pub trait EvaluatorContext<'irb, Data>: VarnodeContext<Data> {

    /// lift a translation block at the given address
    /// 
    /// this function should be called as a prefetch, before any instruction
    /// in the corresponding translation block is fetched
    /// 
    /// prefetching will lift all architectural instructions up to the first 
    /// branching instruction and cache the lifted results in the context
    /// note that this function should not fail or panic. if a lift error 
    /// occurs, it should be cached and only raised when the evaluator
    /// actually attempts to execute it.
    /// 
    /// checking for the presence of an existing block is not necessary
    /// as the check should be performed in the evaluator step function.
    fn lift_block(
        &mut self,
        address: impl Into<Address>,
        irb: &'irb IRBuilderArena,
        // observers: &Vec<&mut dyn observer::BlockObserver>,
    ) -> TranslationBlock;

    /// get Arc reference to the lifted pcode of the architectural instruction
    /// at the given address
    /// 
    /// fetch should assume that prefetch has occured beforehand and
    /// will return an error if the location has not already been prefetched 
    fn fetch(&self, address: impl Into<Address>) -> Result<Arc<PCode<'irb>>, eval::Error>;

    /// fork the evaluator context
    /// 
    /// the intended behavior is for the forked context to be a clone
    /// of the original in the exact same state that the evaluator can 
    /// then operate on independently such that no alterations of the
    /// forked context will affect the original and vice versa.
    /// 
    /// note, however, that in order to improve performance, the 
    /// translation cache is in fact shared between clones and all lifted 
    /// instructions will be backed by the same IRBuilderArena (bumpalo arena).
    /// 
    /// because of this, some odd situations may arise if forked contexts are 
    /// modified concurrently; most obviously, self-modifying code is not 
    /// supported. (although self-generated code that does not overwrite
    /// existing instructions is probably allowable.)
    /// 
    /// also note that we do not need explicit restore functions for the context
    /// because the context being restored is instead directly passed to the 
    /// evaluator.
    fn fork(&self) -> Self;

}

/// evaluator trait
/// 
/// any evaluator implementation must define a step and substep function that
/// can be called to modify its associated context.
/// 
/// at this point, a step is assumed to always execute a full architectural instruction
/// while a substep should execute a single pcode operation
pub trait Evaluator<'irb> {
    type Data;
    type Context: EvaluatorContext<'irb, Self::Data>;

    /// perform a single architectural step on the given context
    /// 
    /// must also be passed the IRBuilderArena (bump arena) where 
    /// lifted pcode will be allocated
    fn step(
        &mut self,
        irb: &'irb IRBuilderArena,
        context: &mut Self::Context,
    ) -> Result<(), eval::Error>;

    /// evaluate a single pcode operation on the given context
    /// 
    /// this should be used as part of step() to evaluate instructions
    fn evaluate(&self,
        operation: &PCodeData,
        context: &mut Self::Context,
    ) -> Result<eval::Target, eval::Error>;
}


/// evaluator observer traits
/// 
/// observer traits may or may not need to be implemeneted
/// on a per-evaluator basis
pub mod observer {
    use crate::context::types::TranslationBlock;

    
    /// a block observer will be updated only when a new translation block
    /// is found and lifted, and before any of its instructions are
    /// evaluated. revisiting an old block will not update the observer
    pub trait BlockObserver {
        /// on update, the block observer will be given an immutable 
        /// reference to the raw translation block.
        /// this block will contain the lifted bytes, as well as the
        /// offsets of each lifted instruction, but will not contain
        /// any of the lifted instructions themselves
        fn update(&mut self, block: &TranslationBlock) {
            todo!();
        }
    }
}