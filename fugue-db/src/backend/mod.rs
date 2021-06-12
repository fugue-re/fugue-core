use std::{ops::Deref, path::Path};

pub trait Backend {
    type Error: Into<crate::Error>;

    fn name(&self) -> &'static str;

    fn is_available(&self) -> bool;
    fn is_preferred_for(&self, path: &Path) -> bool;

    fn import_full(
        &self,
        program: &Path,
        db_path: &Path,
        fdb_path: &Path,
        overwrite_fdb: bool,
        rebase: Option<u64>,
        rebase_relative: i32,
    ) -> Result<(), Self::Error>;
}

#[repr(transparent)]
struct Importer<B, E>(B)
where
    B: Backend<Error = E>;

impl<B, E> Backend for Importer<B, E>
where
    B: Backend<Error = E>,
    E: Into<crate::Error>,
{
    type Error = crate::Error;

    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn is_available(&self) -> bool {
        self.0.is_available()
    }

    fn is_preferred_for(&self, path: &Path) -> bool {
        self.0.is_preferred_for(path)
    }

    fn import_full(
        &self,
        program: &Path,
        db_path: &Path,
        fdb_path: &Path,
        overwrite_fdb: bool,
        rebase: Option<u64>,
        rebase_relative: i32,
    ) -> Result<(), Self::Error> {
        self.0
            .import_full(
                program,
                db_path,
                fdb_path,
                overwrite_fdb,
                rebase,
                rebase_relative,
            )
            .map_err(|e| e.into())
    }
}

#[repr(transparent)]
pub struct DatabaseImporterBackend(Box<dyn Backend<Error = crate::Error>>);

impl DatabaseImporterBackend {
    pub fn new<B, E>(backend: B) -> Self
    where
        B: Backend<Error = E> + 'static,
        E: Into<crate::Error> + 'static,
    {
        Self(Box::new(Importer(backend)))
    }
}

impl Deref for DatabaseImporterBackend {
    type Target = dyn Backend<Error=crate::Error>;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

inventory::collect!(DatabaseImporterBackend);
