use std::fmt;
use std::ops::Add;

use fugue_ir::{Address, VarnodeData};
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Archive, Deserialize, Serialize)]
pub struct Location {
    #[with(crate::types::address::Address)]
    pub address: Address,
    pub position: u32,
}

impl Default for Location {
    fn default() -> Self {
        Address::from(0u32).into()
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.address, self.position)
    }
}

impl Add<u32> for Location {
    type Output = Self;

    fn add(self, rhs: u32) -> Self::Output {
        Self {
            position: self.position + rhs,
            ..self
        }
    }
}

impl Add<usize> for Location {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self {
            position: self.position + rhs as u32,
            ..self
        }
    }
}

impl Location {
    pub fn new(address: impl Into<Address>, position: u32) -> Location {
        Self {
            address: address.into(),
            position,
        }
    }

    pub fn address(&self) -> Address {
        self.address
    }

    pub fn position(&self) -> u32 {
        self.position
    }

    pub(super) fn absolute_from(base: Address, address: VarnodeData, position: u32) -> Self {
        if !address.space().is_constant() {
            return Self::new(address.offset(), 0); // position);
        }

        let offset = address.offset() as i64;
        let position = if offset.is_negative() {
            position
                .checked_sub(offset.abs() as u32)
                .expect("negative offset from position in valid range")
        } else {
            position
                .checked_add(offset as u32)
                .expect("positive offset from position in valid range")
        };

        Self {
            address: base.into(),
            position,
        }
    }
}

impl<T> From<T> for Location where Address: From<T> {
    fn from(value: T) -> Self {
        Self {
            address: value.into(),
            position: 0,
        }
    }
}
