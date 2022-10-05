use std::borrow::Borrow;
use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

use fugue_bv::BitVec;

use crate::disassembly::IRBuilderArena;
use crate::space::{AddressSpace, AddressSpaceId};
use crate::space_manager::{FromSpace, SpaceManager};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct AddressValue {
    space: AddressSpaceId,
    word_size: u32,
    highest: u64,
    offset: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
#[repr(transparent)]
pub struct Address(u64);

impl Address {
    pub fn new(space: &AddressSpace, offset: u64) -> Self {
        Self(space.wrap_offset(offset))
    }

    pub fn from_value<V: Into<Address>>(value: V) -> Self {
        value.into()
    }

    pub fn offset(&self) -> u64 {
        self.0
    }
}

impl PartialEq<u32> for Address {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other as u64
    }
}

impl PartialEq<u64> for Address {
    fn eq(&self, other: &u64) -> bool {
        self.0 == *other
    }
}

impl PartialEq<Address> for u32 {
    fn eq(&self, other: &Address) -> bool {
        *self as u64 == other.0
    }
}

impl PartialEq<Address> for u64 {
    fn eq(&self, other: &Address) -> bool {
        *self == other.0
    }
}

impl From<u64> for Address {
    fn from(v: u64) -> Self {
        Self(v)
    }
}

impl From<u32> for Address {
    fn from(v: u32) -> Self {
        Self(v as u64)
    }
}

impl From<u16> for Address {
    fn from(v: u16) -> Self {
        Self(v as u64)
    }
}

impl From<u8> for Address {
    fn from(v: u8) -> Self {
        Self(v as u64)
    }
}

impl From<AddressValue> for Address {
    fn from(v: AddressValue) -> Self {
        Self(v.offset())
    }
}

impl From<&'_ AddressValue> for Address {
    fn from(v: &AddressValue) -> Self {
        Self(v.offset())
    }
}

impl<'z> FromSpace<'z, Address> for AddressValue {
    fn from_space(t: Address, manager: &SpaceManager) -> Self {
        AddressValue::new(manager.default_space_ref(), t.offset())
    }

    fn from_space_with(t: Address, _arena: &'z IRBuilderArena, manager: &SpaceManager) -> Self {
        AddressValue::new(manager.default_space_ref(), t.offset())
    }
}

pub trait IntoAddress {
    fn into_address(self, space: &AddressSpace) -> Address;
    fn into_address_value(self, space: &AddressSpace) -> AddressValue;
}

impl IntoAddress for Address {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self.0)
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, self.0)
    }
}

impl IntoAddress for &'_ Address {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self.0)
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, self.0)
    }
}

impl IntoAddress for AddressValue {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self.offset)
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, self.offset)
    }
}

impl IntoAddress for &'_ AddressValue {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self.offset)
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, self.offset)
    }
}

impl IntoAddress for usize {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self as u64)
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, self as u64)
    }
}

impl IntoAddress for &'_ usize {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, *self as u64)
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, *self as u64)
    }
}

impl IntoAddress for u8 {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self as u64)
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, self as u64)
    }
}

impl IntoAddress for &'_ u8 {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, *self as u64)
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, *self as u64)
    }
}

impl IntoAddress for u16 {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self as u64)
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, self as u64)
    }
}

impl IntoAddress for &'_ u16 {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, *self as u64)
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, *self as u64)
    }
}

impl IntoAddress for u32 {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self as u64)
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, self as u64)
    }
}

impl IntoAddress for &'_ u32 {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, *self as u64)
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, *self as u64)
    }
}

impl IntoAddress for u64 {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self)
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, self)
    }
}

impl IntoAddress for &'_ u64 {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, *self)
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, *self)
    }
}

impl IntoAddress for BitVec {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self.to_u64().expect("64-bit address limit"))
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, self.to_u64().expect("64-bit address limit"))
    }
}

