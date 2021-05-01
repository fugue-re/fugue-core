use std::num::ParseIntError;
use std::str::ParseBoolError;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("attribute `{0}` expected")]
    AttributeExpected(&'static str),
    #[error("cannot deserialise dependency `{}`: {}", path.display(), error)]
    DeserialiseDepends {
        path: PathBuf,
        error: Box<crate::error::Error>,
    },
    #[error("invariant not satisfied: {0}")]
    Invariant(&'static str),
    #[error("could not parse boolean: {0}")]
    ParseBool(#[from] ParseBoolError),
    #[error("could not parse integer: {0}")]
    ParseInteger(#[from] ParseIntError),
    #[error("could not parse endian")]
    ParseEndian,
    #[error("unexpected tag `{0}`")]
    TagUnexpected(String),
    #[error(transparent)]
    Xml(#[from] xml::Error),
}
