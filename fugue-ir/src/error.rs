use std::io;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot deserialise file `{}`: {}", path.display(), error)]
    DeserialiseFile {
        path: PathBuf,
        error: crate::deserialise::Error,
    },
    #[error("cannot parse from file `{}`: {}", path.display(), error)]
    ParseFile {
        path: PathBuf,
        error: io::Error,
    },
    #[error(transparent)]
    Disassembly(#[from] crate::disassembly::Error),
}
