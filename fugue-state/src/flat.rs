use std::fmt;
use std::mem::size_of;
use std::sync::Arc;

use fugue_ir::{Address, AddressValue, AddressSpace};

use crate::traits::{State, StateOps, StateValue};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{access} access violation at {address} of {size} bytes in space `{}`", address.space().index())]
    AccessViolation { address: AddressValue, size: usize, access: Access },
    #[error("out-of-bounds read of `{size}` bytes at {address}")]
    OOBRead { address: Address, size: usize },
    #[error("out-of-bounds write of `{size}` bytes at {address}")]
    OOBWrite { address: Address, size: usize },
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct FlatState<T: StateValue> {
    backing: Vec<T>,
    dirty: DirtyBacking,
    //permissions: Permissions,
    space: Arc<AddressSpace>,
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
    pub fn new(space: Arc<AddressSpace>, size: usize) -> Self {
        Self {
            backing: vec![T::default(); size],
            dirty: DirtyBacking::new(size),
            //permissions: Permissions::new(space.clone(), size),
            space,
        }
    }

    pub fn read_only(space: Arc<AddressSpace>, size: usize) -> Self {
        Self {
            backing: vec![T::default(); size],
            dirty: DirtyBacking::new(size),
            //permissions: Permissions::new_with(space.clone(), size, PERM_READ_MASK),
            space,
        }
    }

    pub fn from_vec(space: Arc<AddressSpace>, values: Vec<T>) -> Self {
        let size = values.len();
        Self {
            backing: values,
            dirty: DirtyBacking::new(size),
            //permissions: Permissions::new(space.clone(), size),
            space,
        }
    }

    /*
    pub fn permissions(&self) -> &Permissions {
        &self.permissions
    }

    pub fn permissions_mut(&mut self) -> &mut Permissions {
        &mut self.permissions
    }
    */

    pub fn address_space(&self) -> Arc<AddressSpace> {
        self.space.clone()
    }

    pub fn address_space_ref(&self) -> &AddressSpace {
        self.space.as_ref()
    }
}

impl<V: StateValue> State for FlatState<V> {
    type Error = Error;

    fn fork(&self) -> Self {
        Self {
            backing: self.backing.clone(),
            dirty: self.dirty.fork(),
            //permissions: self.permissions.clone(),
            space: self.space.clone(),
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
        //self.permissions.restore(&other.permissions);
        self.dirty.clone_from(&other.dirty);
    }
}

impl<V: StateValue> StateOps for FlatState<V> {
    type Value = V;

    fn len(&self) -> usize {
        self.backing.len()
    }

    fn copy_values<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Error>
    where F: Into<Address>,
          T: Into<Address> {
        let from = from.into();
        let to = to.into();

        let soff = usize::from(from);
        let doff = usize::from(to);

        if soff > self.len() || soff.checked_add(size).is_none() || soff + size > self.len() {
            return Err(Error::OOBRead {
                address: from.clone(),
                size, //(soff + size) - self.len(),
            });
        }

        /*
        if !self.permissions.all_readable(&from, size) {
            return Err(Error::AccessViolation {
                address: AddressValue::new(self.space.clone(), from.into()),
                size,
                access: Access::Read,
            })
        }
        */

        if doff > self.len() || doff.checked_add(size).is_none() || doff + size > self.len() {
            return Err(Error::OOBWrite {
                address: to.clone(),
                size, // (doff + size) - self.len(),
            });
        }

        /*
        if !self.permissions.all_writable(&to, size) {
            return Err(Error::AccessViolation {
                address: AddressValue::new(self.space.clone(), to.into()),
                size,
                access: Access::Write,
            })
        }
        */

        if doff == soff {
            return Ok(())
        }

        if doff >= soff + size {
            let (shalf, dhalf) = self.backing.split_at_mut(doff);
            dhalf.clone_from_slice(&shalf[soff..(soff + size)]);
        } else if doff + size <= soff {
            let (dhalf, shalf) = self.backing.split_at_mut(soff);
            dhalf[doff..(doff+size)].clone_from_slice(&shalf);
        } else { // overlap; TODO: see if we can avoid superfluous clones
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
    where A: Into<Address> {
        let address = address.into();
        let size = values.len();
        let start = usize::from(address);
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBRead {
                address: address.clone(),
                size: values.len(),
            });
        }

        /*
        if !self.permissions.all_readable(&address, size) {
            return Err(Error::AccessViolation {
                address: AddressValue::new(self.space.clone(), address.into()),
                size,
                access: Access::Read,
            })
        }
        */

        let end = end.unwrap();

        values[..].clone_from_slice(&self.backing[start..end]);

        Ok(())
    }

