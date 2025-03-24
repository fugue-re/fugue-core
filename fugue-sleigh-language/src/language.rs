use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ahash::AHashMap as Map;
use fugue_arch::{ArchDefParseError, ArchitectureDef};
use fugue_bytes::Endian;
use fugue_ghidra_marshal::sla::*;
use fugue_ghidra_marshal::Decoder;
use itertools::Itertools;
use thiserror::Error;
use ustr::Ustr;
use walkdir::WalkDir;

use crate::compiler::CompilerSpec;
use crate::convention::Convention;
use crate::deserialise::{DeserialiseError, XmlExt};
use crate::float_format::{FloatFormat, FloatFormats};
use crate::processor::ProcessorSpec;
use crate::register::RegisterNames;
use crate::spaces::AddressSpaces;
use crate::symbol::{Symbol, SymbolScope, SymbolTable};
use crate::varnode::VarnodeData;

pub type UserOpStr = Ustr;

#[derive(Debug, Error)]
pub enum LanguageError {
    #[error("cannot deserialise file `{}`: {}", path.display(), error)]
    DeserialiseFile {
        path: PathBuf,
        error: crate::deserialise::DeserialiseError,
    },
    #[error("cannot parse from file `{}`: {}", path.display(), error)]
    ParseFile { path: PathBuf, error: io::Error },
}

// Language is used for parsing the processor spec XML and
// lifting instructions
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct Language {
    alignment: usize,
    big_endian: bool,
    unique_base: u64,
    unique_mask: u64,
    maximum_delay: usize,
    section_count: usize,
    float_formats: FloatFormats,
    spaces: AddressSpaces,
    symbol_table: SymbolTable,
    root: Arc<Symbol>,
    global_scope: Arc<SymbolScope>,
    registers: Arc<RegisterNames>,
    registers_size: usize,
    program_counter: VarnodeData,
    user_ops: Vec<UserOpStr>,
    architecture: ArchitectureDef,
    compiler_conventions: Map<String, Convention>,
    source_files: Map<String, usize>,
}

impl Language {
    pub fn is_big_endian(&self) -> bool {
        self.big_endian
    }

    pub fn is_little_endian(&self) -> bool {
        !self.big_endian
    }

    pub fn alignment(&self) -> usize {
        self.alignment
    }

    pub fn unique_base(&self) -> u64 {
        self.unique_base
    }

    pub fn unique_mask(&self) -> u64 {
        self.unique_mask
    }

    pub fn float_formats(&self) -> &Map<usize, Arc<FloatFormat>> {
        &self.float_formats
    }

    pub fn float_format(&self, size: usize) -> Option<Arc<FloatFormat>> {
        self.float_formats.get(&size).cloned()
    }

    pub fn spaces(&self) -> &AddressSpaces {
        &self.spaces
    }

    pub fn registers(&self) -> &Arc<RegisterNames> {
        &self.registers
    }

    pub fn register_by_name<S: AsRef<str>>(&self, name: S) -> Option<VarnodeData> {
        self.registers
            .get_by_name(name.as_ref())
            .map(|(_, offset, size)| {
                VarnodeData::new(&self.registers.register_space(), offset, size)
            })
    }

    pub fn register_space_size(&self) -> usize {
        self.registers_size
    }

    pub fn unique_space_size(&self) -> usize {
        // base is first free offset
        self.unique_base as usize
    }

    pub fn root_symbol(&self) -> &Symbol {
        &*self.root
    }

    pub fn symbol_table(&self) -> &SymbolTable {
        &self.symbol_table
    }

    pub fn user_ops(&self) -> &[UserOpStr] {
        &self.user_ops
    }

    pub fn architecture(&self) -> &ArchitectureDef {
        &self.architecture
    }

    pub fn program_counter(&self) -> &VarnodeData {
        &self.program_counter
    }

    pub fn compiler_conventions(&self) -> &Map<String, Convention> {
        &self.compiler_conventions
    }

    pub fn source_files(&self) -> &Map<String, usize> {
        &self.source_files
    }

    pub fn from_file<PC: AsRef<str>, P: AsRef<Path>>(
        program_counter: PC,
        architecture: &ArchitectureDef,
        compiler_specs: &Map<String, CompilerSpec>,
        path: P,
    ) -> Result<Self, LanguageError> {
        let path = path.as_ref();
        let input = fs::read_to_string(path).map_err(|error| LanguageError::ParseFile {
            path: path.to_owned(),
            error,
        })?;

        Self::from_str(program_counter, architecture, compiler_specs, &input).map_err(|error| {
            LanguageError::DeserialiseFile {
                path: path.to_owned(),
                error,
            }
        })
    }

