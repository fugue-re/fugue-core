//! evaluator types
//! 
//! common struct and enum definitions for all evaluators

use fugue_core::ir::Location;

/// control flow target type
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Target {
    Branch(Location),
    Call(Location),
    Return(Location),
    Fall,
}

