use std::path::Path;
use std::path::PathBuf;

use heed::types::Bytes;
use heed::{Database, Env, EnvOpenOptions, RoTxn, RwTxn};
use rkyv::ser::serializers::AllocSerializer;
use rkyv::{Archive, Archived, Serialize};
use tempfile::TempDir;
use thiserror::Error;

use crate::types::key::Key;

pub struct ProjectDB {
    environment: Env,
    database: Database<Bytes, Bytes>,
    mappings: Database<Bytes, Bytes>,
    location: ProjectDBLocation,
}

#[derive(Debug, Error)]
pub enum ProjectDBError {
    #[error(transparent)]
    Database(anyhow::Error),
    #[error(transparent)]
    IO(anyhow::Error),
}

impl ProjectDBError {
    pub fn database<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Database(e.into())
    }

    pub fn database_with<M>(m: M) -> Self
    where
        M: std::fmt::Debug + std::fmt::Display + Send + Sync + 'static,
    {
        Self::Database(anyhow::Error::msg(m))
    }

    pub fn io<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::IO(e.into())
    }
}

pub enum ProjectDBLocation {
    Temporary(TempDir),
    Path(PathBuf),
}

impl ProjectDBLocation {
    pub fn temporary() -> Result<Self, ProjectDBError> {
        Ok(Self::Temporary(TempDir::new().map_err(ProjectDBError::io)?))
    }

    pub fn fixed(path: impl AsRef<Path>) -> Self {
        Self::Path(path.as_ref().to_path_buf())
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::Temporary(d) => d.path(),
            Self::Path(d) => d,
        }
    }
}

impl<T> From<T> for ProjectDBLocation
where
    T: AsRef<Path>,
{
    fn from(value: T) -> Self {
        Self::fixed(value)
    }
}

pub struct ProjectDBReader<'a> {
    txn: RoTxn<'a>,
    database: &'a Database<Bytes, Bytes>,
    mappings: &'a Database<Bytes, Bytes>,
}

pub struct ProjectDBWriter<'a> {
    txn: RwTxn<'a>,
    database: &'a Database<Bytes, Bytes>,
    mappings: &'a Database<Bytes, Bytes>,
}

impl<'a> ProjectDBWriter<'a> {
    pub fn write<T, const N: usize>(&mut self, key: &Key, val: &T) -> Result<(), ProjectDBError>
    where
        T: Serialize<AllocSerializer<N>>,
    {
        self.database
            .put(
                &mut self.txn,
                key.as_ref(),
                rkyv::to_bytes::<_, N>(val)
                    .map_err(ProjectDBError::database)?
                    .as_ref(),
            )
            .map_err(ProjectDBError::database)?;

        Ok(())
    }
}

pub trait ProjectDBStorable: Archive {
    fn key(&self) -> Key;

    fn fetch<'a>(db: &ProjectDBReader<'a>) -> Result<&'a Archived<Self>, ProjectDBError>;
    fn store<'a>(&self, db: &mut ProjectDBWriter<'a>) -> Result<(), ProjectDBError>;
}

impl ProjectDB {
    pub fn new() -> Result<Self, ProjectDBError> {
        Self::new_with(ProjectDBLocation::temporary()?)
    }

    pub fn new_with(location: impl Into<ProjectDBLocation>) -> Result<Self, ProjectDBError> {
        let location = location.into();
        let environment = unsafe {
            EnvOpenOptions::new()
                .max_dbs(2)
                .map_size(4 * 4 * 1024 * 1024 * 1024)
                .open(location.path())
                .map_err(ProjectDBError::database)?
        };

        let (database, mappings) = {
            let mut txn = environment.write_txn().map_err(ProjectDBError::database)?;

            let database = environment
                .create_database(&mut txn, Some("root"))
                .map_err(ProjectDBError::database)?;

            let mappings = environment
                .create_database(&mut txn, Some("maps"))
                .map_err(ProjectDBError::database)?;

            txn.commit().map_err(ProjectDBError::database)?;

            (database, mappings)
        };

        Ok(Self {
            environment,
            database,
            mappings,
            location,
        })
    }

    pub fn reader(&self) -> Result<ProjectDBReader, ProjectDBError> {
        Ok(ProjectDBReader {
            txn: self
                .environment
                .read_txn()
                .map_err(ProjectDBError::database)?,
            database: &self.database,
            mappings: &self.mappings,
        })
    }

    pub fn writer(&mut self) -> Result<ProjectDBWriter, ProjectDBError> {
        Ok(ProjectDBWriter {
            txn: self
                .environment
                .write_txn()
                .map_err(ProjectDBError::database)?,
            database: &self.database,
            mappings: &self.mappings,
        })
    }
}
