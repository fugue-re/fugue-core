use std::borrow::Borrow;
use std::borrow::Cow;
use std::fmt::{Debug, Display};

use bitflags::bitflags;
use fugue_lifter::{Language, Lifter, LifterBuilderError};
use thiserror::Error;

use crate::types::{Address, Attribute};

pub mod object;
pub mod shellcode;

#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("cannot load object: {0}")]
    Format(anyhow::Error),
    #[error(transparent)]
    Lifter(#[from] LifterBuilderError),
}

impl LoaderError {
    pub fn format<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Format(e.into())
    }

    pub fn format_with<M>(m: M) -> Self
    where
        M: Debug + Display + Send + Sync + 'static,
    {
        Self::Format(anyhow::Error::msg(m))
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct LoadableSegmentProperties: u8 {
        const PERM_READ     = 0b0000_0001;
        const PERM_WRITE    = 0b0000_0010;
        const PERM_EXECUTE  = 0b0000_0100;

        const PERM_ALL      = Self::PERM_READ.bits() | Self::PERM_WRITE.bits() | Self::PERM_EXECUTE.bits();

        const UNINITIALISED = 0b0001_0000;
    }
}

impl LoadableSegmentProperties {
    pub fn is_readable(&self) -> bool {
        self.contains(Self::PERM_READ)
    }

    pub fn is_writable(&self) -> bool {
        self.contains(Self::PERM_WRITE)
    }

    pub fn is_executable(&self) -> bool {
        self.contains(Self::PERM_EXECUTE)
    }

    pub fn is_initialised(&self) -> bool {
        !self.is_uninitialised()
    }

    pub fn is_uninitialised(&self) -> bool {
        self.contains(Self::UNINITIALISED)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoadableSegment<'a> {
    name: Cow<'a, str>,
    address: Address,
    properties: LoadableSegmentProperties,
    bytes: Cow<'a, [u8]>,
}

pub trait Loadable {
    fn get_attr<T>(&self, key: impl Borrow<str>) -> Option<T>
    where
        T: Attribute;

    fn set_attr(&mut self, key: impl ToString, val: impl Attribute);

    fn entry_address(&self) -> Option<Address>;

    fn language(&self) -> &'static Language;
    fn lifter(&self) -> Lifter;

    fn segments<'a>(&'a self) -> impl Iterator<Item = LoadableSegment<'a>> + 'a;
}
