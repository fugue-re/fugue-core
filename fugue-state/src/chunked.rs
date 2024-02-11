use std::sync::Arc;

use fugue_ir::{Address, AddressSpace};
use iset::IntervalSet;
use thiserror::Error;

use crate::flat::{self, FlatState};
use crate::traits::{State, StateOps, StateValue};

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Backing(flat::Error),
    #[error("not enough free space to allocate {0} bytes")]
    NotEnoughFreeSpace(usize),
    #[error("attempt to access unmanaged region of `{size}` bytes at {address}")]
    AccessUnmanaged { address: Address, size: usize },
    #[error("attempt to free unmanaged region at {0}")]
    FreeUnmanaged(Address),
    #[error("attempt to reallocate unmanaged region at {0}")]
    ReallocateUnmanaged(Address),
    #[error("access at {address} of `{size}` bytes spans multiple allocations")]
    HeapOverflow { address: Address, size: usize },
}

impl Error {
    fn backing(base: Address, e: flat::Error) -> Self {
        Self::Backing(match e {
            flat::Error::OOBRead { address, size } => flat::Error::OOBRead {
                address: address + base,
                size,
            },
            flat::Error::OOBWrite { address, size } => flat::Error::OOBWrite {
                address: address + base,
                size,
            },
            flat::Error::AccessViolation {
                address,
                access,
                size,
            } => flat::Error::AccessViolation {
                address: address + base,
                access,
                size,
            },
        })
    }
}

#[derive(Debug, Clone)]
enum ChunkStatus {
    Taken { offset: usize, size: usize },
    Free { offset: usize, size: usize },
}

impl ChunkStatus {
    fn free(offset: usize, size: usize) -> Self {
        Self::Free { offset, size }
    }

    fn taken(offset: usize, size: usize) -> Self {
        Self::Taken { offset, size }
    }

    fn is_free(&self) -> bool {
        matches!(self, Self::Free { .. })
    }

    fn is_taken(&self) -> bool {
        matches!(self, Self::Taken { .. })
    }

    fn offset(&self) -> usize {
        match self {
            Self::Free { offset, .. } | Self::Taken { offset, .. } => *offset,
        }
    }

