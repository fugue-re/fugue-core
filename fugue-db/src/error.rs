use crate::architecture::ArchitectureDef;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{name} backend: {source}")]
    ImporterBackend { name: &'static str, source: Box<dyn std::error::Error + Send + Sync + 'static> },
    #[error(transparent)]
    CannotCreateTempDir(std::io::Error),
    #[error(transparent)]
    CannotReadFile(std::io::Error),
    #[error(transparent)]
    CannotWriteFile(std::io::Error),
    #[error(transparent)]
    Deserialisation(capnp::Error),
    #[error("function at {0:#x} has no corresponding segment")]
    NoFunctionSegment(u64),
    #[error("block at {0:#x} has no corresponding segment")]
    NoBlockSegment(u64),
    #[error("no importer backends available")]
    NoBackendsAvailable,
    #[error("file not found at `{}`", _0.display())]
    FileNotFound(std::path::PathBuf),
    #[error(transparent)]
    Serialisation(capnp::Error),
    #[error(transparent)]
    Translator(fugue_ir::error::Error),
    #[error("unsupported architecture: {0}")]
    UnsupportedArchitecture(ArchitectureDef),
    #[error("unsupported format `{0}`")]
    UnsupportedFormat(String),
}

impl Error {
    pub fn importer_error<E>(name: &'static str, e: E) -> Self
    where E: std::error::Error + Send + Sync + 'static {
        Self::ImporterBackend { name, source: Box::new(e) }
    }
}
