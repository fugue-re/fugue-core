use std::ops::{Deref, DerefMut};

use fugue_ir::{Address, Translator};

use crate::flat::FlatState;
use crate::traits::{State, StateOps, StateValue};

pub use crate::flat::Error;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct UniqueState<T: StateValue>(FlatState<T>);

impl<T: StateValue> AsRef<Self> for UniqueState<T> {
    #[inline(always)]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T: StateValue> AsMut<Self> for UniqueState<T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T: StateValue> AsRef<FlatState<T>> for UniqueState<T> {
    #[inline(always)]
    fn as_ref(&self) -> &FlatState<T> {
        &self.0
    }
}

impl<T: StateValue> AsMut<FlatState<T>> for UniqueState<T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut FlatState<T> {
        &mut self.0
    }
}

impl<T: StateValue> Deref for UniqueState<T> {
    type Target = FlatState<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: StateValue> DerefMut for UniqueState<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: StateValue> From<UniqueState<T>> for FlatState<T> {
    fn from(t: UniqueState<T>) -> Self {
        t.0
    }
}

impl<V: StateValue> State for UniqueState<V> {
    type Error = Error;

    #[inline(always)]
    fn fork(&self) -> Self {
        Self(self.0.fork())
    }

    #[inline(always)]
    fn restore(&mut self, other: &Self) {
        self.0.restore(&other.0)
    }
}

impl<V: StateValue> StateOps for UniqueState<V> {
    type Value = V;

    #[inline(always)]
    fn len(&self) -> usize {
        self.0.len()
    }

    #[inline(always)]
    fn copy_values<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Self::Error>
    where F: Into<Address>,
          T: Into<Address> {
        self.0.copy_values(from, to, size)
    }

    #[inline(always)]
    fn get_values<A>(&self, address: A, bytes: &mut [Self::Value]) -> Result<(), Self::Error>
    where A: Into<Address> {
        self.0.get_values(address, bytes)
    }

    #[inline(always)]
    fn view_values<A>(&self, address: A, size: usize) -> Result<&[Self::Value], Self::Error>
    where A: Into<Address> {
        self.0.view_values(address, size)
    }

    #[inline(always)]
    fn view_values_mut<A>(&mut self, address: A, size: usize) -> Result<&mut [Self::Value], Self::Error>
    where A: Into<Address> {
        self.0.view_values_mut(address, size)
    }

    #[inline(always)]
    fn set_values<A>(&mut self, address: A, bytes: &[Self::Value]) -> Result<(), Self::Error>
    where A: Into<Address> {
        self.0.set_values(address, bytes)
    }
}

impl<T: StateValue> UniqueState<T> {
    pub fn new(translator: &Translator) -> Self {
        let space = translator.manager().unique_space();
        let size = translator.unique_space_size();

        log::debug!("unique space size: {} bytes", size);

        Self(FlatState::new(space, size))
    }
}
