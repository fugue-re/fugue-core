use std::path::Path;

use thiserror::Error;

use crate::arch::Arch;
use crate::lifter::{Language, Lifter};
use crate::loader::{Loadable, LoadableFromBytes, LoadableSegment, Loader, LoaderError};
use crate::storage::{InMemoryStorage, StorageError, StorageProvider};
use crate::types::{Address, AttributeMap};

pub struct Project<P: StorageProvider = InMemoryStorage> {
    arch: Arch,
    lifter: Lifter,
    language: &'static Language,
    memory: P,
}

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error(transparent)]
    Loader(#[from] LoaderError),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

impl<P> Project<P>
where
    P: StorageProvider,
{
    pub fn new(loader: &impl Loadable) -> Result<Self, ProjectError> {
        Self::from_loadable(loader).map_err(ProjectError::from)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProjectError> {
        Self::from_bytes_with(bytes, AttributeMap::default())
    }

    pub fn from_bytes_with(
        bytes: &[u8],
        attributes: impl Into<AttributeMap>,
    ) -> Result<Self, ProjectError> {
        Loader::from_bytes_with(bytes, attributes)
            .map_err(ProjectError::from)
            .and_then(|loader| Self::new(&loader))
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ProjectError> {
        Self::from_file_with(path, AttributeMap::default())
    }

    pub fn from_file_with(
        path: impl AsRef<Path>,
        attributes: impl Into<AttributeMap>,
    ) -> Result<Self, ProjectError> {
        Loader::from_file_with(path, attributes)
            .map_err(ProjectError::from)
            .and_then(|loader| Self::new(&loader))
    }

    pub fn architecture(&self) -> &Arch {
        &self.arch
    }

    pub fn lifter(&self) -> &Lifter {
        &self.lifter
    }

    pub fn language(&self) -> &'static Language {
        self.language
    }

    pub fn memory(&self) -> &P {
        &self.memory
    }

    pub fn memory_mut(&mut self) -> &mut P {
        &mut self.memory
    }
}

impl<P> StorageProvider for Project<P>
where
    P: StorageProvider,
{
    fn from_loadable(loadable: &impl Loadable) -> Result<Self, StorageError> {
        let arch = loadable.architecture();
        let lifter = loadable.lifter();
        let language = loadable.language();

        let memory = P::from_loadable(loadable)?;

        Ok(Self {
            arch,
            lifter,
            language,
            memory,
        })
    }

    fn read_bytes(&self, addr: Address, bytes: &mut [u8]) -> Result<(), StorageError> {
        self.memory.read_bytes(addr, bytes)
    }

    fn write_bytes(&mut self, addr: Address, bytes: &[u8]) -> Result<(), StorageError> {
        self.memory.write_bytes(addr, bytes)
    }

    fn insert_segment(&mut self, segm: LoadableSegment) -> Result<(), StorageError> {
        self.memory.insert_segment(segm)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_project() -> Result<(), Box<dyn std::error::Error>> {
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
            .with_line_number(true)
            .with_file(true)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let _project = Project::<InMemoryStorage>::from_file("tests/ls.elf")?;

            Ok(())
        })
    }
}
