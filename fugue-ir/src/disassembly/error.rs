use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("specification deserialisation error: {0}")]
    Specification(crate::deserialise::Error),
    #[error("address `{:#x}` misaligned; expected alignment is {}", address, alignment)]
    IncorrectAlignment {
        address: u64,
        alignment: usize,
    },
    #[error("context commit")]
    ContextCommit,
    #[error("input could not be resolved to known instruction")]
    InstructionResolution,
    #[error("next address undefined")]
    InvalidNextAddress,
    #[error("constructor invalid")]
    InvalidConstructor,
    #[error("pattern invalid")]
    InvalidPattern,
    #[error("symbol invalid")]
    InvalidSymbol,
    #[error("space invalid")]
    InvalidSpace,
    #[error("handle invalid")]
    InvalidHandle,
    #[error("inconsistent disassembly state")]
    InconsistentState,
    #[error("{0}")]
    Invariant(String),
}
