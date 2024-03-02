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

        let translator = { TRANSLATOR_CACHE.read().get(&tag).cloned() };

        let translator = if let Some(translator) = translator {
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

#[cfg(test)]
mod test {
    use fugue_ir::Address;

    use super::*;

    #[test]
    #[ignore]
    fn test_load_arm32() -> anyhow::Result<()> {
        let lbuilder = LanguageBuilder::new("data")?;
        let language = lbuilder.build("ARM:LE:32:v7", "default")?;

        let memory = &[
            0x03, 0x00, 0x51, 0xE3, 0x0A, 0x00, 0x00, 0x9A, 0x00, 0x30, 0xA0, 0xE3, 0x01, 0x10,
            0x80, 0xE0, 0x03, 0x00, 0x80, 0xE2, 0x01, 0x20, 0xD0, 0xE4, 0x02, 0x30, 0x83, 0xE0,
            0x01, 0x00, 0x50, 0xE1, 0xFF, 0x30, 0x03, 0xE2, 0xFA, 0xFF, 0xFF, 0x1A, 0x00, 0x00,
            0x63, 0xE2, 0xFF, 0x00, 0x00, 0xE2, 0x1E, 0xFF, 0x2F, 0xE1, 0x00, 0x00, 0xA0, 0xE3,
            0x1E, 0xFF, 0x2F, 0xE1,
        ];

        let address = Address::from(0x00015E38u32);
        let mut off = 0usize;

        let mut lifter = language.lifter();
        let irb = lifter.irb(1024);

        while off < memory.len() {
            let insn = lifter.disassemble(&irb, address + off, &memory[off..])?;
            let pcode = lifter.lift(&irb, address + off, &memory[off..])?;

            println!("--- insn @ {} ---", insn.address());
            println!("{} {}", insn.mnemonic(), insn.operands());
            println!();

            println!("--- pcode @ {} ---", pcode.address());
            for (i, op) in pcode.operations().iter().enumerate() {
                println!("{i:02} {}", op.display(language.translator()));
            }
            println!();

            off += insn.len();
        }

        Ok(())
    }

    #[test]
    fn test_load_xtensa() -> anyhow::Result<()> {
        env_logger::try_init().ok();

        let lbuilder = LanguageBuilder::new("data")?;
        let language = lbuilder.build("Xtensa:LE:32:default", "default")?;

        let memory = &[
            0x36, 0x41, 0x00, 0x25, 0xFE, 0xFF, 0x0C, 0x1B, 0xAD, 0x02, 0x81, 0x4C, 0xFA, 0xE0,
            0x08, 0x00, 0x1D, 0xF0,
        ];

        let address = Address::from(0x40375C28u32);
        let mut off = 0usize;

        let mut lifter = language.lifter();
        let irb = lifter.irb(1024);

        while off < memory.len() {
            let insn = lifter.disassemble(&irb, address + off, &memory[off..])?;
            let pcode = lifter.lift(&irb, address + off, &memory[off..])?;

            println!("--- insn @ {} ---", insn.address());
            println!("{} {}", insn.mnemonic(), insn.operands());
            println!();

            println!("--- pcode @ {} ---", pcode.address());
            for (i, op) in pcode.operations().iter().enumerate() {
                println!("{i:02} {}", op.display(language.translator()));
            }
            println!();

            off += insn.len();
        }

        Ok(())
    }
}