    pub fn from_str<PC: AsRef<str>, S: AsRef<str>>(
        program_counter: PC,
        architecture: &ArchitectureDef,
        compiler_specs: &Map<String, CompilerSpec>,
        input: S,
    ) -> Result<Self, DeserialiseError> {
        let document = xml::Document::parse(input.as_ref()).map_err(DeserialiseError::Xml)?;

        Self::from_xml(
            program_counter,
            architecture,
            compiler_specs,
            document.root_element(),
        )
    }

    fn build_xrefs<PC: AsRef<str>>(
        &mut self,
        program_counter: PC,
        compiler_specs: &Map<String, CompilerSpec>,
    ) -> Result<(), DeserialiseError> {
        let registers = Arc::<RegisterNames>::get_mut(&mut self.registers)
            .expect("unique access to RegisterNames");

        let user_ops = &mut self.user_ops;
        let mut pc = None;
        let mut registers_size = 0;

        let pc_name = program_counter.as_ref();

        for sym_id in self.global_scope.iter() {
            match self.symbol_table.symbol(*sym_id) {
                None => return Err(DeserialiseError::Invariant("invalid symbol")),
                Some(Symbol::Varnode {
                    name,
                    ref offset,
                    ref size,
                    ..
                }) => {
                    registers.insert(*offset, *size, name.clone());

                    if let Some(size) = size.checked_add(*offset as usize) {
                        registers_size = registers_size.max(size);
                    } else {
                        return Err(DeserialiseError::Invariant(
                            "offset with size of varnode overflows",
                        ));
                    }

                    if pc_name == name {
                        if pc.is_some() {
                            return Err(DeserialiseError::Invariant(
                                "duplicate definition of program counter",
                            ));
                        }
                        pc = Some((*offset, *size));
                    }
                }
                Some(Symbol::UserOp { index, name, .. }) => {
                    if user_ops.len() <= *index {
                        user_ops.resize_with(index + 1, Ustr::default);
                    }
                    user_ops[*index] = name.clone();
                }
                _ => (),
            }
        }

        if let Some((pc_offset, pc_size)) = pc {
            self.program_counter.offset = pc_offset;
            self.program_counter.size = pc_size as _;
        } else {
            return Err(DeserialiseError::Invariant(
                "program counter not defined as a register",
            ));
        }

        self.registers_size = registers_size;

        for (name, spec) in compiler_specs.iter() {
            let conv = Convention::from_spec(spec, &self.registers, &self.spaces)?;
            #[cfg(feature = "tracing")]
            tracing::debug!("loaded compiler convention `{}`", name);
            self.compiler_conventions.insert(name.clone(), conv);
        }

        Ok(())
    }