    fn size(&self) -> usize {
        match self {
            Self::Free { size, .. } | Self::Taken { size, .. } => *size,
        }
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
struct ChunkList(Vec<ChunkStatus>);

impl ChunkList {
    fn new(size: usize) -> Self {
        Self(vec![ChunkStatus::free(0, size)])
    }

    fn allocate(&mut self, size: usize) -> Option<usize> {
        for i in 0..self.0.len() {
            if self.0[i].is_free() {
                let free_size = self.0[i].size();
                if free_size == size {
                    let offset = self.0[i].offset();
                    // mut to taken
                    self.0[i] = ChunkStatus::taken(offset, size);
                    return Some(offset);
                } else if free_size > size {
                    // split to taken/free
                    let offset = self.0[i].offset();
                    self.0[i] = ChunkStatus::taken(offset, size);
                    self.0
                        .insert(i + 1, ChunkStatus::free(offset + size, free_size - size));
                    return Some(offset);
                }
            }
        }
        None
    }

    fn reallocate(&mut self, offset: usize, new_size: usize) -> Option<(usize, usize)> {
        for i in 0..self.0.len() {
            if self.0[i].is_taken() && self.0[i].offset() == offset {
                let mut size = self.0[i].size();
                let old_size = size;

                if new_size == old_size {
                    // do nothing
                    return Some((offset, old_size));
                } else if new_size < old_size {
                    self.0[i] = ChunkStatus::taken(offset, new_size);
                    let diff = old_size - new_size;

                    // maybe merge up
                    if i < self.0.len() - 1 && self.0[i + 1].is_free() {
                        let upd_offset = self.0[i + 1].offset() - diff;
                        let upd_size = self.0[i + 1].size() + diff;

                        self.0[i + 1] = ChunkStatus::free(upd_offset, upd_size);
                    } else {
                        self.0
                            .insert(i + 1, ChunkStatus::free(offset + new_size, diff));
                    }

                    return Some((offset, old_size));
                }

                // test if we can merge frees
                let mut spos = i;
                let mut epos = i;

                let mut offset = offset;

                if i < self.0.len() - 1 && self.0[i + 1].is_free() {
                    size += self.0[i + 1].size();
                    epos = i + 1;
                }

                if size > new_size {
                    self.0[spos] = ChunkStatus::taken(offset, new_size);
                    self.0[epos] = ChunkStatus::free(offset + new_size, size - new_size);
                    return Some((offset, old_size));
                } else if size == new_size {
                    self.0[spos] = ChunkStatus::taken(offset, new_size);
                    self.0.remove(epos);
                    return Some((offset, old_size));
                }

                if i > 0 && self.0[i - 1].is_free() {
                    offset = self.0[i - 1].offset();
                    size += self.0[i - 1].size();
                    spos = i - 1;
                }

                if size > new_size {
                    self.0[spos] = ChunkStatus::taken(offset, new_size);
                    self.0[spos + 1] = ChunkStatus::free(offset + new_size, size - new_size);
                    self.0.remove(i + 1);
                    return Some((offset, old_size));
                } else if size == new_size {
                    self.0[spos] = ChunkStatus::taken(offset, new_size);
                    self.0.remove(i);
                    self.0.remove(i + 1);
                    return Some((offset, old_size));
                }

                // all else fails.
                if let Some(new_offset) = self.allocate(new_size) {
                    self.deallocate(offset);
                    return Some((new_offset, old_size));
                } else {
                    return None;
                }
            }
        }
        None
    }

    fn deallocate(&mut self, offset: usize) -> Option<usize> {
        for i in 0..self.0.len() {
            if self.0[i].is_taken() && self.0[i].offset() == offset {
                // see if we should merge frees
                let mut spos = i;
                let mut offset = offset;
                let mut size = self.0[i].size();
                let old_size = size;

                if i < self.0.len() - 1 && self.0[i + 1].is_free() {
                    size += self.0[i + 1].size();
                    self.0.remove(i + 1);
                }

                if i > 0 && self.0[i - 1].is_free() {
                    offset = self.0[i - 1].offset();
                    size += self.0[i - 1].size();
                    self.0.remove(i);
                    spos = i - 1;
                }

                self.0[spos] = ChunkStatus::free(offset, size);
                return Some(old_size);
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct ChunkState<T: StateValue> {
    base_address: Address,
    chunks: ChunkList,
    regions: IntervalSet<Address>,
    backing: FlatState<T>,
    space: Arc<AddressSpace>,
}

impl<T: StateValue> AsRef<Self> for ChunkState<T> {
    #[inline(always)]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T: StateValue> AsMut<Self> for ChunkState<T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T: StateValue> AsRef<FlatState<T>> for ChunkState<T> {
    #[inline(always)]
    fn as_ref(&self) -> &FlatState<T> {
        &self.backing
    }
}

impl<T: StateValue> AsMut<FlatState<T>> for ChunkState<T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut FlatState<T> {
        &mut self.backing
    }
}

impl<T: StateValue> ChunkState<T> {
    pub fn new<A>(space: Arc<AddressSpace>, base_address: A, size: usize) -> Self
    where
        A: Into<Address>,
    {
        Self {
            base_address: base_address.into(),
            chunks: ChunkList::new(size),
            regions: IntervalSet::new(),
            backing: FlatState::read_only(space.clone(), size),
            space,
        }
    }

    pub fn base_address(&self) -> Address {
        self.base_address
    }

    pub fn inner(&self) -> &FlatState<T> {
        &self.backing
    }

    pub fn inner_mut(&mut self) -> &mut FlatState<T> {
        &mut self.backing
    }

    pub fn allocate(&mut self, size: usize) -> Result<Address, Error> {
        self.allocate_with(size, |_, _| ())
    }

    pub fn allocate_all(&mut self) -> Result<(), Error> {
        self.allocate_with(self.len() - 1, |_, _| ()).map(|_| ())
    }

    #[inline]
    pub fn allocate_with<F>(&mut self, size: usize, f: F) -> Result<Address, Error>
    where
        F: FnOnce(Address, &mut [T]),
    {
        // we allocate +1 on size to mark the last part as a red-zone
        let offset = self
            .chunks
            .allocate(size + 1)
            .ok_or_else(|| Error::NotEnoughFreeSpace(size))?;
        let address = self.base_address + offset;

        // set R/W permissions
        /*
        self.backing.permissions_mut().set_region(
            &Address::from(offset as u64),
            size,
            Access::ReadWrite,
        );
        self.backing
            .permissions_mut()
            .clear_byte(&(Address::from(offset as u64) + size), Access::Write);
        */

        // update region mappings
        self.regions.insert(address..address + size);

        // get a mutable view over the backing
        let view = self
            .backing
            .view_values_mut(Address::from(offset as u64), size)
            .map_err(Error::Backing)?;

        f(address, view);

        Ok(address)
    }

    pub fn reallocate<A>(&mut self, address: A, size: usize) -> Result<Address, Error>
    where
        A: Into<Address>,
    {
        let address = address.into();
        let interval = self
            .regions
            .overlap(address)
            .next()
            .ok_or_else(|| Error::ReallocateUnmanaged(address))?;

        if interval.start != address {
            return Err(Error::ReallocateUnmanaged(address));
        }

        // check permissions first
        let old_offset = address - self.base_address;
        let old_size = interval.end - interval.start;

        /*
        if !self
            .backing
            .permissions()
            .all_readable(&old_offset, old_size.into())
        {
            return Err(Error::Backing(flat::Error::AccessViolation {
                address: AddressValue::new(self.space.clone(), address.into()),
                access: Access::Read,
                size,
            }));
        }
        */

        // size + 1 to use the last byte as a red-zone
        let (offset, _old_size) = self
            .chunks
            .reallocate(old_offset.into(), size + 1)
            .ok_or_else(|| Error::NotEnoughFreeSpace(size))?;

        let new_address = self.base_address + offset;

        /*
        // set R/W permissions
        self.backing.permissions_mut().set_region(
            &Address::from(offset as u64),
            size,
            Access::ReadWrite,
        );
        self.backing
            .permissions_mut()
            .clear_byte(&(Address::from(offset as u64) + size), Access::Write);
        */

        // copy if moved
        let offset = Address::from(offset as u64);
        if old_offset != offset {
            self.backing
                .copy_values(old_offset, offset, old_size.into())
                .map_err(Error::Backing)?;

            /*
            self.backing.permissions_mut().clear_region(
                &old_offset,
                old_size.into(),
                Access::Write,
            );
            */
        }

        // update region mappings
        self.regions.remove(interval);
        self.regions
            .insert(new_address..new_address + size);

        Ok(new_address)
    }

    pub fn deallocate<A>(&mut self, address: A) -> Result<(), Error>
    where
        A: Into<Address>,
    {
        let address = address.into();
        let interval = self
            .regions
            .overlap(address)
            .next()
            .ok_or_else(|| Error::FreeUnmanaged(address))?;

        if interval.start != address {
            return Err(Error::FreeUnmanaged(address));
        }

        let offset = address - self.base_address;
        self.chunks
            .deallocate(offset.into())
            .ok_or_else(|| Error::FreeUnmanaged(address))?;

        /*
        let size = usize::from(interval.end - interval.start);

        self.backing
            .permissions_mut()
            .clear_region(&offset, size, flat::Access::Write);
        */

        self.regions.remove(interval);

        Ok(())
    }

    pub(crate) fn translate_checked<A>(&self, address: A, size: usize) -> Result<usize, Error>
    where
        A: Into<Address>,
    {
        let address = address.into();
        let mut regions = self.regions.iter(address..address + size);

        // we just need to know that it exists
        let _region = regions.next().ok_or_else(|| {
            Error::AccessUnmanaged { address, size }
        })?;

        // ..and that another does not exist
        if regions.next().is_some() {
            // violation
            return Err(Error::HeapOverflow { address, size });
        }

        Ok(usize::from(address - self.base_address))
    }
}

impl<V: StateValue> State for ChunkState<V> {
    type Error = Error;

    fn fork(&self) -> Self {
        Self {
            base_address: self.base_address.clone(),
            chunks: self.chunks.clone(),
            regions: self.regions.clone(),
            backing: self.backing.fork(),
            space: self.space.clone(),
        }
    }

    fn restore(&mut self, other: &Self) {
        self.base_address = other.base_address;
        self.chunks = other.chunks.clone();
        self.regions = other.regions.clone();
        self.backing.restore(&other.backing);
    }
}

impl<V: StateValue> StateOps for ChunkState<V> {
    type Value = V;

    fn len(&self) -> usize {
        self.backing.len()
    }

    fn copy_values<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Error>
    where
        F: Into<Address>,
        T: Into<Address>,
    {
        let from = self.translate_checked(from, size)?;
        let to = self.translate_checked(to, size)?;

        self.backing
            .copy_values(from as u64, to as u64, size)
            .map_err(|e| Error::backing(self.base_address, e))
    }

    fn get_values<A>(&self, address: A, values: &mut [Self::Value]) -> Result<(), Error>
    where
        A: Into<Address>,
    {
        let size = values.len();
        let address = self.translate_checked(address, size)?;

        self.backing
            .get_values(address as u64, values)
            .map_err(|e| Error::backing(self.base_address, e))
    }

    fn view_values<A>(&self, address: A, n: usize) -> Result<&[Self::Value], Error>
    where
        A: Into<Address>,
    {
        let address = self.translate_checked(address, n)?;

        self.backing
            .view_values(address as u64, n)
            .map_err(|e| Error::backing(self.base_address, e))
    }

    fn view_values_mut<A>(&mut self, address: A, n: usize) -> Result<&mut [Self::Value], Error>
    where
        A: Into<Address>,
    {
        let address = self.translate_checked(address, n)?;
        let base_address = self.base_address;

        self.backing
            .view_values_mut(address as u64, n)
            .map_err(|e| Error::backing(base_address, e))
    }

    fn set_values<A>(&mut self, address: A, values: &[Self::Value]) -> Result<(), Error>
    where
        A: Into<Address>,
    {
        let size = values.len();
        let address = self.translate_checked(address, size)?;

        self.backing
            .set_values(address as u64, values)
            .map_err(|e| Error::backing(self.base_address, e))
    }
}
