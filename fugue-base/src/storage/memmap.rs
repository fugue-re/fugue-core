use std::borrow::Cow;
use std::fs::{self, File, OpenOptions};
use std::ops::Range;
use std::path::{Path, PathBuf};

use fallible_iterator::FallibleIterator;
use memmap2::MmapMut;
use thiserror::Error;

use crate::loader::{Loadable, LoadableSegment};
use crate::memory::SegmentProperties;
use crate::storage::{StorageError, StorageProvider, StorageProviderFromLoadable};
use crate::types::attributes::ATTRIBUTE_PROJECT_PATH;
use crate::types::Address;

const PROJECT_MEMORY_MAPPING: &str = "segments.mmap";

pub struct MemoryMappedStorage {
    backing: MmapMut,
    segments: Vec<LoadableSegmentMetadata>,
    project: PathBuf,
}

#[derive(Debug, Error)]
pub enum MemoryMappedStorageError {
    #[error("failed to create project: {0}")]
    CreateProject(std::io::Error),
    #[error("failed to create project memory mapping: {0}")]
    CreateProjectMapping(std::io::Error),
    #[error("invalid address")]
    InvalidAddress,
    #[error("invalid size")]
    InvalidSize,
    #[error("no project path specified")]
    NoProjectPath,
}

impl From<MemoryMappedStorageError> for StorageError {
    fn from(e: MemoryMappedStorageError) -> Self {
        match e {
            MemoryMappedStorageError::CreateProject(_)
            | MemoryMappedStorageError::CreateProjectMapping(_) => StorageError::backing(e),
            MemoryMappedStorageError::InvalidAddress => StorageError::InvalidAddress,
            MemoryMappedStorageError::InvalidSize => StorageError::InvalidSize,
            MemoryMappedStorageError::NoProjectPath => StorageError::InvalidAddress,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoadableSegmentMetadata {
    name: String,
    address: Address,
    physical_offset: usize,
    properties: SegmentProperties,
    size: usize,
}

impl LoadableSegmentMetadata {
    pub fn new(segm: &LoadableSegment, physical_offset: usize) -> Self {
        Self {
            address: segm.address(),
            name: segm.name().to_owned(),
            physical_offset,
            properties: segm.properties(),
            size: segm.len(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn address(&self) -> Address {
        self.address
    }

    pub fn last_address(&self) -> Address {
        self.address + self.size as u64 - 1usize
    }

    pub fn next_address(&self) -> Address {
        self.address + self.size
    }

    pub fn physical_offset(&self) -> usize {
        self.physical_offset
    }

    pub fn physical_range(&self) -> Range<usize> {
        self.physical_offset..self.physical_offset + self.size
    }

    pub fn properties(&self) -> SegmentProperties {
        self.properties
    }

    pub fn len(&self) -> usize {
        self.size
    }
}

impl MemoryMappedStorage {
    fn from_loadable_aux(
        project: impl AsRef<Path>,
        loader: &impl Loadable,
    ) -> Result<MemoryMappedStorage, StorageError> {
        let project = project.as_ref();
        fs::create_dir_all(&project).map_err(MemoryMappedStorageError::CreateProject)?;

        let path = project.join(PROJECT_MEMORY_MAPPING);

        tracing::trace!(
            "creating memory-mapped storage for project {} at {}",
            project.display(),
            path.display(),
        );

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .map_err(MemoryMappedStorageError::CreateProjectMapping)?;

        let (start, end) = loader.segment_range();
        let size = usize::from(end - start) + 1usize;

        tracing::trace!("memory-mapped storage size is {size} bytes");

        file.set_len(size as _)
            .map_err(MemoryMappedStorageError::CreateProjectMapping)?;

        let mut siter = loader.segments();

        let mut offset = 0;
        let mut backing = unsafe { MmapMut::map_mut(&file) }
            .map_err(MemoryMappedStorageError::CreateProjectMapping)?;

        let mut segments = Vec::new();

        while let Some(segm) = siter.next()? {
            tracing::trace!(
                "loading segment {} ({}-{}) into memory-mapped storage",
                segm.name(),
                segm.address(),
                segm.next_address()
            );
            segments.push(LoadableSegmentMetadata::new(&segm, offset));

            backing[offset..offset + segm.len()].copy_from_slice(segm.bytes());

            offset += segm.len();
        }

        segments.sort_by(|a, b| a.address().cmp(&b.address()));

        Ok(Self {
            backing,
            segments,
            project: project.to_owned(),
        })
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

    fn overlapping(
        &self,
        addr: Address,
        size: usize,
    ) -> Option<impl Iterator<Item = &LoadableSegmentMetadata>> {
        let last_addr = addr + size;

        if last_addr < addr {
            return None;
        }

        let first = self.position(addr)?;
        let last = self.position(last_addr)?;

        let view = &self.segments[first..last + 1];

        for i in 0..view.len() - 1usize {
            if view[i].next_address() != view[i + 1].address() {
                return None;
            }
        }

        Some(view.iter())
    }
}

impl Drop for MemoryMappedStorage {
    fn drop(&mut self) {
        if let Err(e) = fs::remove_dir_all(&self.project) {
            tracing::error!(
                "failed to clean-up memory-mapped storage at {}: {e}",
                self.project.display()
            );
        } else {
            tracing::trace!(
                "successfully cleaned-up memory-mapped storage at {}",
                self.project.display()
            );
        }
    }
}

impl StorageProviderFromLoadable for MemoryMappedStorage {
    fn from_loadable(loader: &impl Loadable) -> Result<Self, StorageError> {
        let project = loader
            .attributes()
            .get_attr::<PathBuf>(ATTRIBUTE_PROJECT_PATH)
            .ok_or_else(|| MemoryMappedStorageError::NoProjectPath)?;

        let res = Self::from_loadable_aux(&project, loader);

        if res.is_err() {
            tracing::error!("failed to create memory-mapped storage; cleaning-up");
            fs::remove_dir_all(&project).ok();
        }

        res
    }
}

impl StorageProvider for MemoryMappedStorage {
    fn read_bytes(&self, addr: Address, bytes: &mut [u8]) -> Result<usize, StorageError> {
        let mut size = bytes.len();
        let mut offset = 0;

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

            let read_offset = segm.physical_offset() + usize::from(read_addr - segm_addr);
            let read_size = size.min(usize::from(segm_last_addr - read_addr) + 1);

            let segm_bytes = self
                .backing
                .get(read_offset..read_offset + read_size)
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

    fn write_bytes(&mut self, addr: Address, bytes: &[u8]) -> Result<usize, StorageError> {
        let mut size = bytes.len();
        let mut offset = 0;

        tracing::trace!("writing {size} bytes to address {addr}");

        if bytes.is_empty() {
            return Ok(offset);
        }

        let last_addr = addr + size;

        if last_addr < addr {
            return Err(StorageError::InvalidAddress);
        }

        let first = self.position(addr).ok_or(StorageError::InvalidAddress)?;
        let last = self
            .position(last_addr)
            .ok_or(StorageError::InvalidAddress)?;

        let segms = &self.segments[first..last + 1];

        for i in 0..segms.len() - 1usize {
            if segms[i].next_address() != segms[i + 1].address() {
                return Err(StorageError::InvalidAddress);
            }
        }

        for segm in segms.iter() {
            let write_addr = addr + offset;

            let segm_addr = segm.address();
            let segm_last_addr = segm.last_address();

            let write_offset = segm.physical_offset() + usize::from(write_addr - segm_addr);
            let write_size = size.min(usize::from(segm_last_addr - write_addr) + 1);

            let segm_bytes = self
                .backing
                .get_mut(write_offset..write_offset + write_size)
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

    fn contains_segment(&self, addr: Address) -> bool {
        self.position(addr).is_some()
    }

    fn find_segment_containing(
        &self,
        addr: Address,
    ) -> Result<Cow<LoadableSegment<'_>>, StorageError> {
        self.position(addr)
            .map(|pos| {
                let segm = &self.segments[pos];
                let bytes = Cow::Borrowed(&self.backing[segm.physical_range()]);
                Cow::Owned(LoadableSegment::from_parts(
                    segm.name(),
                    segm.address(),
                    segm.properties(),
                    bytes,
                ))
            })
            .ok_or(StorageError::InvalidAddress)
    }
}
