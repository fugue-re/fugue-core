use std::marker::PhantomData;
use std::path::Path;

use heed::types::Bytes;
use heed::{BytesEncode, Database, Env, EnvOpenOptions, RoTxn, RwTxn};
use rkyv::ser::serializers::AllocSerializer;
use rkyv::{Archive, Serialize};
use tempfile::TempDir;
use thiserror::Error;

pub struct MmapTable<K> {
    environment: Env,
    database: Database<K, Bytes>,
    temporary: Option<TempDir>,
}

pub struct MmapTableReader<'a, K, T>
where
    T: Archive,
{
    table: &'a MmapTable<K>,
    txn: RoTxn<'a>,
    _marker: PhantomData<T>,
}

pub struct MmapTableWriter<'a, K, T>
where
    T: Archive,
{
    table: &'a MmapTable<K>,
    txn: RwTxn<'a>,
    _marker: PhantomData<T>,
}

#[derive(Debug, Error)]
pub enum MmapTableError {
    #[error(transparent)]
    Database(anyhow::Error),
    #[error(transparent)]
    IO(anyhow::Error),
}

impl MmapTableError {
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

impl<K> MmapTable<K>
where
    K: 'static,
{
    pub fn new(name: impl AsRef<str>, backing: impl AsRef<Path>) -> Result<Self, MmapTableError> {
        let environment = unsafe {
            EnvOpenOptions::new()
                .max_dbs(16)
                .map_size(4 * 1024 * 1024 * 1024)
                .open(backing.as_ref())
                .map_err(MmapTableError::database)?
        };

        let database = {
            let mut txn = environment.write_txn().map_err(MmapTableError::database)?;
            let database = environment
                .create_database(&mut txn, Some(name.as_ref()))
                .map_err(MmapTableError::database)?;
            txn.commit().map_err(MmapTableError::database)?;
            database
        };

        Ok(Self {
            environment,
            database,
            temporary: None,
        })
    }

    pub fn temporary(name: impl AsRef<str>) -> Result<Self, MmapTableError> {
        let backing = tempfile::tempdir().map_err(MmapTableError::io)?;

        let mut slf = Self::new(name.as_ref(), backing.as_ref())?;

        slf.temporary = Some(backing);

        Ok(slf)
    }

    pub fn reader<'a, T>(&'a self) -> Result<MmapTableReader<'a, K, T>, MmapTableError>
    where
        T: Archive,
    {
        let txn = self
            .environment
            .read_txn()
            .map_err(MmapTableError::database)?;
        Ok(MmapTableReader {
            table: self,
            txn,
            _marker: PhantomData,
        })
    }

    pub fn writer<'a, T>(&'a mut self) -> Result<MmapTableWriter<'a, K, T>, MmapTableError>
    where
        T: Archive,
    {
        let txn = self
            .environment
            .write_txn()
            .map_err(MmapTableError::database)?;
        Ok(MmapTableWriter {
            table: self,
            txn,
            _marker: PhantomData,
        })
    }
}

impl<'a, K, T> MmapTableReader<'a, K, T>
where
    T: Archive,
{
    pub fn get<KE>(&self, key: impl AsRef<KE>) -> Result<Option<&T::Archived>, MmapTableError>
    where
        K: for<'b> BytesEncode<'b, EItem = KE>,
        KE: ?Sized + 'static,
    {
        let val = self
            .table
            .database
            .get(&self.txn, key.as_ref())
            .map_err(MmapTableError::database)?;

        Ok(val.map(|val| unsafe { rkyv::archived_root::<T>(val) }))
    }
}

impl<'a, K, T> MmapTableWriter<'a, K, T>
where
    T: Archive + Serialize<AllocSerializer<1024>>,
{
    pub fn get<KE>(&self, key: impl AsRef<KE>) -> Result<Option<&T::Archived>, MmapTableError>
    where
        K: for<'b> BytesEncode<'b, EItem = KE>,
        KE: ?Sized + 'static,
    {
        let val = self
            .table
            .database
            .get(&self.txn, key.as_ref())
            .map_err(MmapTableError::database)?;

        Ok(val.map(|val| unsafe { rkyv::archived_root::<T>(val) }))
    }

    pub fn set<KE>(&mut self, key: impl AsRef<KE>, val: impl AsRef<T>) -> Result<(), MmapTableError>
    where
        K: for<'b> BytesEncode<'b, EItem = KE>,
        KE: ?Sized + 'static,
    {
        let val = rkyv::to_bytes::<_, 1024>(val.as_ref()).map_err(MmapTableError::database)?;

        self.table
            .database
            .put(&mut self.txn, key.as_ref(), val.as_ref())
            .map_err(MmapTableError::database)?;

        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), MmapTableError> {
        self.table
            .database
            .clear(&mut self.txn)
            .map_err(MmapTableError::database)?;

        Ok(())
    }

    pub fn remove<KE>(&mut self, key: impl AsRef<KE>) -> Result<(), MmapTableError>
    where
        K: for<'b> BytesEncode<'b, EItem = KE>,
        KE: ?Sized + 'static,
    {
        self.table
            .database
            .delete(&mut self.txn, key.as_ref())
            .map_err(MmapTableError::database)?;

        Ok(())
    }

    pub fn abort(self) {
        self.txn.abort()
    }

    pub fn commit(self) -> Result<(), MmapTableError> {
        self.txn.commit().map_err(MmapTableError::database)
    }
}

#[cfg(test)]
mod test {
    use heed::types::Str;

    use super::*;

    #[test]
    fn test_project() -> Result<(), Box<dyn std::error::Error>> {
        let mut pt = MmapTable::<Str>::temporary("project")?;

        {
            let mut writer = pt.writer::<Vec<u8>>()?;

            writer.set("mapping1", vec![0u8; 10])?;
            writer.set("mapping2", vec![0u8; 100 * 1024 * 1024])?;
            writer.set("mapping3", vec![0u8; 256 * 1024 * 1024])?;

            writer.commit()?;
        }

        {
            let reader = pt.reader::<Vec<u8>>()?;

            let bytes = reader.get("mapping2")?.unwrap();

            assert_eq!(bytes.len(), 100 * 1024 * 1024);
        }

        Ok(())
    }
}
