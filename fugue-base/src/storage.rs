use std::borrow::Cow;

use fallible_iterator::FallibleIterator;
use thiserror::Error;

use crate::loader::{Loadable, LoadableSegment, LoaderError};
use crate::types::Address;

pub mod mdbx;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage error: {0}")]
    Backing(anyhow::Error),
    #[error("invalid address range")]
    InvalidAddressRange,
    #[error("invalid address")]
    InvalidAddress,
    #[error("invalid size")]
    InvalidSize,
    #[error(transparent)]
    Loader(#[from] LoaderError),
}

impl StorageError {
    pub fn backing<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Backing(anyhow::Error::from(e))
    }

    pub fn backing_with<M>(msg: M) -> Self
    where
        M: std::fmt::Debug + std::fmt::Display + Send + Sync + 'static,
    {
        Self::Backing(anyhow::Error::msg(msg))
    }
}

pub trait StorageProvider {
    // Creates a new storage provider from the given loadable object.
    fn from_loadable(loader: &impl Loadable) -> Result<Self, StorageError>
    where
        Self: Sized;

    // Reads the given bytes from the storage at the specified address; returns the number of bytes
    // read.
    fn read_bytes(&self, addr: impl Into<Address>, bytes: &mut [u8])
        -> Result<usize, StorageError>;

    // Reads the given bytes from the storage at the specified address; fails if not all bytes can
    // be read, e.g., due to gaps or lack of segment coverage.
    fn read_bytes_exact(
        &self,
        addr: impl Into<Address>,
        bytes: &mut [u8],
    ) -> Result<(), StorageError> {
        if self.read_bytes(addr, bytes)? != bytes.len() {
            return Err(StorageError::InvalidAddressRange);
        }
        Ok(())
    }

    // Writes the given bytes to the storage at the specified address; returns the number of bytes
    // written.
    fn write_bytes(
        &mut self,
        addr: impl Into<Address>,
        bytes: &[u8],
    ) -> Result<usize, StorageError>;

    // Writes the given bytes to the storage at the specified address; fails if not all bytes can
    // be written, e.g., due to gaps or lack of segment coverage.
    fn write_bytes_exact(
        &mut self,
        addr: impl Into<Address>,
        bytes: &[u8],
    ) -> Result<(), StorageError> {
        if self.write_bytes(addr, bytes)? != bytes.len() {
            return Err(StorageError::InvalidAddressRange);
        }
        Ok(())
    }

    // Returns true if the storage has a segment that contaings the given address.
    fn contains_segment(&self, at: impl Into<Address>) -> bool;

    // Returns the segment that contains the given address, if any.
    fn find_segment_containing(
        &self,
        addr: impl Into<Address>,
    ) -> Result<Cow<LoadableSegment<'_>>, StorageError>;

    // Returns a view of length `size` over the bytes of the segment containing the given address.
    fn view_segment_bytes(
        &self,
        addr: impl Into<Address>,
        size: usize,
    ) -> Result<Cow<[u8]>, StorageError> {
        let addr = addr.into();
        let segm = self.find_segment_containing(addr)?;

        let offset = usize::from(addr - segm.address());
        let bytes = match segm {
            Cow::Borrowed(segm) => Cow::Borrowed(
                segm.view_bytes(offset, size)
                    .ok_or(StorageError::InvalidSize)?,
            ),
            Cow::Owned(segm) => Cow::Owned(
                segm.view_bytes(offset, size)
                    .ok_or(StorageError::InvalidSize)?
                    .to_owned(),
            ),
        };

        Ok(bytes)
    }

    // Returns a view over the bytes of the segment containing the given address, starting from the
    // given address.
    fn view_segment_bytes_from(&self, addr: impl Into<Address>) -> Result<Cow<[u8]>, StorageError> {
        let addr = addr.into();
        let segm = self.find_segment_containing(addr)?;

        let offset = usize::from(addr - segm.address());
        let bytes = match segm {
            Cow::Borrowed(segm) => Cow::Borrowed(
                segm.view_bytes_from(offset)
                    .ok_or(StorageError::InvalidSize)?,
            ),
            Cow::Owned(segm) => Cow::Owned(
                segm.view_bytes_from(offset)
                    .ok_or(StorageError::InvalidSize)?
                    .to_owned(),
            ),
        };

        Ok(bytes)
    }
}

