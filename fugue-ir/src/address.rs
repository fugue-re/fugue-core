use std::fmt;
use std::ops::{Add, Sub};
use std::sync::Arc;

use crate::space::{AddressSpace, SpaceKind};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddressValue {
    space: Arc<AddressSpace>,
    offset: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address(u64);

impl Address {
    pub fn new(space: &AddressSpace, offset: u64) -> Self {
        Self(space.wrap_offset(offset))
    }
}

impl From<AddressValue> for Address {
    fn from(v: AddressValue) -> Self {
        Self(v.offset())
    }
}

pub trait IntoAddress {
    fn into_address(self, space: &AddressSpace) -> Address;
}

impl IntoAddress for Address {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self.0)
    }
}

impl IntoAddress for &'_ Address {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self.0)
    }
}

impl IntoAddress for usize {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self as u64)
    }
}

impl IntoAddress for &'_ usize {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, *self as u64)
    }
}

impl IntoAddress for u32 {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self as u64)
    }
}

impl IntoAddress for &'_ u32 {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, *self as u64)
    }
}

impl IntoAddress for u64 {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self)
    }
}

impl IntoAddress for &'_ u64 {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, *self)
    }
}

impl From<Address> for usize {
    fn from(t: Address) -> Self {
        t.0 as _
    }
}

impl From<&'_ Address> for usize {
    fn from(t: &'_ Address) -> Self {
        t.0 as _
    }
}

impl From<Address> for u64 {
    fn from(t: Address) -> Self {
        t.0 as _
    }
}

impl From<&'_ Address> for u64 {
    fn from(t: &'_ Address) -> Self {
        t.0 as _
    }
}

impl From<Address> for u32 {
    fn from(t: Address) -> Self {
        t.0 as _
    }
}

impl From<&'_ Address> for u32 {
    fn from(t: &'_ Address) -> Self {
        t.0 as _
    }
}

impl From<AddressValue> for usize {
    fn from(t: AddressValue) -> Self {
        t.offset as _
    }
}

impl From<&'_ AddressValue> for usize {
    fn from(t: &'_ AddressValue) -> Self {
        t.offset as _
    }
}

impl From<AddressValue> for u64 {
    fn from(t: AddressValue) -> Self {
        t.offset as _
    }
}

impl From<&'_ AddressValue> for u64 {
    fn from(t: &'_ AddressValue) -> Self {
        t.offset as _
    }
}

impl From<AddressValue> for u32 {
    fn from(t: AddressValue) -> Self {
        t.offset as _
    }
}

impl From<&'_ AddressValue> for u32 {
    fn from(t: &'_ AddressValue) -> Self {
        t.offset as _
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:#x}", self.0)
    }
}

impl Add<Address> for Address {
    type Output = Self;

    fn add(self, rhs: Address) -> Self {
        Self(self.0.wrapping_add(rhs.0))
    }
}

impl Sub<Address> for Address {
    type Output = Self;

    fn sub(self, rhs: Address) -> Self {
        Self(self.0.wrapping_sub(rhs.0))
    }
}

impl Add<&'_ Address> for Address {
    type Output = Self;

    fn add(self, rhs: &Address) -> Self {
        Self(self.0.wrapping_add(rhs.0))
    }
}

impl Sub<&'_ Address> for Address {
    type Output = Self;

    fn sub(self, rhs: &Address) -> Self {
        Self(self.0.wrapping_sub(rhs.0))
    }
}

impl Add<usize> for Address {
    type Output = Self;

    fn add(self, rhs: usize) -> Self {
        Self(self.0.wrapping_add(rhs as u64))
    }
}

impl Sub<usize> for Address {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self {
        Self(self.0.wrapping_sub(rhs as u64))
    }
}

impl Add<u64> for Address {
    type Output = Self;

    fn add(self, rhs: u64) -> Self {
        Self(self.0.wrapping_add(rhs))
    }
}

impl Sub<u64> for Address {
    type Output = Self;

    fn sub(self, rhs: u64) -> Self {
        Self(self.0.wrapping_sub(rhs))
    }
}

impl fmt::Display for AddressValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:#x}", self.offset * self.space.word_size() as u64)
    }
}

impl Add<AddressValue> for AddressValue {
    type Output = Self;

    fn add(self, rhs: AddressValue) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_add(rhs.offset)),
            space: self.space,
        }
    }
}

impl Sub<AddressValue> for AddressValue {
    type Output = Self;

