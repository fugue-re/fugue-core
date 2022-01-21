use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::iter::FromIterator;
use std::path::{Path, PathBuf};

use fs_extra::file::{CopyOptions, copy as copy_file};

use fugue_ir::LanguageDB;

use fugue_ir::Translator;
use intervals::Interval;
use intervals::collections::IntervalTree;
use unicase::UniCase;
use url::Url;

use crate::architecture::{self, ArchitectureDef};
use crate::BasicBlock;
use crate::Function;
use crate::Id;
use crate::Metadata;
use crate::Segment;

use crate::backend::{Backend, DatabaseImporterBackend, Imported};
use crate::error::Error;
use crate::schema;

#[ouroboros::self_referencing(chain_hack)]
#[derive(educe::Educe)]
#[educe(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DatabaseImpl {
    #[educe(Debug(ignore), PartialEq(ignore), Eq(ignore), Hash(ignore))]
    translators: Box<Vec<Translator>>,
    segments: Box<IntervalTree<u64, Segment>>,
    #[borrows(segments, translators)]
    #[covariant]
    functions: Vec<Function<'this>>,
    metadata: Metadata,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Database(DatabaseImpl);

impl Default for DatabaseImpl {
    fn default() -> Self {
        DatabaseImpl::new(
            Box::new(Vec::new()),
            Box::new(IntervalTree::from_iter(
                Vec::<(Interval<u64>, Segment)>::new(),
            )),
            |_, _| Vec::new(),
            Metadata::default(),
        )
    }
}

impl Database {
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

    pub fn metadata(&self) -> &Metadata {
        self.0.borrow_metadata()
    }

    pub fn from_bytes(bytes: &[u8], language_db: &LanguageDB) -> Result<Self, Error> {
        let reader = schema::root_as_project(&bytes)
            .map_err(Error::Deserialisation)?;

        Self::from_reader(reader, language_db)
    }

    pub fn from_file<P: AsRef<Path>>(path: P, language_db: &LanguageDB) -> Result<Self, Error> {
        let path = path.as_ref();
        let mut file = BufReader::new(File::open(path).map_err(Error::CannotReadFile)?);

        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(Error::CannotReadFile)?;

        Self::from_bytes(&bytes, language_db)
    }

    fn from_reader<'a>(database: schema::Project<'a>, language_db: &LanguageDB) -> Result<Self, Error> {
        let metadata = Metadata::from_reader(
            database.metadata()
                .ok_or(Error::DeserialiseField("metadata"))?
        )?;

        let architectures = database.architectures()
            .ok_or(Error::DeserialiseField("architectures"))?
            .into_iter()
            .map(|r| architecture::from_reader(&r))
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

        let segments = database.segments()
            .ok_or(Error::DeserialiseField("segments"))?
            .into_iter()
            .filter_map(|r| match Segment::from_reader(&r) {
                Ok(seg) if seg.len() != 0 => {
                    Some(Ok((seg.address()..=seg.address() + (seg.len() as u64 - 1), seg)))
                },
                Ok(_) => None,
                Err(e) => Some(Err(e)),
            })
            .collect::<Result<IntervalTree<_, _>, Error>>()?;

        Ok(Self(DatabaseImpl::try_new(
            Box::new(translators),
            Box::new(segments),
            |segments, translators| {
                database.functions()
                    .ok_or(Error::DeserialiseField("functions"))?
                    .into_iter()
                    .map(|r| Function::from_reader(r, segments, translators))
                    .collect::<Result<Vec<_>, _>>()
            },
            metadata,
        )?))
    }

    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let path = path.as_ref();
        let mut file = File::create(path).map_err(Error::CannotWriteFile)?;

        let mut builder = flatbuffers::FlatBufferBuilder::new();

        let project = self.to_builder(&mut builder)?;
        schema::finish_project_buffer(&mut builder, project);

        file.write_all(builder.finished_data()).map_err(Error::CannotWriteFile)?;

        Ok(())
    }

    pub(crate) fn to_builder<'a: 'b, 'b>(
        &self,
        builder: &'b mut flatbuffers::FlatBufferBuilder<'a>
    ) -> Result<flatbuffers::WIPOffset<schema::Project<'a>>, Error> {
        let architectures = self.architectures()
            .map(|r| architecture::to_builder(r, builder))
            .collect::<Result<Vec<_>, _>>()?;
        let avec = builder.create_vector_from_iter(architectures.into_iter());

        let segments = self.segments()
            .values()
            .map(|r| r.to_builder(builder))
            .collect::<Result<Vec<_>, _>>()?;
        let svec = builder.create_vector_from_iter(segments.into_iter());

        let functions = self.functions()
            .iter()
            .map(|r| r.to_builder(builder))
            .collect::<Result<Vec<_>, _>>()?;
        let fvec = builder.create_vector_from_iter(functions.into_iter());

        let meta = self.metadata().to_builder(builder)?;

        let mut dbuilder = schema::ProjectBuilder::new(builder);

        dbuilder.add_architectures(avec);
        dbuilder.add_segments(svec);
        dbuilder.add_functions(fvec);
        dbuilder.add_metadata(meta);

        Ok(dbuilder.finish())
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

        // Try all backends
        let mut res = Err(Error::NoBackendsAvailable);
        for (_, backend) in backends {
            // log::debug!("Trying backend {}", backend.name());
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
            Ok(Imported::Bytes(ref bytes)) => {
                let db = Database::from_bytes(bytes, language_db)?;
                if let Some(ref fdb_path) = self.fdb_path {
                    db.to_file(fdb_path)?;
                }
                Ok(db)
            },
            Err(e) => Err(e),
        }
    }
}
