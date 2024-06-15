//! emulation errors

use thiserror::Error;
use anyhow;

use crate::eval;
use crate::context;

#[derive(Debug, Error)]
pub enum Error {
    #[error("emulation time: {1} | evaluator error: {0}")]
    Evaluator(eval::Error, usize),
    #[error("emulation time: {1} | context error: {0}")]
    Context(context::Error, usize),
    #[error("clocked element error: {0}")]
    Clocked(anyhow::Error),
}