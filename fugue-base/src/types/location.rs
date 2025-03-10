use std::fmt;
use std::ops::{Add, AddAssign};

use fugue_lifter::{Language, Varnode};

use crate::types::Address;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Location {
    address: Address,
    position: u16,
}

impl Default for Location {
    fn default() -> Self {
        Address::default().into()
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.address, self.position)
    }
}

impl AsRef<Location> for Location {
    fn as_ref(&self) -> &Location {
        self
    }
}

impl AsRef<Address> for Location {
    fn as_ref(&self) -> &Address {
        &self.address
    }
}

impl Add<u16> for Location {
    type Output = Self;

    fn add(self, rhs: u16) -> Self::Output {
        Self {
            position: self.position + rhs,
            ..self
        }
    }
}

impl AddAssign<u16> for Location {
    fn add_assign(&mut self, rhs: u16) {
        self.position += rhs;
    }
}

impl Add<usize> for Location {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self {
            position: self.position + rhs as u16,
            ..self
        }
    }
}

impl AddAssign<usize> for Location {
    fn add_assign(&mut self, rhs: usize) {
        self.position += rhs as u16;
    }
}

impl Location {
    pub fn new(address: impl Into<Address>, position: u16) -> Location {
        Self {
            address: address.into(),
            position,
        }
    }

    pub fn address(&self) -> Address {
        self.address
    }

    pub fn position(&self) -> u16 {
        self.position
    }

    pub fn absolute_from(
        language: &Language,
        base: Address,
        address: Varnode,
        position: u16,
    ) -> Option<Self> {
        if language.in_default_space(&address) {
            return Some(Self::new(address.offset(), 0));
        }

        if !language.in_constant_space(&address) {
            return None;
        }

        let offset = address.offset() as i64;
        let position = if offset.is_negative() {
            position
                .checked_sub(offset.abs() as u16)
                .expect("negative offset from position in valid range")
        } else {
            position
                .checked_add(offset as u16)
                .expect("positive offset from position in valid range")
        };

        Some(Self {
            address: base.into(),
            position,
        })
    }
}

impl<T> From<T> for Location
where
    Address: From<T>,
{
    fn from(value: T) -> Self {
        Self {
            address: value.into(),
            position: 0,
        }
    }
}