impl IntoAddress for &'_ BitVec {
    fn into_address(self, space: &AddressSpace) -> Address {
        Address::new(space, self.to_u64().expect("64-bit address limit"))
    }

    fn into_address_value(self, space: &AddressSpace) -> AddressValue {
        AddressValue::new(space, self.to_u64().expect("64-bit address limit"))
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

impl From<Address> for u16 {
    fn from(t: Address) -> Self {
        t.0 as _
    }
}

impl From<&'_ Address> for u16 {
    fn from(t: &'_ Address) -> Self {
        t.0 as _
    }
}

impl From<Address> for u8 {
    fn from(t: Address) -> Self {
        t.0 as _
    }
}

impl From<&'_ Address> for u8 {
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

impl From<AddressValue> for u16 {
    fn from(t: AddressValue) -> Self {
        t.offset as _
    }
}

impl From<&'_ AddressValue> for u16 {
    fn from(t: &'_ AddressValue) -> Self {
        t.offset as _
    }
}

impl From<AddressValue> for u8 {
    fn from(t: AddressValue) -> Self {
        t.offset as _
    }
}

impl From<&'_ AddressValue> for u8 {
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

impl Add<u32> for Address {
    type Output = Self;

    fn add(self, rhs: u32) -> Self {
        Self(self.0.wrapping_add(rhs as u64))
    }
}

impl Sub<u32> for Address {
    type Output = Self;

    fn sub(self, rhs: u32) -> Self {
        Self(self.0.wrapping_sub(rhs as u64))
    }
}

impl AddAssign<Address> for Address {
    fn add_assign(&mut self, rhs: Address) {
        self.0 = self.0.wrapping_add(rhs.0)
    }
}

impl SubAssign<Address> for Address {
    fn sub_assign(&mut self, rhs: Address) {
        self.0 = self.0.wrapping_sub(rhs.0)
    }
}

impl AddAssign<&'_ Address> for Address {
    fn add_assign(&mut self, rhs: &'_ Address) {
        self.0 = self.0.wrapping_add(rhs.0)
    }
}

impl SubAssign<&'_ Address> for Address {
    fn sub_assign(&mut self, rhs: &'_ Address) {
        self.0 = self.0.wrapping_sub(rhs.0)
    }
}

impl AddAssign<usize> for Address {
    fn add_assign(&mut self, rhs: usize) {
        self.0 = self.0.wrapping_add(rhs as u64)
    }
}

impl SubAssign<usize> for Address {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 = self.0.wrapping_sub(rhs as u64)
    }
}

impl AddAssign<u64> for Address {
    fn add_assign(&mut self, rhs: u64) {
        self.0 = self.0.wrapping_add(rhs)
    }
}

impl SubAssign<u64> for Address {
    fn sub_assign(&mut self, rhs: u64) {
        self.0 = self.0.wrapping_sub(rhs)
    }
}

impl AddAssign<u32> for Address {
    fn add_assign(&mut self, rhs: u32) {
        self.0 = self.0.wrapping_add(rhs as u64)
    }
}

impl SubAssign<u32> for Address {
    fn sub_assign(&mut self, rhs: u32) {
        self.0 = self.0.wrapping_sub(rhs as u64)
    }
}

impl fmt::Display for AddressValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:#x}", self.offset * self.word_size as u64)
    }
}

impl Add<AddressValue> for AddressValue {
    type Output = Self;

    fn add(self, rhs: AddressValue) -> Self {
        debug_assert_eq!(self.space, rhs.space);
        Self {
            offset: self.wrap_offset(self.offset.wrapping_add(rhs.offset)),
            ..self
        }
    }
}

impl Sub<AddressValue> for AddressValue {
    type Output = Self;

    fn sub(self, rhs: AddressValue) -> Self {
        debug_assert_eq!(self.space, rhs.space);
        Self {
            offset: self.wrap_offset(self.offset.wrapping_sub(rhs.offset)),
            ..self
        }
    }
}

impl Add<&'_ AddressValue> for AddressValue {
    type Output = Self;

    fn add(self, rhs: &AddressValue) -> Self {
        debug_assert_eq!(self.space, rhs.space);
        Self {
            offset: self.wrap_offset(self.offset.wrapping_add(rhs.offset)),
            ..self
        }
    }
}

