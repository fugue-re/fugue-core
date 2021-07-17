use std::convert::TryInto;
use std::fs::File;
use std::io::BufReader;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};

use fs_extra::file::{CopyOptions, copy as copy_file};

use capnp::message::{Reader, ReaderOptions, ReaderSegments, SegmentArray};
use capnp::serialize_packed::{read_message, write_message};

use fugue_ir::LanguageDB;

use fugue_ir::Translator;
use interval_tree::{Interval, IntervalTree};
use unicase::UniCase;
use url::Url;

use crate::architecture::{self, ArchitectureDef};
use crate::BasicBlock;
use crate::Endian;
use crate::ExportInfo;
use crate::Format;
use crate::Function;
use crate::Id;
use crate::Segment;

use crate::backend::{Backend, DatabaseImporterBackend, Imported};
use crate::error::Error;
use crate::schema;

#[ouroboros::self_referencing(chain_hack)]
#[derive(educe::Educe)]
#[educe(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DatabaseImpl {
    endian: Endian,
    format: Format,
    #[educe(Debug(ignore), PartialEq(ignore), Eq(ignore), Hash(ignore))]
    translators: Box<Vec<Translator>>,
    segments: Box<IntervalTree<u64, Segment>>,
    #[borrows(segments, translators)]
    #[covariant]
    functions: Vec<Function<'this>>,
    export_info: ExportInfo,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Database(DatabaseImpl);

impl Default for DatabaseImpl {
    fn default() -> Self {
        DatabaseImpl::new(
            Endian::Little,
            Format::ELF,
            Box::new(Vec::new()),
            Box::new(IntervalTree::from_iter(
                Vec::<(Interval<u64>, Segment)>::new(),
            )),
            |_, _| Vec::new(),
            ExportInfo::default(),
        )
    }
}

impl Database {
    pub fn default_with(endian: Endian) -> Self {
        Self(DatabaseImpl::new(
            endian,
            Format::ELF,
            Box::new(Vec::new()),
            Box::new(IntervalTree::from_iter(
                Vec::<(Interval<u64>, Segment)>::new(),
            )),
            |_, _| Vec::new(),
            ExportInfo::default(),
        ))
    }

    pub fn endian(&self) -> Endian {
        *self.0.borrow_endian()
    }

    pub fn format(&self) -> Format {
        *self.0.borrow_format()
    }

    pub fn architectures(&self) -> impl Iterator<Item=&ArchitectureDef> {
        self.0.borrow_translators().iter().map(Translator::architecture)
    }

    pub fn default_translator(&self) -> Translator {
        self.0.borrow_translators()
            .first()
            .map(|t| t.clone())
            .expect("default translator")
    }

    pub fn translators(&self) -> impl Iterator<Item=&Translator> {
        self.0.borrow_translators().iter()
    }

    pub fn segments(&self) -> &IntervalTree<u64, Segment> {
        self.0.borrow_segments()
    }

    pub fn segment<S: AsRef<str>>(&self, name: S) -> Option<&Segment> {
        let name = name.as_ref();
        self.segments().values().find(|s| s.name() == name)
    }

    pub fn functions(&self) -> &[Function] {
        self.0.borrow_functions()
    }

    pub fn functions_in<S: AsRef<str>>(
        &self,
        segment: S,
    ) -> Option<impl Iterator<Item = &Function>> {
        let name = segment.as_ref();
        if let Some(id) = self.segments().values().position(|s| s.name() == name) {
            let id = Id::from(id);
            Some(
                self.functions()
                    .iter()
                    .filter(move |f| f.segment_id() == id),
            )
        } else {
            None
        }
    }

    pub fn function_with<F>(&self, f: F) -> Option<&Function>
    where
        F: FnMut(&Function) -> bool,
    {
        let mut f = f;
        self.0.borrow_functions().iter().find(|&fun| f(fun))
    }

    pub fn function<S: AsRef<str>>(&self, name: S) -> Option<&Function> {
        let name = name.as_ref();
        self.0.borrow_functions().iter().find(|f| f.name() == name)
    }

    pub fn externals(&self) -> Option<impl Iterator<Item = &Function>> {
        self.functions_in(".extern") // Binary Ninja
            .or_else(|| self.functions_in("extern")) // IDA Pro
            .or_else(|| self.functions_in("EXTERNAL")) // Ghidra
    }

    pub fn blocks(&self) -> impl Iterator<Item = &BasicBlock> {
        self.functions().iter().map(Function::blocks).flatten()
    }

