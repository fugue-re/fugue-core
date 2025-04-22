use std::fmt;
use std::mem::size_of;
use std::sync::Arc;

use fugue_base::types::Address;
use thiserror::Error;

use crate::traits::{State, StateOps, StateValue};

#[derive(Debug, Error)]
pub enum Error {
    #[error("out-of-bounds read of `{size}` bytes at {address}")]
    OOBRead { address: Address, size: usize },
    #[error("out-of-bounds write of `{size}` bytes at {address}")]
    OOBWrite { address: Address, size: usize },
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct FlatState<T: StateValue> {
    backing: Vec<T>,
    dirty: DirtyBacking,
}

impl<T: StateValue> AsRef<Self> for FlatState<T> {
    #[inline(always)]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T: StateValue> AsMut<Self> for FlatState<T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T: StateValue> FlatState<T> {
    pub fn new(size: usize) -> Self {
        Self {
            backing: vec![T::default(); size],
            dirty: DirtyBacking::new(size),
        }
    }

    pub fn read_only(space: Arc<AddressSpace>, size: usize) -> Self {
        Self {
            backing: vec![T::default(); size],
            dirty: DirtyBacking::new(size),
        }
    }

    pub fn from_vec(values: Vec<T>) -> Self {
        let size = values.len();
        Self {
            backing: values,
            dirty: DirtyBacking::new(size),
        }
    }
}

impl<V: StateValue> State for FlatState<V> {
    type Error = Error;

    fn fork(&self) -> Self {
        Self {
            backing: self.backing.clone(),
            dirty: self.dirty.fork(),
        }
    }

    fn restore(&mut self, other: &Self) {
        for block in self.dirty.indices.drain(..) {
            let start = usize::from(block.start_address());
            let end = usize::from(block.end_address());

            let real_end = self.backing.len().min(end);

            self.dirty.bitsmap[block.index()] = 0;
            self.backing[start..real_end].clone_from_slice(&other.backing[start..real_end]);
        }
        self.dirty.clone_from(&other.dirty);
    }
}

impl<V: StateValue> StateOps for FlatState<V> {
    type Value = V;

    fn len(&self) -> usize {
        self.backing.len()
    }

    fn copy_values<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Error>
    where
        F: Into<Address>,
        T: Into<Address>,
    {
        let from = from.into();
        let to = to.into();

        let soff = usize::from(from);
        let doff = usize::from(to);

        if soff > self.len() || soff.checked_add(size).is_none() || soff + size > self.len() {
            return Err(Error::OOBRead {
                address: from,
                size,
            });
        }

        if doff > self.len() || doff.checked_add(size).is_none() || doff + size > self.len() {
            return Err(Error::OOBWrite { address: to, size });
        }

        if doff == soff {
            return Ok(());
        }

        if doff >= soff + size {
            let (shalf, dhalf) = self.backing.split_at_mut(doff);
            dhalf.clone_from_slice(&shalf[soff..(soff + size)]);
        } else if doff + size <= soff {
            let (dhalf, shalf) = self.backing.split_at_mut(soff);
            dhalf[doff..(doff + size)].clone_from_slice(&shalf);
        } else {
            // overlap; TODO: see if we can avoid superfluous clones
            if doff < soff {
                for i in 0..size {
                    unsafe {
                        let dptr = self.backing.as_mut_ptr().add(doff + i);
                        let sptr = self.backing.as_ptr().add(soff + i);
                        (&mut *dptr).clone_from(&*sptr);
                    }
                }
            } else {
                for i in (0..size).rev() {
                    unsafe {
                        let dptr = self.backing.as_mut_ptr().add(doff + i);
                        let sptr = self.backing.as_ptr().add(soff + i);
                        (&mut *dptr).clone_from(&*sptr);
                    }
                }
            }
        }

        self.dirty.dirty_region(&to, size);

        Ok(())
    }

    fn get_values<A>(&self, address: A, values: &mut [Self::Value]) -> Result<(), Error>
    where
        A: Into<Address>,
    {
        let address = address.into();
        let size = values.len();
        let start = usize::from(address);
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBRead {
                address,
                size: values.len(),
            });
        }

        let end = end.unwrap();

        values[..].clone_from_slice(&self.backing[start..end]);

        Ok(())
    }

    fn view_values<A>(&self, address: A, size: usize) -> Result<&[Self::Value], Error>
    where
        A: Into<Address>,
    {
        let address = address.into();
        let start = usize::from(address);
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBRead {
                address,
                size,
            });
        }

        let end = end.unwrap();

        Ok(&self.backing[start..end])
    }

    fn view_values_mut<A>(&mut self, address: A, size: usize) -> Result<&mut [Self::Value], Error>
    where
        A: Into<Address>,
    {
        let address = address.into();
        let start = usize::from(address);
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBRead {
                address,
                size,
            });
        }

        let end = end.unwrap();

        self.dirty.dirty_region(&address, size);

        Ok(&mut self.backing[start..end])
    }

    fn set_values<A>(&mut self, address: A, values: &[Self::Value]) -> Result<(), Error>
    where
        A: Into<Address>,
    {
        let address = address.into();
        let size = values.len();
        let start = usize::from(address);
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBWrite {
                address,
                size,
            });
        }

        let end = end.unwrap();

        self.backing[start..end].clone_from_slice(values);
        self.dirty.dirty_region(&address, size);

        Ok(())
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct Block(u64);

pub const BLOCK_SIZE: u64 = 64;

impl From<&'_ Address> for Block {
    fn from(t: &Address) -> Block {
        Self(u64::from(*t) / BLOCK_SIZE)
    }
}

impl From<Address> for Block {
    fn from(t: Address) -> Block {
        Self(u64::from(t) / BLOCK_SIZE)
    }
}

impl From<u64> for Block {
    fn from(t: u64) -> Block {
        Self(t)
    }
}

impl Block {
    #[inline]
    fn bit(&self) -> usize {
        self.0 as usize % size_of::<Self>()
    }

    #[inline]
    fn index(&self) -> usize {
        self.0 as usize / size_of::<Self>()
    }

    #[inline]
    fn start_address(&self) -> Address {
        (self.0 * BLOCK_SIZE).into()
    }

    #[inline]
    fn end_address(&self) -> Address {
        ((self.0 + 1) * BLOCK_SIZE).into()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct DirtyBacking {
    indices: Vec<Block>,
    bitsmap: Vec<u64>,
}

impl DirtyBacking {
    pub fn new(size: usize) -> Self {
        let backing_size = 1 + (size as u64 / BLOCK_SIZE) as usize;
        Self {
            indices: Vec::with_capacity(backing_size),
            bitsmap: vec![0 as u64; 1 + backing_size / size_of::<u64>()],
        }
    }

    #[inline]
    pub fn fork(&self) -> Self {
        self.clone()
    }

    #[inline]
    pub fn dirty<B: Into<Block>>(&mut self, block: B) {
        let block = block.into();
        let index = block.index();
        let check = 1 << block.bit();

        if self.bitsmap[index] & check == 0 {
            self.bitsmap[index] |= check;
            self.indices.push(block);
        }
    }

    #[inline]
    pub fn dirty_region(&mut self, start: &Address, size: usize) {
        let sblock = Block::from(start).0;
        let eblock = Block::from(*start + size as u64).0;

        for block in sblock..=eblock {
            self.dirty(block);
        }
    }
}
