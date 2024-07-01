//! evaluator errors

use thiserror::Error;
use anyhow;

use fugue_ir;
use fugue_ir::Address;

use crate::context;

/// error types for the eval module
#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("instruction @ {0:#x?} not in translation cache")]
    TranslationCache(Address),
    #[error("context error: {0}")]
    Context(context::Error),
}

impl From<context::Error> for Error {
    fn from(value: context::Error) -> Self {
        Self::Context(value)
    }
}

impl Error {
    /// convert an arbitrary error into an evaluator runtime error
    pub fn runtime<E>(err: E) -> Self
    where
        E: std::error::Error + std::fmt::Debug + Send + Sync + 'static,
    {
        Self::Runtime(format!("{:?}", err))
    }

    /// create a runtime error form a static message
    pub fn runtime_with<M>(msg: M) -> Self
    where
        M: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static,
    {
        Self::Runtime(format!("{0}", msg))
    }
}