use std::convert::TryInto;
use std::fs::File;
use std::io::BufReader;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};

use capnp::serialize_packed::{read_message, write_message};
use capnp::message::ReaderOptions;

use interval_tree::{Interval, IntervalTree};

use crate::architecture::{self, Architecture};
use crate::BasicBlock;
use crate::Endian;
use crate::ExportInfo;
use crate::Format;
use crate::Function;
use crate::Id;
use crate::Segment;

use crate::backend;
use crate::error::Error;
use crate::schema;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Database {
    endian: Endian,
    format: Format,
    architectures: Vec<Architecture>,
    segments: IntervalTree<u64, Segment>,
    functions: Vec<Function>,
    export_info: ExportInfo,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            endian: Endian::Little,
            format: Format::ELF,
            architectures: Vec::new(),
            segments: IntervalTree::from_iter(Vec::<(Interval<u64>, Segment)>::new()),
            functions: Vec::new(),
            export_info: ExportInfo::default(),
        }
    }
}

impl Database {
    pub fn default_with(endian: Endian) -> Self {
        Self {
            endian,
            ..Default::default()
        }
    }

    pub fn endian(&self) -> Endian {
        self.endian
    }

    pub fn format(&self) -> Format {
        self.format
    }

    pub fn architectures(&self) -> &[Architecture] {
        &self.architectures
    }

    pub fn segments(&self) -> &IntervalTree<u64, Segment> {
        &self.segments
    }

    pub fn segment<S: AsRef<str>>(&self, name: S) -> Option<&Segment> {
        let name = name.as_ref();
        self.segments().values().find(|s| s.name() == name)
    }

    pub fn functions(&self) -> &[Function] {
        &self.functions
    }

    pub fn functions_in<S: AsRef<str>>(&self, segment: S) -> Option<impl Iterator<Item=&Function>> {
        let name = segment.as_ref();
        if let Some(id) = self.segments().values().position(|s| s.name() == name) {
            let id = Id::from(id);
            Some(self.functions().iter().filter(move |f| f.segment_id() == id))
        } else {
            None
        }
    }

    pub fn function_with<F>(&self, f: F) -> Option<&Function>
    where F: FnMut(&Function) -> bool {
        let mut f = f;
        self.functions.iter().find(|&fun| f(fun))
    }

    pub fn function<S: AsRef<str>>(&self, name: S) -> Option<&Function> {
        let name = name.as_ref();
        self.functions.iter().find(|f| f.name() == name)
    }

    pub fn externals(&self) -> Option<impl Iterator<Item=&Function>> {
        self.functions_in(".extern") // Binary Ninja
            .or_else(|| self.functions_in("extern")) // IDA Pro
            .or_else(|| self.functions_in("EXTERNAL")) // Ghidra
    }

    pub fn blocks(&self) -> impl Iterator<Item=&BasicBlock> {
        self.functions().iter().map(Function::blocks).flatten()
    }

    pub fn block_count(&self) -> usize {
        self.functions().iter().map(|f| f.blocks().len()).sum()
    }

    pub fn edge_count(&self) -> usize {
        self.functions()
            .iter()
            .map(|f| f.blocks().iter().map(|b| b.predecessors().len()).sum::<usize>() + f.references().len())
            .sum()
    }

    pub fn blocks_in<S: AsRef<str>>(&self, name: S) -> Option<impl Iterator<Item=(&BasicBlock, &[u8])>> {
        let name = name.as_ref();
        if let Some(id) = self.segments().values().position(|s| s.name() == name) {
            let id = Id::from(id);
            Some(self.blocks().filter_map(move |b| if b.segment_id() == id {
                Some((b, b.bytes(self)))
            } else {
                None
            }))
        } else {
            None
        }
    }