    pub fn from_decoder<D: Decoder>(
        program_counter: impl AsRef<str>,
        architecture: &ArchitectureDef,
        compiler_specs: &Map<String, CompilerSpec>,
        input: &mut D,
    ) -> Result<Self, DeserialiseError> {
        #[cfg(feature = "tracing")]
        tracing::trace!("decoding sleigh language definition");

        let sleigh = input.open_element_with_id(&ELEM_SLEIGH)?;

        let alignment = input.read_signed_integer_with_id(&ATTRIB_ALIGN)? as usize;
        let big_endian = input.read_bool_with_id(&ATTRIB_BIGENDIAN)?;
        let unique_base = input.read_unsigned_integer_with_id(&ATTRIB_UNIQBASE)?;
        let _version = input
            .try_read_signed_integer_with_id(&ATTRIB_VERSION)?
            .unwrap_or(4);

        let maximum_delay = input
            .try_read_signed_integer_with_id(&ATTRIB_MAXDELAY)?
            .unwrap_or(0) as usize;
        let unique_mask = input
            .try_read_unsigned_integer_with_id(&ATTRIB_UNIQMASK)?
            .unwrap_or(0);
        let section_count = input
            .try_read_signed_integer_with_id(&ATTRIB_NUMSECTIONS)?
            .unwrap_or(0) as usize;

        let mut source_files = Map::new();
        {
            #[cfg(feature = "tracing")]
            tracing::trace!("decoding sleigh language definition source files");

            let id = input.open_element_with_id(&ELEM_SOURCEFILES)?;

            while input.peek_element()? == ELEM_SOURCEFILE.id() {
                let id = input.open_element()?;

                let name = input.read_string_with_id(&ATTRIB_NAME)?;
                let index = input.read_signed_integer_with_id(&ATTRIB_INDEX)? as usize;

                source_files.insert(name, index);

                input.close_element(id)?;
            }

            input.close_element(id)?;
        }

        // NOTE: this seems to be missing now?
        let mut float_formats = Map::new();
        {
            float_formats.insert(16, Arc::new(FloatFormat::float2()));
            float_formats.insert(32, Arc::new(FloatFormat::float4()));
            float_formats.insert(64, Arc::new(FloatFormat::float8()));
            float_formats.insert(80, Arc::new(FloatFormat::float10()));
            float_formats.insert(128, Arc::new(FloatFormat::float16()));
        }

        #[cfg(feature = "tracing")]
        tracing::trace!("decoding address space definitions");
        let spaces = AddressSpaces::from_decoder(input)?;

        #[cfg(feature = "tracing")]
        tracing::trace!("decoding symbol table");
        let symbol_table = SymbolTable::from_decoder(&spaces, input)?;

        input.close_element(sleigh)?;

        let register_space = spaces.register_space();
        let program_counter_vnd = VarnodeData::new(&*register_space, 0, 0);

        let global_scope = Arc::new(
            symbol_table
                .global_scope()
                .ok_or_else(|| DeserialiseError::Invariant("global scope not defined"))?
                .to_owned(),
        );

        let root = Arc::new(
            symbol_table
                .global_scope()
                .ok_or_else(|| DeserialiseError::Invariant("global scope not defined"))?
                .find("instruction", &symbol_table)
                .ok_or_else(|| DeserialiseError::Invariant("instruction root symbol not defined"))?
                .to_owned(),
        );

        let mut slf = Self {
            alignment,
            big_endian,
            unique_base,
            unique_mask,
            maximum_delay,
            section_count,
            float_formats,
            spaces,
            symbol_table,
            root,
            global_scope,
            registers: Arc::new(RegisterNames::new(register_space)),
            registers_size: 0,
            program_counter: program_counter_vnd,
            user_ops: Vec::new(),
            architecture: architecture.clone(),
            compiler_conventions: Map::default(),
            source_files,
        };

        slf.build_xrefs(program_counter, compiler_specs)?;

        Ok(slf)
    }

    pub fn from_xml(
        program_counter: impl AsRef<str>,
        architecture: &ArchitectureDef,
        compiler_specs: &Map<String, CompilerSpec>,
        input: xml::Node,
    ) -> Result<Self, DeserialiseError> {
        if input.tag_name().name() != "sleigh" {
            return Err(DeserialiseError::TagUnexpected(
                input.tag_name().name().to_owned(),
            ));
        }

        let alignment = input.attribute_int("align")?;
        let big_endian = input.attribute_bool("bigendian")?;
        let unique_base = input.attribute_int("uniqbase")?;
        let version = input.attribute_int_opt("version", 2)?;

        let maximum_delay = input.attribute_int_opt("maxdelay", 0)?;
        let unique_mask = input.attribute_int_opt("uniqmask", 0)?;
        let section_count = input.attribute_int_opt("numsections", 0)?;

        let mut children = input.children().filter(xml::Node::is_element).peekable();

        let mut float_formats = children
            .peeking_take_while(|node| node.tag_name().name() == "floatformat")
            .map(|node| {
                let ff = Arc::new(FloatFormat::from_xml(node)?);
                Ok((ff.bits(), ff))
            })
            .collect::<Result<Map<_, _>, DeserialiseError>>()?;

        if float_formats.is_empty() {
            float_formats.insert(16, Arc::new(FloatFormat::float2()));
            float_formats.insert(32, Arc::new(FloatFormat::float4()));
            float_formats.insert(64, Arc::new(FloatFormat::float8()));
            float_formats.insert(80, Arc::new(FloatFormat::float10()));
            float_formats.insert(128, Arc::new(FloatFormat::float16()));
        }

        let mut source_files = Map::default();

        if version >= 3
            && matches!(
                children.peek().map(|node| node.tag_name().name()),
                Some("sourcefiles")
            )
        {
            let sources = children.next().unwrap();
            for source in sources.children().filter(xml::Node::is_element) {
                let name = source.attribute_string("name")?;
                let index = source.attribute_int("index")?;

                source_files.insert(name, index);
            }
        }

        let spaces = AddressSpaces::from_xml(
            children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("spaces not defined"))?,
        )?;

