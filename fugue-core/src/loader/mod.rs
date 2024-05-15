use std::borrow::Cow;
use std::fmt::{Debug, Display};
use std::ops::Deref;

use fugue_bytes::Endian;
use fugue_ir::Address;
use thiserror::Error;

use crate::attributes::common::CompilerConvention;
use crate::attributes::Attribute;
use crate::language::{Language, LanguageBuilder, LanguageBuilderError};
use crate::util::BytesOrMapping;

pub mod object;

pub use self::object::Object;

#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("cannot load object: {0}")]
    Format(anyhow::Error),
    #[error(transparent)]
    Language(#[from] LanguageBuilderError),
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

pub trait Loadable<'a>: Sized {
    fn new(data: impl Into<BytesOrMapping<'a>>) -> Result<Self, LoaderError>;

    fn get_attr<T>(&self) -> Option<&T>
    where
        T: Attribute<'a>;

    fn get_attr_as<'slf, T, U>(&'slf self) -> Option<&'slf U>
    where
        T: Attribute<'a> + AsRef<U> + 'slf,
        U: ?Sized,
    {
        self.get_attr::<T>().map(T::as_ref)
    }

    fn get_attr_as_deref<'slf, T, U>(&'slf self) -> Option<&'slf U>
    where
        T: Attribute<'a> + Deref<Target = U> + 'slf,
        U: ?Sized,
    {
        self.get_attr::<T>().map(T::deref)
    }

    fn set_attr<T>(&mut self, attr: T)
    where
        T: Attribute<'a>;

    fn endian(&self) -> Endian;

    fn language(&self, builder: &LanguageBuilder) -> Result<Language, LoaderError> {
        let convention = self
            .get_attr_as::<CompilerConvention, _>()
            .unwrap_or("default");
        self.language_with(builder, convention)
    }

    fn language_with(
        &self,
        builder: &LanguageBuilder,
        convention: impl AsRef<str>,
    ) -> Result<Language, LoaderError>;

    fn entry(&self) -> Option<Address> {
        None
    }

    fn segments<'slf>(&'slf self) -> impl Iterator<Item = LoadableSegment<'slf>>;
}

pub struct LoadableSegment<'a> {
    addr: Address,
    data: Cow<'a, [u8]>,
}

impl<'a> From<Vec<u8>> for LoadableSegment<'a> {
    fn from(value: Vec<u8>) -> Self {
        Self::new(0u32, value)
    }
}

impl<'a> From<&'a [u8]> for LoadableSegment<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self::new(0u32, value)
    }
}

impl<'a> LoadableSegment<'a> {
    pub fn new(addr: impl Into<Address>, data: impl Into<Cow<'a, [u8]>>) -> Self {
        Self {
            addr: addr.into(),
            data: data.into(),
        }
    }

    pub fn address(&self) -> Address {
        self.addr
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_ref()
    }

    pub fn into_parts(self) -> (Address, Cow<'a, [u8]>) {
        (self.addr, self.data)
    }
}
