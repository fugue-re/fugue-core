use std::fmt::{Debug, Display, LowerHex, UpperHex};
use std::ops::{Add, AddAssign, Sub, SubAssign};

use fugue_lifter::{Language, Varnode};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Address(u64);

impl Debug for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl LowerHex for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        LowerHex::fmt(&self.0, f)
    }
}

impl UpperHex for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        UpperHex::fmt(&self.0, f)
    }
}

impl AsRef<Address> for Address {
    fn as_ref(&self) -> &Address {
        self
    }
}

impl AsRef<u64> for Address {
    fn as_ref(&self) -> &u64 {
        &self.0
    }
}

impl Default for Address {
    fn default() -> Self {
        Self(0)
    }
}

impl PartialEq<u8> for Address {
    fn eq(&self, other: &u8) -> bool {
        self.0 == *other as u64
    }
}

impl PartialEq<u16> for Address {
    fn eq(&self, other: &u16) -> bool {
        self.0 == *other as u64
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

impl PartialEq<Address> for u8 {
    fn eq(&self, other: &Address) -> bool {
        *self as u64 == other.0
    }
}

impl PartialEq<Address> for u16 {
    fn eq(&self, other: &Address) -> bool {
        *self as u64 == other.0
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

impl Address {
    pub const MAX: Self = Self(u64::MAX);

    pub const fn zero() -> Self {
        Self(0u64)
    }

    pub fn offset(&self) -> u64 {
        self.0
    }

    pub fn wrap(&self, language: &Language) -> Address {
        language.wrap_offset_in_default_space(self.offset()).into()
    }

    pub fn in_space_bounds(&self, language: &Language) -> bool {
        *self == self.wrap(language)
    }

    pub fn range_in_space_bounds(&self, language: &Language, size: usize) -> bool {
        let upper = *self + size;
        *self <= upper && self.in_space_bounds(language) && upper.in_space_bounds(language)
    }
}

pub trait ToAddress {
    fn to_address(&self, language: &Language) -> Option<Address>;
}

impl ToAddress for Varnode {
    fn to_address(&self, language: &Language) -> Option<Address> {
        if language.in_default_space(self) {
            Some(self.offset().into())
        } else {
            None
        }
    }
}