    fn sub(self, rhs: AddressValue) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_sub(rhs.offset)),
            space: self.space,
        }
    }
}

impl Add<&'_ AddressValue> for AddressValue {
    type Output = Self;

    fn add(self, rhs: &AddressValue) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_add(rhs.offset)),
            space: self.space,
        }
    }
}

impl Sub<&'_ AddressValue> for AddressValue {
    type Output = Self;

    fn sub(self, rhs: &AddressValue) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_sub(rhs.offset)),
            space: self.space,
        }
    }
}

impl Add<Address> for AddressValue {
    type Output = Self;

    fn add(self, rhs: Address) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_add(rhs.0)),
            space: self.space,
        }
    }
}

impl Sub<Address> for AddressValue {
    type Output = Self;

    fn sub(self, rhs: Address) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_sub(rhs.0)),
            space: self.space,
        }
    }
}

impl Add<&'_ Address> for AddressValue {
    type Output = Self;

    fn add(self, rhs: &Address) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_add(rhs.0)),
            space: self.space,
        }
    }
}

impl Sub<&'_ Address> for AddressValue {
    type Output = Self;

    fn sub(self, rhs: &Address) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_sub(rhs.0)),
            space: self.space,
        }
    }
}

impl Add<usize> for AddressValue {
    type Output = Self;

    fn add(self, rhs: usize) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_add(rhs as u64)),
            space: self.space,
        }
    }
}

impl Sub<usize> for AddressValue {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_sub(rhs as u64)),
            space: self.space,
        }
    }
}

impl Add<u64> for AddressValue {
    type Output = Self;

    fn add(self, rhs: u64) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_add(rhs)),
            space: self.space,
        }
    }
}

impl Sub<u64> for AddressValue {
    type Output = Self;

    fn sub(self, rhs: u64) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_sub(rhs)),
            space: self.space,
        }
    }
}

impl AddressValue {
    pub fn new(space: Arc<AddressSpace>, offset: u64) -> Self {
        let offset = space.wrap_offset(offset);
        Self { space, offset, }
    }

    pub fn is_big_endian(&self) -> bool {
        self.space.properties().is_big_endian()
    }

    pub fn is_little_endian(&self) -> bool {
        !self.space.properties().is_big_endian()
    }

    pub fn address_size(&self) -> usize {
        self.space.address_size()
    }

    pub fn space(&self) -> Arc<AddressSpace> {
        self.space.clone()
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn difference(&self, other: &AddressValue) -> AddressValue {
        // reinterpret other as if it were in `self's` space
        Self::new(self.space.clone(), self.offset.wrapping_sub(self.space.wrap_offset(other.offset())))
    }

    pub fn is_constant(&self) -> bool {
        self.space.kind() == SpaceKind::Constant
    }

    pub fn is_contiguous(&self, size: usize, other: &Self, other_size: usize) -> bool {
        if self.space != other.space {
            false
        } else if self.is_big_endian() {
            self.space
                .wrap_offset(self.offset.wrapping_add(size as u64))
                == other.offset
        } else {
            self.space
                .wrap_offset(other.offset.wrapping_add(other_size as u64))
                == self.offset
        }
    }

    pub fn contained_by(&self, size: usize, other: &Self, other_size: usize) -> bool {
        self.space == other.space
            && other.offset <= self.offset
            && other.offset.wrapping_add((other_size - 1) as u64)
                >= self.offset.wrapping_add((size - 1) as u64)
    }

    pub fn justified_contain(
        &self,
        size: usize,
        other: &Self,
        other_size: usize,
        force_left: bool,
    ) -> Option<u64> {
        if self.space != other.space || other.offset < self.offset {
            None
        } else {
            let off1 = self.offset.wrapping_add((size - 1) as u64);
            let off2 = other.offset.wrapping_add((other_size - 1) as u64);
            if off2 > off1 {
                None
            } else if self.is_big_endian() && !force_left {
                Some(off1 - off2)
            } else {
                Some(other.offset - self.offset)
            }
        }
    }

    pub fn overlap(&self, skip: usize, other: &Self, other_size: usize) -> Option<u64> {
        if self.space != other.space || self.is_constant() {
            None
        } else {
            let dist = self.space.wrap_offset(
                self.offset
                    .wrapping_add(skip as u64)
                    .wrapping_sub(other.offset),
            );
            if dist >= other_size as u64 {
                None
            } else {
                Some(dist)
            }
        }
    }
}
