use std::path::Path;

use thiserror::Error;

use crate::arch::Arch;
use crate::lifter::{Language, Lifter};
use crate::loader::{
    ExternSymbols, Loadable, LoadableFromBytes, LoadableSegment, Loader, LoaderError, LocalSymbols,
    SymbolEntry,
};
use crate::storage::{InMemoryStorage, StorageError, StorageProvider};
use crate::types::{Address, AttributeMap};

pub struct Project<P: StorageProvider = InMemoryStorage> {
    pub(crate) arch: Arch,
    pub(crate) lifter: Lifter,
    pub(crate) language: &'static Language,
    pub(crate) local_symbols: Option<LocalSymbols>,
    pub(crate) extern_symbols: Option<ExternSymbols>,
    pub(crate) storage: P,
}

pub struct ProjectRef<'a, P: StorageProvider> {
    pub arch: &'a Arch,
    pub lifter: &'a Lifter,
    pub language: &'static Language,
    pub local_symbols: Option<&'a LocalSymbols>,
    pub extern_symbols: Option<&'a ExternSymbols>,
    pub storage: &'a P,
}

pub struct ProjectMut<'a, P: StorageProvider> {
    pub arch: &'a mut Arch,
    pub lifter: &'a mut Lifter,
    pub language: &'static Language,
    pub local_symbols: Option<&'a mut LocalSymbols>,
    pub extern_symbols: Option<&'a mut ExternSymbols>,
    pub storage: &'a mut P,
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
    pub fn new(loadable: &impl Loadable) -> Result<Self, ProjectError> {
        Self::from_loadable(loadable).map_err(ProjectError::from)
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

    pub fn lifter_mut(&mut self) -> &mut Lifter {
        &mut self.lifter
    }

    pub fn language(&self) -> &'static Language {
        self.language
    }

    pub fn local_symbols(&self) -> Option<&LocalSymbols> {
        self.local_symbols.as_ref()
    }

    pub fn iter_local_symbols<'a>(&'a self) -> impl Iterator<Item = SymbolEntry> + 'a {
        self.local_symbols
            .as_ref()
            .into_iter()
            .flat_map(|symbols| symbols.iter())
    }

    pub fn extern_symbols(&self) -> Option<&ExternSymbols> {
        self.extern_symbols.as_ref()
    }

    pub fn iter_extern_symbols<'a>(&'a self) -> impl Iterator<Item = SymbolEntry> + 'a {
        self.extern_symbols
            .as_ref()
            .into_iter()
            .flat_map(|symbols| symbols.iter())
    }

    pub fn storage(&self) -> &P {
        &self.storage
    }

    pub fn storage_mut(&mut self) -> &mut P {
        &mut self.storage
    }

    pub fn fields(&self) -> ProjectRef<P> {
        ProjectRef {
            arch: &self.arch,
            lifter: &self.lifter,
            language: self.language,
            local_symbols: self.local_symbols.as_ref(),
            extern_symbols: self.extern_symbols.as_ref(),
            storage: &self.storage,
        }
    }

    pub fn fields_mut(&mut self) -> ProjectMut<P> {
        ProjectMut {
            arch: &mut self.arch,
            lifter: &mut self.lifter,
            language: self.language,
            local_symbols: self.local_symbols.as_mut(),
            extern_symbols: self.extern_symbols.as_mut(),
            storage: &mut self.storage,
        }
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
        let storage = P::from_loadable(loadable)?;

        // FIXME: ideally we should not clone these, since we could consume the loadable, but I
        // can see scenarios where this isn't desirable.

        let local_symbols = loadable.local_symbols().cloned();
        let extern_symbols = loadable.extern_symbols().cloned();

        Ok(Self {
            arch,
            lifter,
            language,
            local_symbols,
            extern_symbols,
            storage,
        })
    }

    fn read_bytes(
        &self,
        addr: impl Into<Address>,
        bytes: &mut [u8],
    ) -> Result<usize, StorageError> {
        self.storage.read_bytes(addr, bytes)
    }

    fn write_bytes(
        &mut self,
        addr: impl Into<Address>,
        bytes: &[u8],
    ) -> Result<usize, StorageError> {
        self.storage.write_bytes(addr, bytes)
    }

    fn insert_segment(&mut self, segm: LoadableSegment) -> Result<(), StorageError> {
        self.storage.insert_segment(segm)
    }

    fn contains_segment(&self, at: impl Into<Address>) -> bool {
        self.storage.contains_segment(at)
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
            let project = Project::<InMemoryStorage>::from_file("tests/ls.elf")?;

            let mut bytes = [0u8; 32];
            project.storage().read_bytes(0x4000u32, &mut bytes)?;

            assert_eq!(
                &bytes,
                &[
                    0xF3, 0x0F, 0x1E, 0xFA, 0x48, 0x83, 0xEC, 0x08, 0x48, 0x8B, 0x05, 0xB9, 0xEF,
                    0x01, 0x00, 0x48, 0x85, 0xC0, 0x74, 0x02, 0xFF, 0xD0, 0x48, 0x83, 0xC4, 0x08,
                    0xC3, 0x00, 0x00, 0x00, 0x00, 0x00
                ]
            );

            Ok(())
        })
    }
}
