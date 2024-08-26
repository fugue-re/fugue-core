//! simulation errors

use thiserror::Error;
use anyhow;

use crate::eval;
use crate::context;
use super::channel;

#[derive(Error, Debug)]
pub enum Error {
    #[error("simulation time: {1} | evaluator error: {0}")]
    Evaluator(eval::Error, usize),
    #[error("simulation time: {1} | context error: {0}")]
    Context(context::Error, usize),
    #[error("clocked element error: {0}")]
    Clocked(anyhow::Error),
    #[error("clock error: {0}")]
    Clock(String),
    #[error(transparent)]
    Channel(#[from] channel::Error),
}