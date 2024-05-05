use std::fmt::{Debug, Display};

use thiserror::Error;

use crate::language::LanguageBuilderError;

pub mod object;
pub use self::object::Object;

#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("cannot load object: {0}")]
    Format(anyhow::Error),
    #[error(transparent)]
    Language(#[from] LanguageBuilderError),
}

impl LoaderError {
    pub fn format<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Format(e.into())
    }

    pub fn format_with<M>(m: M) -> Self
    where
        M: Debug + Display + Send + Sync + 'static,
    {
        Self::Format(anyhow::Error::msg(m))
    }
}
