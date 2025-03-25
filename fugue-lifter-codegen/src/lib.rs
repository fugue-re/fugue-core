use std::env;
use std::path::Path;

use fugue_sleigh_language::{Language, LanguageDB};
#[cfg(feature = "bundled-compiler")]
use fugue_sleighc::{SleighCompiler, SleighCompilerError};
use prettyplease::unparse;
use proc_macro2::TokenStream;
use quote::ToTokens;
use thiserror::Error;

pub mod core;
pub mod error;
pub mod types;

pub use self::core::LifterGenerator;
pub use self::error::LifterGeneratorError;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("cannot format generated lifter: {0:#?}")]
    Format(anyhow::Error),
    #[error("cannot generate lifter: {0}")]
    Generate(LifterGeneratorError),
    #[error("cannot locate language `{0}` in language database")]
    Language(String),
    #[error("cannot build language for `{0}`: {1}")]
    LanguageBuild(String, anyhow::Error),
    #[cfg(feature = "bundled-compiler")]
    #[error("cannot compile language: {0}")]
    LanguageCompile(#[from] SleighCompilerError),
    #[error("cannot load/locate language database: {0}")]
    LanguageDB(anyhow::Error),
}

pub fn from_language(language: &Language) -> Result<TokenStream, LifterGeneratorError> {
    LifterGenerator::new(language).map(ToTokens::into_token_stream)
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

    let sla_file = language.language().sla_file();

    let language = if sla_file.exists() {
        language.build()
    } else {
        #[cfg(not(feature = "bundled-compiler"))]
        return Err(CodegenError::LanguageBuild(
            language_def.to_owned(),
            anyhow::msg!("no compiler available"),
        ));
        #[cfg(feature = "bundled-compiler")]
        {
            let slaf = Path::new(&env::var("OUT_DIR").expect("OUR_DIR set"))
                .join(sla_file.file_name().expect("sla file name"));
            let spec = sla_file.with_extension("");
            let slac = SleighCompiler::new()?
                .build_with(spec, slaf)?
                .expect("compiled sla file name");
            language.build_with_sla(slac)
        }
    };

    let language =
        language.map_err(|e| CodegenError::LanguageBuild(language_def.to_owned(), e.into()))?;

    let tokens = from_language(&language).map_err(CodegenError::Generate)?;
    let output = tokens.to_string();

    if pretty {
        Ok(unparse(
            &syn::parse_file(&output).map_err(|e| CodegenError::Format(e.into()))?,
        ))
    } else {
        Ok(output)
    }
}
