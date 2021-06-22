use std::fmt;
use std::ops::{Add, Sub};

use crate::space::{AddressSpace, SpaceKind};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address<'a> {
    space: &'a AddressSpace,
    offset: u64,
}

pub trait IntoAddress {
    fn into_address<'a>(self, space: &'a AddressSpace) -> Address<'a>;
}

impl IntoAddress for Address<'_> {
    fn into_address<'a>(self, space: &'a AddressSpace) -> Address<'a> {
        Address::new(space, self.offset())
    }
}

impl IntoAddress for &'_ Address<'_> {
    fn into_address<'a>(self, space: &'a AddressSpace) -> Address<'a> {
        Address::new(space, self.offset())
    }
}

impl IntoAddress for usize {
    fn into_address<'a>(self, space: &'a AddressSpace) -> Address<'a> {
        Address::new(space, self as u64)
    }
}

impl IntoAddress for &'_ usize {
    fn into_address<'a>(self, space: &'a AddressSpace) -> Address<'a> {
        Address::new(space, *self as u64)
    }
}

impl IntoAddress for u32 {
    fn into_address<'a>(self, space: &'a AddressSpace) -> Address<'a> {
        Address::new(space, self as u64)
    }
}

impl IntoAddress for &'_ u32 {
    fn into_address<'a>(self, space: &'a AddressSpace) -> Address<'a> {
        Address::new(space, *self as u64)
    }
}

impl IntoAddress for u64 {
    fn into_address<'a>(self, space: &'a AddressSpace) -> Address<'a> {
        Address::new(space, self)
    }
}

impl IntoAddress for &'_ u64 {
    fn into_address<'a>(self, space: &'a AddressSpace) -> Address<'a> {
        Address::new(space, *self)
    }
}

impl<'space> From<Address<'space>> for usize {
    fn from(t: Address<'space>) -> Self {
        t.offset as _
    }
}

impl<'space> From<&'_ Address<'space>> for usize {
    fn from(t: &'_ Address<'space>) -> Self {
        t.offset as _
    }
}

impl<'space> From<Address<'space>> for u64 {
    fn from(t: Address<'space>) -> Self {
        t.offset as _
    }
}

impl<'space> From<&'_ Address<'space>> for u64 {
    fn from(t: &'_ Address<'space>) -> Self {
        t.offset as _
    }
}

impl<'space> From<Address<'space>> for u32 {
    fn from(t: Address<'space>) -> Self {
        t.offset as _
    }
}

impl<'space> From<&'_ Address<'space>> for u32 {
    fn from(t: &'_ Address<'space>) -> Self {
        t.offset as _
    }
}

impl<'a> fmt::Display for Address<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:#x}", self.offset * self.space.word_size() as u64)
    }
}

impl<'a> Add<Address<'a>> for Address<'a> {
    type Output = Self;

    fn add(self, rhs: Address<'a>) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_add(rhs.offset)),
            space: self.space,
        }
    }
}

impl<'a> Sub<Address<'a>> for Address<'a> {
    type Output = Self;

    fn sub(self, rhs: Address<'a>) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_sub(rhs.offset)),
            space: self.space,
        }
    }
}

impl<'a> Add<&'_ Address<'a>> for Address<'a> {
    type Output = Self;

    fn add(self, rhs: &Address<'a>) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_add(rhs.offset)),
            space: self.space,
        }
    }
}

impl<'a> Sub<&'_ Address<'a>> for Address<'a> {
    type Output = Self;

    fn sub(self, rhs: &Address<'a>) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_sub(rhs.offset)),
            space: self.space,
        }
    }
}

impl<'a> Add<usize> for Address<'a> {
    type Output = Self;

    fn add(self, rhs: usize) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_add(rhs as u64)),
            space: self.space,
        }
    }
}

impl<'a> Sub<usize> for Address<'a> {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_sub(rhs as u64)),
            space: self.space,
        }
    }
}

impl<'a> Add<u64> for Address<'a> {
    type Output = Self;

    fn add(self, rhs: u64) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_add(rhs)),
            space: self.space,
        }
    }
}

impl<'a> Sub<u64> for Address<'a> {
    type Output = Self;

    fn sub(self, rhs: u64) -> Self {
        Self {
            offset: self.space.wrap_offset(self.offset.wrapping_sub(rhs)),
            space: self.space,
        }
    }
}

impl<'a> Address<'a> {
    pub fn new(space: &'a AddressSpace, offset: u64) -> Self {
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

    pub fn space(&self) -> &'a AddressSpace {
        self.space
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn difference(&self, other: &Address<'a>) -> Address<'a> {
        // reinterpret other as if it were in `self's` space
        self.offset.wrapping_sub(self.space.wrap_offset(other.offset()))
            .into_address(self.space)
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
