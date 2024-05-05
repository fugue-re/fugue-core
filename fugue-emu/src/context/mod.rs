pub mod concrete;
pub mod manager;

use thiserror::Error;

use fugue::ir::Address;
use fugue::high::eval::EvaluatorContext;

// ContextError
#[derive(Debug, Error)]
pub enum ContextError {
    #[error("{0}")]
    State(anyhow::Error),
}

impl ContextError {

    /// used to generate a generic State EvaluatorError
    pub fn state<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::State(anyhow::Error::new(e))
    }

    /// used to generate a generic State EvaluatorError with custom message
    pub fn state_with<M>(msg: M) -> Self
    where
        M: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static,
    {
        Self::State(anyhow::Error::msg(msg))
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