    pub fn block_count(&self) -> usize {
        self.functions().iter().map(|f| f.blocks().len()).sum()
    }

    pub fn edge_count(&self) -> usize {
        self.functions()
            .iter()
            .map(|f| {
                f.blocks()
                    .iter()
                    .map(|b| b.predecessors().len())
                    .sum::<usize>()
                    + f.references().len()
            })
            .sum()
    }

    pub fn blocks_in<S: AsRef<str>>(
        &self,
        name: S,
    ) -> Option<impl Iterator<Item = (&BasicBlock, &[u8])>> {
        let name = name.as_ref();
        if let Some(segment) = self.segments().values().find(|s| s.name() == name) {
            Some(self.blocks().filter_map(move |b| {
                if b.segment() == segment {
                    Some((b, b.bytes()))
                } else {
                    None
                }
            }))
        } else {
            None
        }
    }

    pub fn export_info(&self) -> &ExportInfo {
        self.0.borrow_export_info()
    }

    pub fn from_segments(segments: Vec<Vec<u8>>, language_db: &LanguageDB) -> Result<Self, Error> {
        let segment_refs = segments.iter().map(|seg| seg.as_ref()).collect::<Vec<_>>();
        let segment_array = SegmentArray::new(&segment_refs);

        let mut options = ReaderOptions::new();
        options.traversal_limit_in_words(None);

        let reader = Reader::new(segment_array, options);

        Self::from_reader(reader, language_db)
    }

    pub fn from_file<P: AsRef<Path>>(path: P, language_db: &LanguageDB) -> Result<Self, Error> {
        let path = path.as_ref();
        let file = BufReader::new(File::open(path).map_err(Error::CannotReadFile)?);

        let mut options = ReaderOptions::new();
        options.traversal_limit_in_words(None);

        let reader = read_message(file, options).map_err(Error::Deserialisation)?;

        Self::from_reader(reader, language_db)
    }

