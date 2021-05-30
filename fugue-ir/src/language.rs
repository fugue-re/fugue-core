use crate::deserialise::parse::XmlExt;
use crate::deserialise::Error as DeserialiseError;

use crate::endian::Endian;

use crate::error::Error;

use crate::compiler::Specification as CSpec;
use crate::processor::Specification as PSpec;
use crate::Translator;

use fnv::FnvHashMap as Map;
use itertools::Itertools;

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Processor {
    Aarch64,
    Arm,
    Mips,
    Ppc,
    V850,
    X86,
    Other(String),
}

impl<'a> From<&'a str> for Processor {
    fn from(s: &'a str) -> Processor {
        match s.to_uppercase().as_ref() {
            "AARCH64" => Self::Aarch64,
            "ARM" => Self::Arm,
            "MIPS" => Self::Mips,
            "PPC" => Self::Ppc,
            "V850" => Self::V850,
            "x86" => Self::X86,
            _ => Self::Other(s.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Variant {
    Default,
    Other(String),
}

impl<'a> From<&'a str> for Variant {
    fn from(s: &'a str) -> Variant {
        match s {
            "default" => Self::Default,
            other => Self::Other(other.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    processor: Processor,
    endian: Endian,
    size: usize,
    variant: Variant,
    version: String,
    sla_file: String,
    processor_spec: PSpec,
    compiler_specs: Map<String, CSpec>,
    id: String,
}

impl Language {
    pub fn from_xml<P: AsRef<Path>>(root: P, input: xml::Node) -> Result<Self, DeserialiseError> {
        if input.tag_name().name() != "language" {
            return Err(DeserialiseError::TagUnexpected(
                input.tag_name().name().to_owned(),
            ));
        }

        let mut path = root.as_ref().to_path_buf();
        let pspec_path = input.attribute_string("processorspec")?;
        path.push(pspec_path);

        let processor_spec =
            PSpec::from_file(&path).map_err(|e| DeserialiseError::DeserialiseDepends {
                path,
                error: Box::new(e),
            })?;

        let compiler_specs = input.children()
            .filter(|e| e.is_element() && e.tag_name().name() == "compiler")
            .map(|compiler| {
                let id = compiler.attribute_string("id")?;
                let name = compiler.attribute_string("name")?;

                let mut path = root.as_ref().to_path_buf();
                let cspec_path = compiler.attribute_string("spec")?;
                path.push(cspec_path);

                Ok((id, name, path))
            })
            .filter_map_ok(|(id, name, path)| {
                CSpec::named_from_file(name, &path).ok().map(|cspec| (id, cspec))
            })
            .collect::<Result<Map<_, _>, DeserialiseError>>()?;

        Ok(Self {
            processor: input.attribute_processor("processor")?,
            endian: input.attribute_endian("endian")?,
            size: input.attribute_int("size")?,
            variant: input.attribute_variant("variant")?,
            version: input.attribute_string("version")?,
            sla_file: input.attribute_string("slafile")?,
            processor_spec,
            compiler_specs,
            id: input.attribute_string("id")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LanguageBuilder<'a> {
    language: &'a Language,
    root: &'a Path,
}

impl<'a> LanguageBuilder<'a> {
    pub fn build(&self) -> Result<Translator, Error> {
        let mut path = self.root.to_path_buf();
        path.push(&self.language.sla_file);
        let mut translator =
            Translator::from_file(self.language.processor_spec.program_counter(), path)?;
        for (name, val) in self.language.processor_spec.context_set() {
            translator
                .set_variable_default(name.as_ref(), val);
        }
        Ok(translator)
    }
}

#[derive(Debug, Clone)]
pub struct LanguageDB {
    db: Map<(Processor, Endian, usize, Variant), Language>,
    root: PathBuf,
}

impl LanguageDB {
    pub fn lookup_default<'a, P: Into<Processor>>(
        &'a self,
        processor: P,
        endian: Endian,
        size: usize,
    ) -> Option<LanguageBuilder<'a>> {
        self.db
            .get(&(processor.into(), endian, size, Variant::Default))
            .map(|language| LanguageBuilder {
                language,
                root: &self.root,
            })
    }

    pub fn lookup<'a, P: Into<Processor>, V: Into<Variant>>(
        &'a self,
        processor: P,
        endian: Endian,
        size: usize,
        variant: V,
    ) -> Option<LanguageBuilder<'a>> {
        self.db
            .get(&(processor.into(), endian, size, variant.into()))
            .map(|language| LanguageBuilder {
                language,
                root: &self.root,
            })
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = LanguageBuilder<'a>> {
        self.db.iter().map(move |(_, language)| LanguageBuilder {
            language,
            root: &self.root,
        })
    }

    pub fn len(&self) -> usize {
        self.db.len()
    }

    pub fn from_xml<P: AsRef<Path>>(root: P, input: xml::Node) -> Result<Self, DeserialiseError> {
        if input.tag_name().name() != "language_definitions" {
            return Err(DeserialiseError::TagUnexpected(
                input.tag_name().name().to_owned(),
            ));
        }

        let root = root.as_ref().to_path_buf();

        Ok(Self {
            db: input
                .children()
                .filter(xml::Node::is_element)
                .filter(|t| t.tag_name().name() == "language")
                .map(|t| {
                    let ldef = Language::from_xml(&root, t)?;
                    Ok((
                        (
                            ldef.processor.clone(),
                            ldef.endian,
                            ldef.size,
                            ldef.variant.clone(),
                        ),
                        ldef,
                    ))
                })
                .collect::<Result<_, DeserialiseError>>()?,
            root,
        })
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let path = path.as_ref();
        let mut file = File::open(path).map_err(|error| Error::ParseFile {
            path: path.to_owned(),
            error,
        })?;

        let mut input = String::new();
        file.read_to_string(&mut input)
            .map_err(|error| Error::ParseFile {
                path: path.to_owned(),
                error,
            })?;

        let root = path
            .parent()
            .ok_or_else(|| {
                DeserialiseError::Invariant("cannot obtain parent directory of language defintions")
            })
            .map_err(|error| Error::DeserialiseFile {
                path: path.to_owned(),
                error,
            })?;

        Self::from_str(root, &input).map_err(|error| Error::DeserialiseFile {
            path: path.to_owned(),
            error,
        })
    }

    pub fn from_str<P: AsRef<Path>, S: AsRef<str>>(
        root: P,
        input: S,
    ) -> Result<Self, DeserialiseError> {
        let document = xml::Document::parse(input.as_ref()).map_err(DeserialiseError::Xml)?;

        Self::from_xml(root, document.root_element())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_language_def_arm() -> Result<(), Error> {
        let ldef = LanguageDB::from_file("./data/arm/ARM.ldefs")?;
        assert_eq!(ldef.len(), 20);
        Ok(())
    }

    #[test]
    fn test_language_def_arm_translator_all() -> Result<(), Error> {
        let ldef = LanguageDB::from_file("./data/arm/ARM.ldefs")?;
        assert_eq!(ldef.len(), 20);

        for builder in ldef.iter() {
            builder.build()?;
        }
        Ok(())
    }

    #[test]
    fn test_language_def_mips() -> Result<(), Error> {
        let ldef = LanguageDB::from_file("./data/mips/mips.ldefs")?;
        assert_eq!(ldef.len(), 18);
        Ok(())
    }

    #[test]
    fn test_language_def_mips_translator_all() -> Result<(), Error> {
        let ldef = LanguageDB::from_file("./data/mips/mips.ldefs")?;
        assert_eq!(ldef.len(), 18);

        for builder in ldef.iter() {
            builder.build()?;
        }

        Ok(())
    }

    #[test]
    fn test_language_def_x86() -> Result<(), Error> {
        let ldef = LanguageDB::from_file("./data/x86/x86.ldefs")?;
        assert_eq!(ldef.len(), 5);
        Ok(())
    }

    #[test]
    fn test_language_def_x86_translator_all() -> Result<(), Error> {
        let ldef = LanguageDB::from_file("./data/x86/x86.ldefs")?;
        assert_eq!(ldef.len(), 5);

        for builder in ldef.iter() {
            builder.build()?;
        }
        Ok(())
    }
}
