use crate::deserialise::parse::XmlExt;
use crate::deserialise::Error as DeserialiseError;

use crate::endian::Endian;
use crate::error::Error;

use crate::compiler::Specification as CSpec;
use crate::processor::Specification as PSpec;
use crate::Translator;

use ahash::AHashMap as Map;
use fugue_arch::{ArchitectureDef, ArchDefParseError};
use itertools::Itertools;
use walkdir::WalkDir;

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use log;

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
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn architecture(&self) -> &ArchitectureDef {
        &self.architecture
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn sla_file(&self) -> &Path {
        &self.sla_file
    }

    pub fn from_xml<P: AsRef<Path>>(root: P, input: xml::Node) -> Result<Self, DeserialiseError> {
        Self::from_xml_with(root, input, false)
    }

    /// Build Language object from each <language> tage specified in .ldef file
    /// # Parameters
    /// root: the search directory for finding related files specified in the .ldef file
    /// input: the xml::Node object which have name language for language definiation
    /// ignore_errors: if ignore parsing errors
    pub fn from_xml_with<P: AsRef<Path>>(
        root: P,
        input: xml::Node,
        ignore_errors: bool,
    ) -> Result<Self, DeserialiseError> {
        // Check the correctness of the tag name
        if input.tag_name().name() != "language" {
            return Err(DeserialiseError::TagUnexpected(
                input.tag_name().name().to_owned(),
            ));
        }

        // Read path to the processor spec (.pspec) file
        let mut path = root.as_ref().to_path_buf();
        let pspec_path = input.attribute_string("processorspec")?;
        path.push(pspec_path);

        // Build processor spec from .pspec file
        let processor_spec =
            PSpec::from_file(&path).map_err(|e| DeserialiseError::DeserialiseDepends {
                path,
                error: Box::new(e),
            })?;

        // Read path to the compiler spec (.cspec) file
        // Each language can have several .cspec file
        let compiler_specs_it = input
            .children()
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

        // Build compiler specs from .cspec file
        let compiler_specs = if ignore_errors {
            compiler_specs_it
                .filter_map_ok(|(id, name, path)| {
                    log::debug!("id: {}, name: {}, path: {:?}", id, name, path);
                    CSpec::named_from_file(name, &path)
                        .ok()
                        .map(|cspec| (id, cspec))
                })
                .collect::<Result<Map<_, _>, DeserialiseError>>()
        } else {
            compiler_specs_it
                .map(|res| {
                    res.and_then(|(id, name, path)| {
                        CSpec::named_from_file(name, &path)
                            .map(|cspec| (id, cspec))
                            .map_err(|e| DeserialiseError::DeserialiseDepends {
                                path,
                                error: Box::new(e),
                            })
                    })
                })
                .collect::<Result<Map<_, _>, DeserialiseError>>()
        }?;

        // Obtain architecture information, enaian, word size, variant etc
        let architecture = ArchitectureDef::new(
            input.attribute_processor("processor")?,
            input.attribute_endian("endian")?,
            input.attribute_int("size")?,
            input.attribute_variant("variant")?,
        );

        log::debug!(
            "loaded {} compiler conventions for {}",
            compiler_specs.len(),
            architecture
        );

        // Read path to the .sla file
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
    pub fn language(&self) -> &'a Language {
        self.language
    }

    #[inline(always)]
    pub fn apply_context(&self, translator: &mut Translator) {
        for (name, val) in self.language.processor_spec.context_set() {
            translator.set_variable_default(name.as_ref(), val);
        }
    }

    pub fn build(&self) -> Result<Translator, Error> {
        let mut translator = Translator::from_file(
            self.language.processor_spec.program_counter(),
            &self.language.architecture,
            &self.language.compiler_specs,
            &self.language.sla_file,
        )?;

        self.apply_context(&mut translator);

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
            .map(|language| LanguageBuilder { language })
    }

    pub fn lookup_str<'a, S: AsRef<str>>(
        &'a self,
        definition: S,
    ) -> Result<Option<LanguageBuilder<'a>>, ArchDefParseError> {
        let def = definition.as_ref().parse::<ArchitectureDef>()?;
        Ok(self.db
            .get(&def)
            .map(|language| LanguageBuilder { language }))
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
            .map(|language| LanguageBuilder { language })
    }

    pub fn definitions<'a>(&'a self) -> impl Iterator<Item = &'a ArchitectureDef> {
        self.db.keys()
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = LanguageBuilder<'a>> {
        self.db
            .iter()
            .map(move |(_, language)| LanguageBuilder { language })
    }

    fn into_iter(self) -> impl Iterator<Item = (ArchitectureDef, Language)> {
        self.db.into_iter()
    }

    pub fn len(&self) -> usize {
        self.db.len()
    }

    pub fn from_xml<P: AsRef<Path>>(root: P, input: xml::Node) -> Result<Self, DeserialiseError> {
        Self::from_xml_with(root, input, false)
    }

    /// Build language DB from XML document
    /// # Parameters
    /// root: the search directory for finding related files specified in the .ldef file
    /// input: xml::Node object parsed from .ldef file using xml::Document::parse()
    /// ignore_errors: If we ignore parsing error
    pub fn from_xml_with<P: AsRef<Path>>(
        root: P,
        input: xml::Node,
        ignore_errors: bool,
    ) -> Result<Self, DeserialiseError> {
        // Example structure of .ldef file
        // <language_definitions>
        //   <language processor="MIPS"
        //   endian="big"
        //   size="32"
        //   variant="default"
        //   version="1.5"
        //   slafile="mips32be.sla"
        //   processorspec="mips32.pspec"
        //   manualindexfile="../manuals/mipsM16.idx"
        //   id="MIPS:BE:32:default">
        //  <description>MIPS32 32-bit addresses, big endian, with mips16e</description>
        //  <compiler name="default" spec="mips32.cspec" id="default"/>
        //  <compiler name="Visual Studio" spec="mips32.cspec" id="windows"/>
        //  <external_name tool="gnu" name="mips:4000"/>
        //  <external_name tool="IDA-PRO" name="mipsb"/>
        //  <external_name tool="DWARF.register.mapping.file" name="mips.dwarf"/>
        //  </language>
        // </language_definitions>
        if input.tag_name().name() != "language_definitions" {
            return Err(DeserialiseError::TagUnexpected(
                input.tag_name().name().to_owned(),
            ));
        }

        let root = root.as_ref().to_path_buf();

        // Go through each language tag and parse them
        let defs = input
            .children()
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

    /// Build fugue language DB from XML file
    /// # Parameters
    /// path: the path to the .ldef file
    /// ignore_errors: Ignore parsing error
    pub fn from_file_with<P: AsRef<Path>>(path: P, ignore_errors: bool) -> Result<Self, Error> {
        // Open file
        let path = path.as_ref();
        let mut file = File::open(path).map_err(|error| Error::ParseFile {
            path: path.to_owned(),
            error,
        })?;

        // Read to string
        let mut input = String::new();
        file.read_to_string(&mut input)
            .map_err(|error| Error::ParseFile {
                path: path.to_owned(),
                error,
            })?;

        // Obtain the folder that the spec is in
        let root = path
            .parent()
            .ok_or_else(|| {
                DeserialiseError::Invariant("cannot obtain parent directory of language defintions")
            })
            .map_err(|error| Error::DeserialiseFile {
                path: path.to_owned(),
                error,
            })?;

        // Parse the string
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

    /// Build the language DB from the XML string
    /// # Parameters
    /// root: the search directory for finding related files specified in the .ldef file
    /// input: .ldef file read as string
    /// ignore_errors: If we ignore parsing errors
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

    pub fn from_directory_with<P: AsRef<Path>>(
        directory: P,
        ignore_errors: bool,
    ) -> Result<Self, Error> {
        WalkDir::new(directory.as_ref())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file()
                    && e.path().extension().map(|e| e == "ldefs").unwrap_or(false)
            })
            .try_fold(Self::default(), |mut acc, ldef| {
                log::debug!("loading language definition from `{:?}`", ldef);
                match Self::from_file_with(ldef.path(), ignore_errors) {
                    Ok(db) => {
                        acc.db.extend(db.into_iter());
                        Ok(acc)
                    }
                    Err(_) if ignore_errors => Ok(acc),
                    Err(e) => Err(e),
                }
            })
    }
}
