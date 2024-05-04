pub mod concrete;
pub mod manager;

use thiserror::Error;

use fugue::bv::BitVec;
use fugue::ir::{
    Address,
    VarnodeData,
};
use fugue::high::eval::{
    EvaluatorContext,
    EvaluatorError,
};

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

/// the context
pub enum Context {
    Concrete(concrete::ConcreteMemory),
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
        address: impl Into<Address>, 
        size: usize
    ) -> Result<Vec<u8>, ContextError>;
    /// write bytes to context
    fn write_bytes(
        &mut self,
        address: impl Into<Address>,
        values: &[u8],
    ) -> Result<(), ContextError>;
}

// pattern inspired by https://stackoverflow.com/questions/59889518
// may need a refactor eventually
impl EvaluatorContext for Context {
    fn read_vnd(
        &mut self, 
        var: &VarnodeData
    ) -> Result<BitVec, EvaluatorError> {
        match self {
            Context::Concrete(c) => c.read_vnd(var),
        }
    }

    fn write_vnd(
        &mut self, 
        var: &VarnodeData, 
        val: &BitVec
    ) -> Result<(), EvaluatorError> {
        match self {
            Context::Concrete(c) => c.write_vnd(var, val),
        }
    }
}

impl MappedContext for Context {
    fn base(&self) -> Address {
        match self {
            Context::Concrete(c) => c.base(),
        }
    }

    fn size(&self) -> usize {
        match self {
            Context::Concrete(c) => c.size(),
        }
    }
    
    fn read_bytes(
        &self, 
        address: impl Into<Address>, 
        size: usize
    ) -> Result<Vec<u8>, ContextError> {
        match self {
            Context::Concrete(c) => {
                c.read_bytes(address, size)
            },
        }
    }
    
    fn write_bytes(
        &mut self,
        address: impl Into<Address>,
        values: &[u8],
    ) -> Result<(), ContextError> {
        match self {
            Context::Concrete(c) => {
                c.write_bytes(address, values)
            },
        }
    }
}