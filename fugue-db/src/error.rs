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
    Deserialisation(flatbuffers::InvalidFlatbuffer),
    #[error("field `{0}` to deserialise is missing")]
    DeserialiseField(&'static str),
    #[error("export path `{}` for serialised database already exists", _0.display())]
    ExportPathExists(std::path::PathBuf),
    #[error("export of serialised database failed: {0}")]
    ExportViaCopy(fs_extra::error::Error),
    #[error("function at {0:#x} has no corresponding segment")]
    NoFunctionSegment(u64),
    #[error("block at {0:#x} has no corresponding segment")]
    NoBlockSegment(u64),
    #[error("no importer backends available")]
    NoBackendsAvailable,
    #[error("no URL specified for database import")]
    NoImportUrl,
    #[error("file not found at `{}`", _0.display())]
    FileNotFound(std::path::PathBuf),
    #[error("invalid local import URL `{0}`")]
    InvalidLocalImportUrl(url::Url),
    #[error("could not lift instruction at {address:#x}: {source}")]
    Lifting { address: u64, source: fugue_ir::error::Error },
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