pub struct InMemoryStorage {
    segments: Vec<LoadableSegment<'static>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    fn position(&self, addr: Address) -> Option<usize> {
        self.segments
            .binary_search_by(|segm| {
                if addr < segm.address() {
                    std::cmp::Ordering::Greater
                } else if addr > segm.last_address() {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .ok()
    }

    pub fn overlapping(
        &self,
        addr: Address,
        size: usize,
    ) -> Option<impl Iterator<Item = &LoadableSegment<'static>>> {
        let last_addr = addr + size as u64;

        if last_addr < addr {
            return None;
        }

        let first = self.position(addr)?;

        Some(
            self.segments[first..]
                .iter()
                .take_while(move |segm| last_addr <= segm.last_address()),
        )
    }

    pub fn overlapping_mut<'a>(
        &'a mut self,
        addr: Address,
        size: usize,
    ) -> Option<impl Iterator<Item = &'a mut LoadableSegment<'static>> + 'a> {
        let last_addr = addr + size as u64;

        if last_addr < addr {
            return None;
        }

        let first = self.position(addr)?;

        Some(
            self.segments[first..]
                .iter_mut()
                .take_while(move |segm| last_addr <= segm.last_address()),
        )
    }
}

pub type BoxedStorage = Box<dyn StorageProvider>;

impl StorageProvider for InMemoryStorage {
    fn from_loadable(loader: &impl Loadable) -> Result<Self, StorageError> {
        let mut segments = Vec::new();
        let mut siter = loader.segments();

        while let Some(segm) = siter.next()? {
            tracing::trace!(
                "loading segment {} ({}-{}) into in-memory storage",
                segm.name(),
                segm.address(),
                segm.next_address()
            );
            segments.push(segm.into_owned());
        }

        segments.sort_by(|a, b| a.address().cmp(&b.address()));

        Ok(Self { segments })
    }

    fn read_bytes(
        &self,
        addr: impl Into<Address>,
        bytes: &mut [u8],
    ) -> Result<usize, StorageError> {
        let mut size = bytes.len();
        let mut offset = 0;

        let addr = addr.into();

        tracing::trace!("reading {size} bytes from address {addr}");

        if bytes.is_empty() {
            return Ok(offset);
        }

        let segms = self
            .overlapping(addr, size)
            .ok_or(StorageError::InvalidAddress)?;

        for segm in segms {
            let read_addr = addr + offset;

            let segm_addr = segm.address();
            let segm_last_addr = segm.last_address();

            let read_offset = usize::from(read_addr - segm_addr);
            let read_size = size.min(usize::from(segm_last_addr - read_addr) + 1);

            let segm_bytes = segm
                .view_bytes(read_offset, read_size)
                .ok_or(StorageError::InvalidAddress)?;

            bytes[offset..offset + read_size].copy_from_slice(segm_bytes);

            size -= read_size;
            offset += read_size;

            if size == 0 {
                break;
            }
        }

        Ok(offset)
    }

    fn write_bytes(
        &mut self,
        addr: impl Into<Address>,
        bytes: &[u8],
    ) -> Result<usize, StorageError> {
        let mut size = bytes.len();
        let mut offset = 0;

        if bytes.is_empty() {
            return Ok(offset);
        }

        let addr = addr.into();

        let segms = self
            .overlapping_mut(addr, size)
            .ok_or(StorageError::InvalidAddress)?;

        for segm in segms {
            let write_addr = addr + offset;

            let segm_addr = segm.address();
            let segm_last_addr = segm.last_address();

            let write_offset = usize::from(write_addr - segm_addr);
            let write_size = size.min(usize::from(segm_last_addr - write_addr) + 1);

            let segm_bytes = segm
                .view_bytes_mut(write_offset, write_size)
                .ok_or(StorageError::InvalidAddress)?;

            segm_bytes.copy_from_slice(&bytes[offset..offset + write_size]);

            size -= write_size;
            offset += write_size;

            if size == 0 {
                break;
            }
        }

        Ok(offset)
    }

    fn contains_segment(&self, at: impl Into<Address>) -> bool {
        self.position(at.into()).is_some()
    }

    fn find_segment_containing(
        &self,
        addr: impl Into<Address>,
    ) -> Result<Cow<LoadableSegment<'_>>, StorageError> {
        let addr = addr.into();
        self.position(addr)
            .map(|pos| Cow::Borrowed(&self.segments[pos]))
            .ok_or(StorageError::InvalidAddress)
    }
}