        let symbol_table = SymbolTable::from_xml(
            &spaces,
            children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("symbol table not defined"))?,
        )?;

        let register_space = spaces.register_space();
        let program_counter_vnd = VarnodeData::new(&*register_space, 0, 0);

        let global_scope = Arc::new(
            symbol_table
                .global_scope()
                .ok_or_else(|| DeserialiseError::Invariant("global scope not defined"))?
                .to_owned(),
        );

        let root = Arc::new(
            symbol_table
                .global_scope()
                .ok_or_else(|| DeserialiseError::Invariant("global scope not defined"))?
                .find("instruction", &symbol_table)
                .ok_or_else(|| DeserialiseError::Invariant("instruction root symbol not defined"))?
                .to_owned(),
        );

        let mut slf = Self {
            alignment,
            big_endian,
            unique_base,
            unique_mask,
            maximum_delay,
            section_count,
            float_formats,
            spaces,
            symbol_table,
            root,
            global_scope,
            registers: Arc::new(RegisterNames::new(register_space)),
            registers_size: 0,
            program_counter: program_counter_vnd,
            user_ops: Vec::new(),
            architecture: architecture.clone(),
            compiler_conventions: Map::default(),
            source_files,
        };

        slf.build_xrefs(program_counter, compiler_specs)?;

        Ok(slf)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageDef {
    id: String,
    architecture: ArchitectureDef,
    version: String,
    sla_file: PathBuf,
    processor_spec: ProcessorSpec,
    compiler_specs: Map<String, CompilerSpec>,
}

impl LanguageDef {
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

    pub fn processor_spec(&self) -> &ProcessorSpec {
        &self.processor_spec
    }

    pub fn compiler_specs(&self) -> &Map<String, CompilerSpec> {
        &self.compiler_specs
    }

    pub fn from_xml<P: AsRef<Path>>(root: P, input: xml::Node) -> Result<Self, DeserialiseError> {
        Self::from_xml_with(root, input, false)
    }

    /// Build LanguageDef object from each <language> tage specified in .ldef file
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
            ProcessorSpec::from_file(&path).map_err(|e| DeserialiseError::DeserialiseDepends {
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

                #[cfg(feature = "tracing")]
                tracing::debug!("loading compiler specification `{}`", cspec_path);

                path.push(cspec_path);

                Ok((id, name, path))
            });

        // Build compiler specs from .cspec file
        let compiler_specs = if ignore_errors {
            compiler_specs_it
                .filter_map_ok(|(id, name, path)| {
                    #[cfg(feature = "tracing")]
                    tracing::debug!("id: {}, name: {}, path: {:?}", id, name, path);
                    CompilerSpec::named_from_file(name, &path)
                        .ok()
                        .map(|cspec| (id, cspec))
                })
                .collect::<Result<Map<_, _>, DeserialiseError>>()
        } else {
            compiler_specs_it
                .map(|res| {
                    res.and_then(|(id, name, path)| {
                        CompilerSpec::named_from_file(name, &path)
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

        #[cfg(feature = "tracing")]
        tracing::debug!(
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

    pub fn context_set(&self) -> impl Iterator<Item = (&str, u32)> {
        self.processor_spec.context_set()
    }

    pub fn tracked_set(&self) -> impl Iterator<Item = (&str, u32)> {
        self.processor_spec.tracked_set()
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct LanguageDefBuilder<'a> {
    language: &'a LanguageDef,
}

impl<'a> LanguageDefBuilder<'a> {
    pub fn language(&self) -> &'a LanguageDef {
        self.language
    }

    pub fn build_with_sla(&self, sla: impl AsRef<Path>) -> Result<Language, LanguageError> {
        Language::from_file(
            self.language.processor_spec.program_counter(),
            &self.language.architecture,
            &self.language.compiler_specs,
            sla,
        )
    }

    pub fn build(&self) -> Result<Language, LanguageError> {
        self.build_with_sla(&self.language.sla_file)
    }
}

#[derive(Debug, Default, Clone)]
#[repr(transparent)]
pub struct LanguageDB {
    db: Map<ArchitectureDef, LanguageDef>,
}

impl LanguageDB {
    pub fn lookup_default<'a, P: Into<String>>(
        &'a self,
        processor: P,
        endian: Endian,
        bits: usize,
    ) -> Option<LanguageDefBuilder<'a>> {
        self.db
            .get(&ArchitectureDef::new(processor, endian, bits, "default"))
            .map(|language| LanguageDefBuilder { language })
    }

    pub fn lookup_str<'a, S: AsRef<str>>(
        &'a self,
        definition: S,
    ) -> Result<Option<LanguageDefBuilder<'a>>, ArchDefParseError> {
        let def = definition.as_ref().parse::<ArchitectureDef>()?;
        Ok(self
            .db
            .get(&def)
            .map(|language| LanguageDefBuilder { language }))
    }

    pub fn lookup<'a, P: Into<String>, V: Into<String>>(
        &'a self,
        processor: P,
        endian: Endian,
        bits: usize,
        variant: V,
    ) -> Option<LanguageDefBuilder<'a>> {
        self.db
            .get(&ArchitectureDef::new(processor, endian, bits, variant))
            .map(|language| LanguageDefBuilder { language })
    }

