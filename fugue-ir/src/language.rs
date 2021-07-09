use crate::deserialise::parse::XmlExt;
use crate::deserialise::Error as DeserialiseError;

use crate::endian::Endian;
use crate::error::Error;

use crate::compiler::Specification as CSpec;
use crate::processor::Specification as PSpec;
use crate::Translator;

use fnv::FnvHashMap as Map;
use fugue_arch::ArchitectureDef;
use itertools::Itertools;
use walkdir::WalkDir;

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    id: String,
    architecture: ArchitectureDef,
    version: String,
    sla_file: PathBuf,
    processor_spec: PSpec,
    compiler_specs: Map<String, CSpec>,
}

impl Language {
    pub fn from_xml<P: AsRef<Path>>(root: P, input: xml::Node) -> Result<Self, DeserialiseError> {
        Self::from_xml_with(root, input, false)
    }

    pub fn from_xml_with<P: AsRef<Path>>(root: P, input: xml::Node, ignore_errors: bool) -> Result<Self, DeserialiseError> {
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

        let compiler_specs_it = input.children()
            .filter(|e| e.is_element() && e.tag_name().name() == "compiler")
            .map(|compiler| {
                let id = compiler.attribute_string("id")?;
                let name = compiler.attribute_string("name")?;

                let mut path = root.as_ref().to_path_buf();
                let cspec_path = compiler.attribute_string("spec")?;

                log::debug!("loading compiler specification `{}`", cspec_path);

                path.push(cspec_path);

                Ok((id, name, path))
            });

        let compiler_specs = if ignore_errors {
            compiler_specs_it.filter_map_ok(|(id, name, path)| {
                CSpec::named_from_file(name, &path).ok().map(|cspec| (id, cspec))
            })
            .collect::<Result<Map<_, _>, DeserialiseError>>()
        } else {
            compiler_specs_it.map(|res| res.and_then(|(id, name, path)| {
                CSpec::named_from_file(name, &path)
                    .map(|cspec| (id, cspec))
                    .map_err(|e| DeserialiseError::DeserialiseDepends {
                        path,
                        error: Box::new(e),
                    })
            }))
            .collect::<Result<Map<_, _>, DeserialiseError>>()
        }?;

        let architecture = ArchitectureDef::new(
            input.attribute_processor("processor")?,
            input.attribute_endian("endian")?,
            input.attribute_int("size")?,
            input.attribute_variant("variant")?,
        );

        log::debug!("loaded {} compiler conventions for {}", compiler_specs.len(), architecture);

        let mut path = root.as_ref().to_path_buf();
        let slafile_path = input.attribute_string("slafile")?;
        path.push(slafile_path);

        Ok(Self {
            id: input.attribute_string("id")?,
            architecture,
            version: input.attribute_string("version")?,
            sla_file: path,
            processor_spec,
            compiler_specs,
        })
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct LanguageBuilder<'a> {
    language: &'a Language,
}

impl<'a> LanguageBuilder<'a> {
    pub fn build(&self) -> Result<Translator, Error> {
        let mut translator =
            Translator::from_file(self.language.processor_spec.program_counter(),
                                  &self.language.architecture,
                                  &self.language.compiler_specs,
                                  &self.language.sla_file)?;
        for (name, val) in self.language.processor_spec.context_set() {
            translator
                .set_variable_default(name.as_ref(), val);
        }
        Ok(translator)
    }
}

#[derive(Debug, Default, Clone)]
#[repr(transparent)]
pub struct LanguageDB {
    db: Map<ArchitectureDef, Language>,
}

impl LanguageDB {
    pub fn lookup_default<'a, P: Into<String>>(
        &'a self,
        processor: P,
        endian: Endian,
        bits: usize,
    ) -> Option<LanguageBuilder<'a>> {
        self.db
            .get(&ArchitectureDef::new(processor, endian, bits, "default"))
            .map(|language| LanguageBuilder {
                language,
            })
    }

    pub fn lookup<'a, P: Into<String>, V: Into<String>>(
        &'a self,
        processor: P,
        endian: Endian,
        bits: usize,
        variant: V,
    ) -> Option<LanguageBuilder<'a>> {
        self.db
            .get(&ArchitectureDef::new(processor, endian, bits, variant))
            .map(|language| LanguageBuilder {
                language,
            })
    }

    pub fn definitions<'a>(&'a self) -> impl Iterator<Item = &'a ArchitectureDef> {
        self.db.keys()
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = LanguageBuilder<'a>> {
        self.db.iter().map(move |(_, language)| LanguageBuilder {
            language,
        })
    }

    fn into_iter(self) -> impl Iterator<Item=(ArchitectureDef, Language)> {
        self.db.into_iter()
    }

    pub fn len(&self) -> usize {
        self.db.len()
    }

    pub fn from_xml<P: AsRef<Path>>(root: P, input: xml::Node) -> Result<Self, DeserialiseError> {
        Self::from_xml_with(root, input, false)
    }

    pub fn from_xml_with<P: AsRef<Path>>(root: P, input: xml::Node, ignore_errors: bool) -> Result<Self, DeserialiseError> {
        if input.tag_name().name() != "language_definitions" {
            return Err(DeserialiseError::TagUnexpected(
                input.tag_name().name().to_owned(),
            ));
        }

        let root = root.as_ref().to_path_buf();

        let defs = input.children()
            .filter(xml::Node::is_element)
            .filter(|t| t.tag_name().name() == "language")
            .map(|t| {
                let ldef = Language::from_xml_with(&root, t, ignore_errors)?;
                Ok((ldef.architecture.clone(), ldef))
            });

        Ok(Self {
            db: if ignore_errors {
                defs.filter_map(|t| t.ok()).collect()
            } else {
                defs.collect::<Result<_, DeserialiseError>>()?
            },
        })
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        Self::from_file_with(path, false)
    }

    pub fn from_file_with<P: AsRef<Path>>(path: P, ignore_errors: bool) -> Result<Self, Error> {
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

        Self::from_str_with(root, &input, ignore_errors).map_err(|error| Error::DeserialiseFile {
            path: path.to_owned(),
            error,
        })
    }

    pub fn from_str<P: AsRef<Path>, S: AsRef<str>>(
        root: P,
        input: S,
    ) -> Result<Self, DeserialiseError> {
        Self::from_str_with(root, input, false)
    }

    pub fn from_str_with<P: AsRef<Path>, S: AsRef<str>>(
        root: P,
        input: S,
        ignore_errors: bool,
    ) -> Result<Self, DeserialiseError> {
        let document = xml::Document::parse(input.as_ref()).map_err(DeserialiseError::Xml)?;

        Self::from_xml_with(root, document.root_element(), ignore_errors)
    }

    pub fn from_directory<P: AsRef<Path>>(directory: P) -> Result<Self, Error> {
        Self::from_directory_with(directory, false)
    }

    pub fn from_directory_with<P: AsRef<Path>>(directory: P, ignore_errors: bool) -> Result<Self, Error> {
        WalkDir::new(directory.as_ref())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file() &&
                    e.path().extension().map(|e| e == "ldefs").unwrap_or(false))
            .try_fold(Self::default(), |mut acc, ldef| {
                log::debug!("loading language definition from `{:?}`", ldef);
                match Self::from_file_with(ldef.path(), ignore_errors) {
                    Ok(db) => { acc.db.extend(db.into_iter()); Ok(acc) },
                    Err(_) if ignore_errors => Ok(acc),
                    Err(e) => Err(e),
                }
            })
    }

}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_language_db_full() -> Result<(), Error> {
        let db = LanguageDB::from_directory("./data")?;
        assert_eq!(db.len(), 43);
        Ok(())
    }

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