    fn view_values<A>(&self, address: A, size: usize) -> Result<&[Self::Value], Error>
    where A: Into<Address> {
        let address = address.into();
        let start = usize::from(address);
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBRead {
                address: address.clone(),
                size,
            });
        }

        /*
        if !self.permissions.all_readable(&address, size) {
            return Err(Error::AccessViolation {
                address: AddressValue::new(self.space.clone(), address.into()),
                size,
                access: Access::Read,
            })
        }
        */

        let end = end.unwrap();

        Ok(&self.backing[start..end])
    }

    fn view_values_mut<A>(&mut self, address: A, size: usize) -> Result<&mut [Self::Value], Error>
    where A: Into<Address> {
        let address = address.into();
        let start = usize::from(address);
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBRead {
                address: address.clone(),
                size,
            });
        }

        /*
        if !self.permissions.all_readable_and_writable(&address, size) {
            return Err(Error::AccessViolation {
                address: AddressValue::new(self.space.clone(), address.into()),
                size,
                access: Access::ReadWrite,
            })
        }
        */

        let end = end.unwrap();

        self.dirty.dirty_region(&address, size);

        Ok(&mut self.backing[start..end])
    }

    fn set_values<A>(&mut self, address: A, values: &[Self::Value]) -> Result<(), Error>
    where A: Into<Address> {
        let address = address.into();
        let size = values.len();
        let start = usize::from(address);
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBWrite {
                address: address.clone(),
                size,
            });
        }

        /*
        if !self.permissions.all_writable(&address, size) {
            return Err(Error::AccessViolation {
                address: AddressValue::new(self.space.clone(), address.into()),
                size,
                access: Access::Write,
            })
        }
        */

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
        /*
        Self {
            indices: Vec::with_capacity(self.indices.capacity()),
            bitsmap: vec![0 as u64; self.bitsmap.len()],
        }
        */
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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Access {
    Read,
    Write,
    ReadWrite,
}

impl fmt::Display for Access {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Access::Read => write!(f, "read"),
            Access::Write => write!(f, "write"),
            Access::ReadWrite => write!(f, "read/write")
        }
    }
}

impl Access {
    #[inline]
    pub fn is_read(&self) -> bool {
        matches!(self, Access::Read)
    }

    #[inline]
    pub fn is_write(&self) -> bool {
        matches!(self, Access::Write)
    }

