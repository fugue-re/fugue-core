use fugue::bv::BitVec;
use fugue::bytes::Endian;
use fugue::ir::{
    Address,
    AddressSpace,
    Translator,
    VarnodeData,
    Location,
};
use fugue::high::{
    lifter::Lifter,
    eval::{
        fixed_state::FixedState,
        EvaluatorError,
        EvaluatorContext,
    },
};


/// A context manager
/// 
/// Takes in multiple contexts that implement EvaluatorContext
/// and implements EvaluatorContext to call indirectly.
/// 
/// The ContextManager allows multiple contexts to be declared
/// and modified by the Evaluator, and facilitates access for
/// the user.
pub struct ContextManager<C: EvaluatorContext> {
    base: Address,
    endian: Endian,
    mems: Vec<C>,
    regs: Vec<C>,
    tmps: Vec<C>,
}

impl ContextManager {
    /// instantiate a new metacontext with list of contexts
}