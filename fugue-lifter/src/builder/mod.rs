use std::path::Path;

use fugue_ir::{LanguageDB, Translator};
use prettyplease::unparse;
use proc_macro2::TokenStream;
use quote::ToTokens;
use thiserror::Error;

pub mod core;
pub mod error;

pub use self::core::LifterGenerator;
pub use self::error::LifterGeneratorError;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("cannot format generated lifter: {0:#?}")]
    Format(anyhow::Error),
    #[error("cannot generate lifter: {0}")]
    Generate(LifterGeneratorError),
    #[error("cannot locate language `{0}` in database database")]
    Language(String),
    #[error("cannot build language translator for `{0}`: {1}")]
    LanguageBuild(String, anyhow::Error),
    #[error("cannot load/locate language database: {0}")]
    LanguageDB(anyhow::Error),
}

pub fn from_translator(translator: &Translator) -> Result<TokenStream, LifterGeneratorError> {
    LifterGenerator::new(translator).map(ToTokens::into_token_stream)
}

pub fn build(root: impl AsRef<Path>, language: impl AsRef<str>) -> Result<String, CodegenError> {
    build_with(root, language, false)
}

pub fn build_with(
    root: impl AsRef<Path>,
    language: impl AsRef<str>,
    pretty: bool,
) -> Result<String, CodegenError> {
    let builder = LanguageDB::from_directory_with(root.as_ref(), true)
        .map_err(|e| CodegenError::LanguageDB(e.into()))?;

    let language_def = language.as_ref();
    let language = builder
        .lookup_str(&language_def)
        .ok()
        .flatten()
        .ok_or_else(|| CodegenError::Language(language_def.to_owned()))?;

    let translator = language
        .build_with(true)
        .map_err(|e| CodegenError::LanguageBuild(language_def.to_owned(), e.into()))?;

    let tokens = from_translator(&translator).map_err(CodegenError::Generate)?;
    let output = tokens.to_string();

    if pretty {
        Ok(unparse(
            &syn::parse_file(&output).map_err(|e| CodegenError::Format(e.into()))?,
        ))
    } else {
        Ok(output)
    }
}