    #[inline]
    pub fn is_read_write(&self) -> bool {
        matches!(self, Access::ReadWrite)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Permissions {
    bitsmap: Vec<u64>,
    space: Arc<AddressSpace>,
}

const PERM_READ_OFF: usize = 1;
const PERM_WRITE_OFF: usize = 0;
const PERM_READ_WRITE_OFF: usize = 0;

const PERM_READ_MASK: u64 = 0xAAAAAAAAAAAAAAAA;
const PERM_WRITE_MASK: u64 = 0x5555555555555555;

const PERM_SCALE: usize = (size_of::<u64>() << 3) >> 1;
const PERM_SELECT: usize = 1;

impl Permissions {
    pub fn new(space: Arc<AddressSpace>, size: usize) -> Self {
        Self::new_with(space, size, PERM_READ_MASK | PERM_WRITE_MASK)
    }

    #[inline]
    pub fn new_with(space: Arc<AddressSpace>, size: usize, mask: u64) -> Self {
        Self {
            // NOTE: we represent each permission by two bits and set
            // each byte to readable by default
            bitsmap: vec![mask; 1 + size / PERM_SCALE],
            space,
        }
    }

    pub fn restore(&mut self, other: &Permissions) {
        for (t, s) in self.bitsmap.iter_mut().zip(other.bitsmap.iter()) {
            *t = *s;
        }
    }

    #[inline]
    pub fn is_marked(&self, address: &Address, access: Access) -> bool {
        let address = u64::from(address);
        let index = (address / PERM_SCALE as u64) as usize;
        let bit = ((address % PERM_SCALE as u64) as usize) << PERM_SELECT;
        let check = if access.is_read_write() {
            0b11 << (bit + PERM_READ_WRITE_OFF)
        } else {
            1 << if access.is_read() {
                bit + PERM_READ_OFF
            } else {
                bit + PERM_WRITE_OFF
            }
        };

        self.bitsmap[index] & check == check
    }

    #[inline]
    pub fn is_readable(&self, address: &Address) -> bool {
        self.is_marked(address, Access::Read)
    }

    #[inline]
    pub fn is_writable(&self, address: &Address) -> bool {
        self.is_marked(address, Access::Write)
    }

    #[inline]
    pub fn is_readable_and_writable(&self, address: &Address) -> bool {
        self.is_marked(address, Access::ReadWrite)
    }

    #[inline]
    pub fn all_marked(&self, address: &Address, size: usize, access: Access) -> bool {
        let start = u64::from(address);
        for addr in start..(start + size as u64) {
            if !self.is_marked(&Address::new(self.space.as_ref(), addr), access) {
                return false
            }
        }
        true
    }

    #[inline]
    pub fn all_readable(&self, address: &Address, size: usize) -> bool {
        self.all_marked(address, size, Access::Read)
    }

    #[inline]
    pub fn all_writable(&self, address: &Address, size: usize) -> bool {
        self.all_marked(address, size, Access::Write)
    }

    #[inline]
    pub fn all_readable_and_writable(&self, address: &Address, size: usize) -> bool {
        self.all_marked(address, size, Access::ReadWrite)
    }

    #[inline]
    pub fn clear_byte(&mut self, address: &Address, access: Access) {
        let address = u64::from(address);
        let index = (address / PERM_SCALE as u64) as usize;
        let bit = ((address % PERM_SCALE as u64) as usize) << PERM_SELECT;
        let check = if access.is_read_write() {
            0b11 << (bit + PERM_READ_WRITE_OFF)
        } else {
            1 << if access.is_read() {
                bit + PERM_READ_OFF
            } else {
                bit + PERM_WRITE_OFF
            }
        };
        self.bitsmap[index] &= !check;
    }

    #[inline]
    pub fn set_byte(&mut self, address: &Address, access: Access) {
        let address = u64::from(address);
        let index = (address / PERM_SCALE as u64) as usize;
        let bit = ((address % PERM_SCALE as u64) as usize) << PERM_SELECT;
        let check = if access.is_read_write() {
            0b11 << (bit + PERM_READ_WRITE_OFF)
        } else {
            1 << if access.is_read() {
                bit + PERM_READ_OFF
            } else {
                bit + PERM_WRITE_OFF
            }
        };

        self.bitsmap[index] |= check;
    }

    #[inline]
    pub fn clear_region(&mut self, address: &Address, size: usize, access: Access) {
        let start = u64::from(address);
        for addr in start..(start + size as u64) {
            self.clear_byte(&Address::new(self.space.as_ref(), addr), access);
        }
    }

    #[inline]
    pub fn set_region(&mut self, address: &Address, size: usize, access: Access) {
        let start = u64::from(address);
        for addr in start..(start + size as u64) {
            self.set_byte(&Address::new(self.space.as_ref(), addr), access);
        }
    }
}