    pub fn export_info(&self) -> &ExportInfo {
        &self.export_info
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let path = path.as_ref();
        let file = BufReader::new(File::open(path).map_err(Error::CannotReadFile)?);
        let mut options = ReaderOptions::new();
        options.traversal_limit_in_words(None);

        let reader = read_message(file, options).map_err(Error::Deserialisation)?;
        let database = reader.get_root::<schema::database::Reader>()
            .map_err(Error::Deserialisation)?;

        let endian = Endian::from(if database.get_endian() { Endian::Big } else { Endian::Little });
        let format = database.get_format().map_err(Error::Deserialisation)?.try_into()?;

        let export_info = ExportInfo::from_reader(database.get_export_info().map_err(Error::Deserialisation)?)?;

        let architectures = database.get_architectures().map_err(Error::Deserialisation)?
            .into_iter()
            .map(architecture::from_reader)
            .collect::<Result<Vec<_>, _>>()?;

        let segments = database.get_segments().map_err(Error::Deserialisation)?
            .into_iter()
            .map(|r| {
                let seg = Segment::from_reader(r)?;
                Ok((seg.address()..=seg.address() + (seg.len() as u64 - 1), seg))
            })
            .collect::<Result<IntervalTree<_, _>, Error>>()?;

        let functions = database.get_functions().map_err(Error::Deserialisation)?
            .into_iter()
            .map(|r| Function::from_reader(r, &segments))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            endian,
            format,
            architectures,
            segments,
            functions,
            export_info,
        })
    }

    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let path = path.as_ref();
        let mut file = File::create(path).map_err(Error::CannotWriteFile)?;
        let mut message = capnp::message::Builder::new_default();
        let mut builder = message.init_root::<schema::database::Builder>();
        self.to_builder(&mut builder)?;
        write_message(&mut file, &mut message).map_err(Error::Serialisation)?;
        Ok(())
    }

    pub(crate) fn to_builder(&self, builder: &mut schema::database::Builder) -> Result<(), Error> {
        builder.set_endian(self.endian().is_big());
        builder.set_format(self.format().into());
        let mut architectures = builder.reborrow().init_architectures(self.architectures.len() as u32);
        self.architectures.iter().enumerate().try_for_each(|(i, a)| {
            let mut builder = architectures.reborrow().get(i as u32);
            architecture::to_builder(a, &mut builder)
        })?;
        let mut segments = builder.reborrow().init_segments(self.segments.len() as u32);
        self.segments.values().enumerate().try_for_each(|(i, s)| {
            let mut builder = segments.reborrow().get(i as u32);
            s.to_builder(&mut builder)
        })?;
        let mut functions = builder.reborrow().init_functions(self.functions.len() as u32);
        self.functions.iter().enumerate().try_for_each(|(i, f)| {
            let mut builder = functions.reborrow().get(i as u32);
            f.to_builder(&mut builder)
        })?;
        self.export_info().to_builder(&mut builder.reborrow().init_export_info())?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DatabaseImporter<'a, B: backend::Backend> {
    backend: &'a B,
    program: PathBuf,
    idb_path: Option<PathBuf>,
    fdb_path: Option<PathBuf>,
    rebase: Option<u64>,
    rebase_relative: i32,
    overwrite_fdb: bool,
}

impl<'a, B> DatabaseImporter<'a, B> where B: backend::Backend {
    pub fn new<P: AsRef<Path>>(backend: &'a B, program: P) -> DatabaseImporter<'a, B> {
        Self {
            backend,
            program: program.as_ref().to_owned(),
            idb_path: None,
            fdb_path: None,
            rebase: None,
            rebase_relative: 0,
            overwrite_fdb: false,
        }
    }

    pub fn program<P: AsRef<Path>>(&mut self, program: P) -> &mut Self {
        self.program = program.as_ref().to_owned();
        self
    }

    pub fn database<P: AsRef<Path>>(&mut self, database: P) -> &mut Self {
        self.idb_path = Some(database.as_ref().to_owned());
        self
    }

    pub fn rebase(&mut self, to: u64) -> &mut Self {
        self.rebase = Some(to);
        self.rebase_relative = 0;
        self
    }

    pub fn rebase_delta(&mut self, delta: i64) -> &mut Self {
        self.rebase = Some(delta.abs() as u64);
        self.rebase_relative = delta.signum() as i32;
        self
    }

    pub fn export_to<P: AsRef<Path>>(&mut self, database: P) -> &mut Self {
        self.fdb_path = Some(database.as_ref().to_owned());
        self
    }

    pub fn overwrite(&mut self, overwrite: bool) -> &mut Self {
        self.overwrite_fdb = overwrite;
        self
    }

    pub fn import(&self) -> Result<Database, Error> {
        if !self.program.exists() {
            return Err(Error::FileNotFound(self.program.clone()))
        }

        let (idb_path, fdb_path) = if self.idb_path.is_none() || self.fdb_path.is_none() {
            let tmpdir = tempfile::tempdir().map_err(Error::CannotCreateTempDir)?.into_path();
            let idb_path = if let Some(idb_path) = &self.idb_path {
                idb_path.to_owned()
            } else {
                tmpdir.join("backend.db")
            };

            let fdb_path = if let Some(fdb_path) = &self.fdb_path {
                fdb_path.to_owned()
            } else {
                tmpdir.join("exported.fdb")
            };
            (idb_path, fdb_path)
        } else {
            let idb_path = self.idb_path.as_ref().unwrap().clone();
            let fdb_path = self.fdb_path.as_ref().unwrap().clone();
            (idb_path, fdb_path)
        };

        self.backend.import_full(&self.program,
                                 idb_path,
                                 &fdb_path,
                                 self.overwrite_fdb,
                                 self.rebase,
                                 self.rebase_relative)?;

        Database::from_file(fdb_path)
    }
}