    pub fn definitions<'a>(&'a self) -> impl Iterator<Item = &'a ArchitectureDef> {
        self.db.keys()
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = LanguageDefBuilder<'a>> {
        self.db
            .iter()
            .map(move |(_, language)| LanguageDefBuilder { language })
    }

    fn into_iter(self) -> impl Iterator<Item = (ArchitectureDef, LanguageDef)> {
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
                let ldef = LanguageDef::from_xml_with(&root, t, ignore_errors)?;
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

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, LanguageError> {
        Self::from_file_with(path, false)
    }

    /// Build fugue language DB from XML file
    /// # Parameters
    /// path: the path to the .ldef file
    /// ignore_errors: Ignore parsing error
    pub fn from_file_with<P: AsRef<Path>>(
        path: P,
        ignore_errors: bool,
    ) -> Result<Self, LanguageError> {
        // Open file
        let path = path.as_ref();
        let mut file = File::open(path).map_err(|error| LanguageError::ParseFile {
            path: path.to_owned(),
            error,
        })?;

        // Read to string
        let mut input = String::new();
        file.read_to_string(&mut input)
            .map_err(|error| LanguageError::ParseFile {
                path: path.to_owned(),
                error,
            })?;

        // Obtain the folder that the spec is in
        let root = path
            .parent()
            .ok_or_else(|| {
                DeserialiseError::Invariant("cannot obtain parent directory of language defintions")
            })
            .map_err(|error| LanguageError::DeserialiseFile {
                path: path.to_owned(),
                error,
            })?;

        // Parse the string
        Self::from_str_with(root, &input, ignore_errors).map_err(|error| {
            LanguageError::DeserialiseFile {
                path: path.to_owned(),
                error,
            }
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

    pub fn from_directory<P: AsRef<Path>>(directory: P) -> Result<Self, LanguageError> {
        Self::from_directory_with(directory, false)
    }

    pub fn from_directory_with<P: AsRef<Path>>(
        directory: P,
        ignore_errors: bool,
    ) -> Result<Self, LanguageError> {
        WalkDir::new(directory.as_ref())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file()
                    && e.path().extension().map(|e| e == "ldefs").unwrap_or(false)
            })
            .try_fold(Self::default(), |mut acc, ldef| {
                #[cfg(feature = "tracing")]
                tracing::debug!("loading language definition from `{:?}`", ldef);
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

#[cfg(test)]
mod test {
    use std::fs::File;

    use fugue_arch::ArchitectureDef;
    use fugue_ghidra_marshal::sla::FormatDecoder;
    use fugue_ghidra_marshal::Decoder;
    use tracing_subscriber::prelude::*;

    use super::{Language, Map};

    #[test]
    fn test_packed_sla() -> Result<(), Box<dyn std::error::Error>> {
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer())
            .with(tracing_subscriber::filter::EnvFilter::from_default_env())
            .init();

        let mut decoder = FormatDecoder::new();

        decoder.ingest_stream(File::open("/Users/slt/Downloads/ghidra_11.2.1_PUBLIC/Ghidra/Processors/x86/data/languages/x86.sla")?)?;

        let _ = Language::from_decoder(
            "EIP",
            &"x86:LE:32:default".parse::<ArchitectureDef>()?,
            &Map::new(),
            &mut decoder,
        )?;

        Ok(())
    }
}
