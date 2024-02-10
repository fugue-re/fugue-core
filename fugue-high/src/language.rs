use std::path::Path;
use std::sync::Arc;

use fugue_ir::convention::Convention;
use fugue_ir::endian::Endian;
use fugue_ir::{LanguageDB, Translator};

use rustc_hash::FxHashMap;
use static_init::dynamic;
use thiserror::Error;

use crate::lifter::Lifter;

#[derive(Debug, Error)]
pub enum LanguageBuilderError {
    #[error(transparent)]
    Arch(#[from] fugue_arch::ArchDefParseError),
    #[error(transparent)]
    Load(#[from] fugue_ir::error::Error),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error("unsupported architecture")]
    UnsupportedArch,
    #[error("unsupported architecture calling convention")]
    UnsupportedConv,
}

#[dynamic(drop)]
static mut TRANSLATOR_CACHE: FxHashMap<String, Arc<Translator>> = FxHashMap::default();

#[derive(Clone)]
pub struct LanguageBuilder {
    language_db: LanguageDB,
}

#[derive(Clone)]
pub struct Language {
    translator: Arc<Translator>,
    convention: Convention,
}

impl LanguageBuilder {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, LanguageBuilderError> {
        Self::new_with(path, true)
    }

    pub fn new_with(
        path: impl AsRef<Path>,
        ignore_errors: bool,
    ) -> Result<Self, LanguageBuilderError> {
        LanguageDB::from_directory_with(path, ignore_errors)
            .map(|language_db| Self { language_db })
            .map_err(LanguageBuilderError::from)
    }

    pub fn build(
        &self,
        tag: impl Into<String>,
        convention: impl AsRef<str>,
    ) -> Result<Language, LanguageBuilderError> {
        let tag = tag.into();
        let convention = convention.as_ref();

        let translator = if let Some(translator) = { TRANSLATOR_CACHE.read().get(&tag).cloned() } {
            translator
        } else {
            let builder = self
                .language_db
                .lookup_str(&*tag)?
                .ok_or_else(|| LanguageBuilderError::UnsupportedArch)?;

            let translator = Arc::new(builder.build().map_err(LanguageBuilderError::from)?);

            TRANSLATOR_CACHE
                .write()
                .entry(tag)
                .or_insert(translator)
                .clone()
        };

        if let Some(convention) = translator.compiler_conventions().get(&*convention).cloned() {
            Ok(Language::new(translator, convention))
        } else {
            Err(LanguageBuilderError::UnsupportedConv)
        }
    }

    pub fn build_with(
        &self,
        processor: impl AsRef<str>,
        endian: Endian,
        bits: u32,
        variant: impl AsRef<str>,
        convention: impl AsRef<str>,
    ) -> Result<Language, LanguageBuilderError> {
        let processor = processor.as_ref();
        let variant = variant.as_ref();

        let tag = format!("{processor}:{endian}:{bits}:{variant}");
        self.build(tag, convention)
    }
}

impl Language {
    pub fn new(translator: Arc<Translator>, convention: Convention) -> Self {
        Self {
            translator,
            convention,
        }
    }

    pub fn lifter(&self) -> Lifter {
        Lifter::new(&self.translator)
    }

    pub fn convention(&self) -> &Convention {
        &self.convention
    }

    pub fn translator(&self) -> &Translator {
        &self.translator
    }

    pub fn translator_mut(&mut self) -> &mut Translator {
        Arc::make_mut(&mut self.translator)
    }
}
