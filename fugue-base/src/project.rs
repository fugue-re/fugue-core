use std::borrow::Cow;
use std::path::Path;

use thiserror::Error;

use crate::arch::Arch;
use crate::lifter::{Language, Lifter};
use crate::loader::{
    ExternSymbols, Loadable, LoadableFromBytes, LoadableSegment, Loader, LoaderError, LocalSymbols,
    SymbolEntry,
};
use crate::storage::{InMemoryStorage, StorageError, StorageProvider, StorageProviderFromLoadable};
use crate::types::attributes::{ATTRIBUTE_FILE_PATH, ATTRIBUTE_PROJECT_PATH};
use crate::types::{Address, AttributeMap};

pub struct Project {
    pub(crate) arch: Arch,
    pub(crate) lifter: Lifter,
    pub(crate) language: &'static Language,
    pub(crate) entry: Option<Address>,
    pub(crate) local_symbols: Option<LocalSymbols>,
    pub(crate) extern_symbols: Option<ExternSymbols>,
    pub(crate) storage: Box<dyn StorageProvider>,
}

pub struct ProjectRef<'a> {
    pub arch: &'a Arch,
    pub lifter: &'a Lifter,
    pub language: &'static Language,
    pub entry: Option<Address>,
    pub local_symbols: Option<&'a LocalSymbols>,
    pub extern_symbols: Option<&'a ExternSymbols>,
    pub storage: &'a Box<dyn StorageProvider>,
}

pub struct ProjectMut<'a> {
    pub arch: &'a mut Arch,
    pub lifter: &'a mut Lifter,
    pub language: &'static Language,
    pub entry: Option<Address>,
    pub local_symbols: Option<&'a mut LocalSymbols>,
    pub extern_symbols: Option<&'a mut ExternSymbols>,
    pub storage: &'a mut Box<dyn StorageProvider>,
}

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error(transparent)]
    Loader(#[from] LoaderError),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

impl Project {
    pub fn new<P>(loadable: &impl Loadable) -> Result<Self, ProjectError>
    where
        P: StorageProviderFromLoadable,
    {
        let arch = loadable.architecture();
        let lifter = loadable.lifter();
        let language = loadable.language();
        let storage = Box::new(P::from_loadable(loadable)?);

        // FIXME: ideally we should not clone these, since we could consume the loadable, but I
        // can see scenarios where this isn't desirable.

        let local_symbols = loadable.local_symbols().cloned();
        let extern_symbols = loadable.extern_symbols().cloned();

        Ok(Self {
            arch,
            lifter,
            language,
            entry: loadable.entry(),
            local_symbols,
            extern_symbols,
            storage,
        })
    }

    pub fn from_bytes<P>(bytes: &[u8]) -> Result<Self, ProjectError>
    where
        P: StorageProviderFromLoadable,
    {
        Self::from_bytes_with::<P>(bytes, AttributeMap::default())
    }

    pub fn from_bytes_with<P>(
        bytes: &[u8],
        attributes: impl Into<AttributeMap>,
    ) -> Result<Self, ProjectError>
    where
        P: StorageProviderFromLoadable,
    {
        Loader::from_bytes_with(bytes, attributes)
            .map_err(ProjectError::from)
            .and_then(|loader| Self::new::<P>(&loader))
    }

    pub fn from_file<P>(path: impl AsRef<Path>) -> Result<Self, ProjectError>
    where
        P: StorageProviderFromLoadable,
    {
        Self::from_file_with::<P>(path, AttributeMap::default())
    }

    pub fn from_file_with<P>(
        path: impl AsRef<Path>,
        attributes: impl Into<AttributeMap>,
    ) -> Result<Self, ProjectError>
    where
        P: StorageProviderFromLoadable,
    {
        let path = path.as_ref();
        let mut attributes = attributes.into();

        if !attributes.contains(ATTRIBUTE_FILE_PATH) {
            attributes.set_attr(ATTRIBUTE_FILE_PATH, path);
        }

        if !attributes.contains(ATTRIBUTE_PROJECT_PATH) {
            attributes.set_attr(ATTRIBUTE_PROJECT_PATH, path.with_extension("fudb"));
        }

        Loader::from_file_with(path, attributes)
            .map_err(ProjectError::from)
            .and_then(|loader| Self::new::<P>(&loader))
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

    pub fn entry(&self) -> Option<Address> {
        self.entry
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

    pub fn storage(&self) -> &impl StorageProvider {
        &self.storage
    }

    pub fn storage_mut(&mut self) -> &mut impl StorageProvider {
        &mut self.storage
    }

    pub fn fields(&self) -> ProjectRef {
        ProjectRef {
            arch: &self.arch,
            lifter: &self.lifter,
            language: self.language,
            entry: self.entry,
            local_symbols: self.local_symbols.as_ref(),
            extern_symbols: self.extern_symbols.as_ref(),
            storage: &self.storage,
        }
    }

    pub fn fields_mut(&mut self) -> ProjectMut {
        ProjectMut {
            arch: &mut self.arch,
            lifter: &mut self.lifter,
            language: self.language,
            entry: self.entry,
            local_symbols: self.local_symbols.as_mut(),
            extern_symbols: self.extern_symbols.as_mut(),
            storage: &mut self.storage,
        }
    }
}

impl StorageProvider for Project {
    fn read_bytes(&self, addr: Address, bytes: &mut [u8]) -> Result<usize, StorageError> {
        self.storage.read_bytes(addr, bytes)
    }

    fn write_bytes(&mut self, addr: Address, bytes: &[u8]) -> Result<usize, StorageError> {
        self.storage.write_bytes(addr, bytes)
    }

    fn contains_segment(&self, at: Address) -> bool {
        self.storage.contains_segment(at)
    }

    fn find_segment_containing(
        &self,
        addr: Address,
    ) -> Result<Cow<LoadableSegment<'_>>, StorageError> {
        self.storage.find_segment_containing(addr)
    }

    fn view_segment_bytes(&self, addr: Address, size: usize) -> Result<Cow<[u8]>, StorageError> {
        self.storage.view_segment_bytes(addr, size)
    }

    fn view_segment_bytes_from(&self, addr: Address) -> Result<Cow<[u8]>, StorageError> {
        self.storage.view_segment_bytes_from(addr)
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
            let project = Project::from_file::<InMemoryStorage>("tests/ls.elf")?;

            let mut bytes = [0u8; 32];
            project.storage().read_bytes(0x4000u32.into(), &mut bytes)?;

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