impl Sub<&'_ AddressValue> for AddressValue {
    type Output = Self;

    fn sub(self, rhs: &AddressValue) -> Self {
        debug_assert_eq!(self.space, rhs.space);
        Self {
            offset: self.wrap_offset(self.offset.wrapping_sub(rhs.offset)),
            ..self
        }
    }
}

impl Add<Address> for AddressValue {
    type Output = Self;

    fn add(self, rhs: Address) -> Self {
        Self {
            offset: self.wrap_offset(self.offset.wrapping_add(rhs.0)),
            ..self
        }
    }
}

impl Sub<Address> for AddressValue {
    type Output = Self;

    fn sub(self, rhs: Address) -> Self {
        Self {
            offset: self.wrap_offset(self.offset.wrapping_sub(rhs.0)),
            ..self
        }
    }
}

impl Add<&'_ Address> for AddressValue {
    type Output = Self;

    fn add(self, rhs: &Address) -> Self {
        Self {
            offset: self.wrap_offset(self.offset.wrapping_add(rhs.0)),
            ..self
        }
    }
}

impl Sub<&'_ Address> for AddressValue {
    type Output = Self;

    fn sub(self, rhs: &Address) -> Self {
        Self {
            offset: self.wrap_offset(self.offset.wrapping_sub(rhs.0)),
            ..self
        }
    }
}

impl Add<usize> for AddressValue {
    type Output = Self;

    fn add(self, rhs: usize) -> Self {
        Self {
            offset: self.wrap_offset(self.offset.wrapping_add(rhs as u64)),
            ..self
        }
    }
}

impl Sub<usize> for AddressValue {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self {
        Self {
            offset: self.wrap_offset(self.offset.wrapping_sub(rhs as u64)),
            ..self
        }
    }
}

impl Add<u64> for AddressValue {
    type Output = Self;

    fn add(self, rhs: u64) -> Self {
        Self {
            offset: self.wrap_offset(self.offset.wrapping_add(rhs)),
            ..self
        }
    }
}

impl Sub<u64> for AddressValue {
    type Output = Self;

    fn sub(self, rhs: u64) -> Self {
        Self {
            offset: self.wrap_offset(self.offset.wrapping_sub(rhs)),
            ..self
        }
    }
}

impl AddressValue {
    pub fn new<S: Borrow<AddressSpace>>(space: S, offset: u64) -> Self {
        let space = space.borrow();
        let offset = space.wrap_offset(offset);
        Self {
            space: space.id(),
            highest: space.highest_offset(),
            word_size: space.word_size() as u32,
            offset,
        }
    }

    pub fn space(&self) -> AddressSpaceId {
        self.space
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn difference(&self, other: &AddressValue) -> AddressValue {
        // reinterpret other as if it were in `self's` space
        Self {
            offset: self.offset.wrapping_sub(self.wrap_offset(other.offset())),
            space: self.space,
            word_size: self.word_size,
            highest: self.highest,
        }
    }

    pub fn wrap<V: Into<u64>>(&self, offset: V) -> AddressValue {
        Self {
            offset: self.wrap_offset(offset.into()),
            space: self.space,
            word_size: self.word_size,
            highest: self.highest,
        }
    }

    pub fn wrap_offset(&self, offset: u64) -> u64 {
        if offset <= self.highest {
            offset
        } else {
            let m = (self.highest + 1) as i64;
            let r = (offset as i64) % m;
            (if r < 0 { r + m } else { r }) as u64
        }
    }

    pub fn highest_offset(&self) -> u64 {
        self.highest
    }

    pub fn is_constant(&self) -> bool {
        //self.space.kind() == SpaceKind::Constant
        self.space.index() == 0
    }

    /*
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
    */

    pub fn contained_by(&self, size: usize, other: &Self, other_size: usize) -> bool {
        self.space == other.space
            && other.offset <= self.offset
            && other.offset.wrapping_add((other_size - 1) as u64)
                >= self.offset.wrapping_add((size - 1) as u64)
    }

    /*
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
    */
}
