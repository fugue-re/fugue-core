pub mod concrete;
pub mod manager;

use thiserror::Error;

use fugue_ir::Address;
use fugue_core::eval::{EvaluatorContext, EvaluatorError};

// ContextError
#[derive(Debug, Error)]
pub enum ContextError {
    #[error("{0}")]
    State(anyhow::Error),
    #[error("access to unmapped address {0}")]
    Unmapped(Address),
    #[error("new context at {0} conflicts with context at {1}")]
    MapConflict(Address, Address),
    #[error("address unaligned {0}")]
    UnalignedAddress(Address),
    #[error("size unaligned {0:#x}, expected {1:#x}-aligned")]
    UnalignedSize(usize, usize),
    #[error("unexpected error {0}")]
    Unexpected(anyhow::Error),
}

impl From<ContextError> for EvaluatorError {
    fn from(err: ContextError) -> Self {
        match err {
            ContextError::State(e) => EvaluatorError::State(e),
            ContextError::Unmapped(addr) => EvaluatorError::Address(addr.offset().into()),
            err => EvaluatorError::state(err),
        }
    }
}

impl From<EvaluatorError> for ContextError {
    fn from(err: EvaluatorError) -> Self {
        match err {
            EvaluatorError::State(e) => ContextError::State(e),
            EvaluatorError::Address(bv) => ContextError::Unmapped(bv.to_u64().unwrap().into()),
            err => ContextError::unexpected(err),
        }
    }
}

impl ContextError {

    /// used to generate a State ContextError
    /// usually indicates a FixedStateError
    pub fn state<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::State(anyhow::Error::new(e))
    }

    /// used to generate a State ContextError with custom message
    /// usually indicates a FixedStateError
    pub fn state_with<M>(msg: M) -> Self
    where
        M: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static,
    {
        Self::State(anyhow::Error::msg(msg))
    }

    /// used to generate an Unexpected ContextError
    /// we never expect to see these, so something cursed is happening
    /// if this pops up.
    pub fn unexpected<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Unexpected(anyhow::Error::new(e))
    }
}

pub enum ContextType {
    Concrete,
}

pub trait MappedContext: EvaluatorContext {
    /// return the context base address
    fn base(&self) -> Address;
    /// return the context size
    fn size(&self) -> usize;
    /// returns a vector of bytes
    fn read_bytes(
        &self, 
        address: Address,
        size: usize
    ) -> Result<Vec<u8>, ContextError>;
    /// write bytes to context
    fn write_bytes(
        &mut self,
        address: Address,
        values: &[u8],
    ) -> Result<(), ContextError>;
}
