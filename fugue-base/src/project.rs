use std::path::Path;

use crate::arch::Arch;
use crate::lifter::{Language, Lifter};
use crate::loader::{Loadable, Loader, LoaderError};

pub struct Project {
    arch: Arch,
    lifter: Lifter,
    language: &'static Language,
}

impl Project {
    pub fn new(loader: &impl Loadable) -> Result<Self, LoaderError> {
        let arch = loader.architecture();
        let lifter = loader.lifter();
        let language = loader.language();

        Ok(Self {
            arch,
            lifter,
            language,
        })
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, LoaderError> {
        Loader::from_file(path).and_then(|loader| Self::new(&loader))
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
}