    fn from_reader<S: ReaderSegments>(reader: Reader<S>, language_db: &LanguageDB) -> Result<Self, Error> {
        let database = reader
            .get_root::<schema::database::Reader>()
            .map_err(Error::Deserialisation)?;

        let endian = Endian::from(if database.get_endian() {
            Endian::Big
        } else {
            Endian::Little
        });
        let format = database
            .get_format()
            .map_err(Error::Deserialisation)?
            .try_into()?;

        let export_info =
            ExportInfo::from_reader(database.get_export_info().map_err(Error::Deserialisation)?)?;

        let architectures = database
            .get_architectures()
            .map_err(Error::Deserialisation)?
            .into_iter()
            .map(architecture::from_reader)
            .collect::<Result<Vec<_>, _>>()?;

        let translators = architectures
            .into_iter()
            .map(|arch| {
                language_db
                    .lookup(arch.processor(), arch.endian(), arch.bits(), arch.variant())
                    .ok_or_else(|| Error::UnsupportedArchitecture(arch))?
                    .build()
                    .map_err(Error::Translator)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let segments = database
            .get_segments()
            .map_err(Error::Deserialisation)?
            .into_iter()
            .map(|r| {
                let seg = Segment::from_reader(r)?;
                Ok((seg.address()..=seg.address() + (seg.len() as u64 - 1), seg))
            })
            .collect::<Result<IntervalTree<_, _>, Error>>()?;

        Ok(Self(DatabaseImpl::try_new(
            endian,
            format,
            Box::new(translators),
            Box::new(segments),
            |segments, translators| {
                database
                    .get_functions()
                    .map_err(Error::Deserialisation)?
                    .into_iter()
                    .map(|r| Function::from_reader(r, segments, translators))
                    .collect::<Result<Vec<_>, _>>()
            },
            export_info,
        )?))
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
        let mut architectures = builder
            .reborrow()
            .init_architectures(self.0.borrow_translators().len() as u32);
        self.architectures()
            .enumerate()
            .try_for_each(|(i, a)| {
                let mut builder = architectures.reborrow().get(i as u32);
                architecture::to_builder(a, &mut builder)
            })?;
        let mut segments = builder
            .reborrow()
            .init_segments(self.segments().len() as u32);
        self.segments()
            .values()
            .enumerate()
            .try_for_each(|(i, s)| {
                let mut builder = segments.reborrow().get(i as u32);
                s.to_builder(&mut builder)
            })?;
        let mut functions = builder
            .reborrow()
            .init_functions(self.functions().len() as u32);
        self.functions().iter().enumerate().try_for_each(|(i, f)| {
            let mut builder = functions.reborrow().get(i as u32);
            f.to_builder(&mut builder)
        })?;
        self.export_info()
            .to_builder(&mut builder.reborrow().init_export_info())?;
        Ok(())
    }
}

pub struct DatabaseImporter {
    program: Option<url::Url>,
    fdb_path: Option<PathBuf>,
    overwrite_fdb: bool,
    backend_pref: Option<String>,
    backends: Vec<DatabaseImporterBackend>,
}

impl Default for DatabaseImporter {
    fn default() -> Self {
        Self {
            program: None,
            fdb_path: None,
            overwrite_fdb: false,
            backend_pref: None,
            backends: Vec::default(),
        }
    }
}

impl DatabaseImporter {
    pub fn new<P: AsRef<Path>>(program: P) -> DatabaseImporter {
        Self::new_url(Self::url_from_path(program))
    }

    pub fn new_url<U: Into<Url>>(program_url: U) -> DatabaseImporter {
        Self {
            program: Some(program_url.into()),
            ..Default::default()
        }
    }

    fn url_from_path<P: AsRef<Path>>(path: P) -> Url {
        let path = path.as_ref();
        if path.is_absolute() {
            Url::from_file_path(path).unwrap()
        } else {
            let apath = std::env::current_dir()
                .unwrap()
                .join(path);
            Url::from_file_path(apath).unwrap()
        }
    }

    pub fn available_backends(&self) -> impl Iterator<Item=&DatabaseImporterBackend> {
        self.backends.iter()
    }

    pub fn prefer_backend<N: Into<String>>(&mut self, backend: N) -> &mut Self {
        self.backend_pref = Some(backend.into());
        self
    }

    pub fn register_backend<B, E>(&mut self, backend: B) -> &mut Self
    where B: Backend<Error = E> + 'static,
          E: Into<Error> + 'static {
        self.backends.push(DatabaseImporterBackend::new(backend));
        self
    }

    pub fn program<P: AsRef<Path>>(&mut self, program: P) -> &mut Self {
        self.program = Some(Self::url_from_path(program));
        self
    }

    pub fn remote<U: Into<Url>>(&mut self, url: U) -> &mut Self {
        self.program = Some(url.into());
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

    pub fn import(&self, language_db: &LanguageDB) -> Result<Database, Error> {
        let program = if let Some(ref program) = self.program {
            program.clone()
        } else {
            return Err(Error::NoImportUrl)
        };

        if let Some(ref fdb_path) = self.fdb_path {
            if fdb_path.exists() && !self.overwrite_fdb {
                return Err(Error::ExportPathExists(fdb_path.to_owned()))
            }
        }

        if program.scheme() == "file" {
            let program = program.to_file_path()
                .map_err(|_| Error::InvalidLocalImportUrl(program.clone()))?;

            // importing from an existing database
            if program
                .extension()
                .map(|e| e == "fdb")
                .unwrap_or(false)
            {
                if let Ok(db) = Database::from_file(&program, language_db) {
                    return Ok(db);
                }
            };
        }

        let mut backends = self.available_backends()
            .filter_map(|b| if !b.is_available() {
                None
            } else if let Some(pref) = b.is_preferred_for(&program) {
                Some((if pref { 5 } else { 1 }, b))
            } else {
                None
            })
            .collect::<Vec<_>>();

        if backends.is_empty() {
            return Err(Error::NoBackendsAvailable);
        }

        backends.sort_by_key(|(base_score, b)| {
            -if let Some(ref pref) = self.backend_pref {
                *base_score + if UniCase::new(pref) == UniCase::new(b.name()) { 1 } else { 0 }
            } else {
                *base_score
            }
        });

        let mut res = Err(Error::NoBackendsAvailable);
        for (_, backend) in backends {
            res = backend.import(&program);
            if res.is_ok() {
                break
            }
        }

        match res {
            Ok(Imported::File(ref path)) => {
                let db = Database::from_file(path, language_db)?;
                if let Some(ref fdb_path) = self.fdb_path {
                    if path != fdb_path { // copy it
                        copy_file(path, fdb_path, &CopyOptions {
                            overwrite: true,
                            skip_exist: false,
                            ..Default::default()
                        })
                        .map_err(Error::ExportViaCopy)?;
                    }
                }
                Ok(db)
            },
            Ok(Imported::Segments(segments)) => {
                let db = Database::from_segments(segments, language_db)?;
                if let Some(ref fdb_path) = self.fdb_path {
                    db.to_file(fdb_path)?;
                }
                Ok(db)
            },
            Err(e) => Err(e),
        }
    }
}
