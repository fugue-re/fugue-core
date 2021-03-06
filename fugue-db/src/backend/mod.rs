use std::ops::Deref;
use std::path::PathBuf;
use url::Url;

#[derive(Debug)]
pub enum Imported {
    Bytes(Vec<u8>),
    File(PathBuf),
}

/// The Backend trait for adding support for new backend
/// # Example
/// ```
/// use fugue_db::backend::{Backend, DatabaseImporterBackend, Imported};
/// use fugue_db::Error;
/// use url::Url;
/// pub struct MyNewTool{}
/// impl Backend for MyNewTool {
///     type Error = Error;
///     fn name(&self) -> &'static str {
///         "fugue-my-new-tool"
///     }
///     fn is_available(&self) -> bool {
///        todo!()
///     }
///     // ...
///     fn import(&self, program: &Url) -> Result<Imported, Self::Error> {
///         todo!()
///     }
///     fn is_preferred_for(&self, _: &Url) -> Option<bool> { todo!() }
/// }
/// ```
pub trait Backend {
    type Error: Into<crate::Error>;

    fn name(&self) -> &'static str;

    fn is_available(&self) -> bool;
    fn is_preferred_for(&self, path: &Url) -> Option<bool>;

    fn import(&self, program: &Url) -> Result<Imported, Self::Error>;
}

/// Wrapper for Backend 
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

    fn is_preferred_for(&self, path: &Url) -> Option<bool> {
        self.0.is_preferred_for(path)
    }

    fn import(&self, program: &Url) -> Result<Imported, Self::Error> {
        self.0.import(program).map_err(|e| e.into())
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
