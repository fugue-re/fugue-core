use std::borrow::Cow;
use std::fs::File;
use std::io::Error;
use std::ops::Deref;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use memmap2::Mmap;
use object::ReadRef;

pub mod patfind;
pub mod table;

pub enum OwnedOrRef<'a, T> {
    Owned(T),
    Ref(&'a T),
}

impl<'a, T> AsRef<T> for OwnedOrRef<'a, T> {
    fn as_ref(&self) -> &T {
        match self {
            Self::Owned(ref t) => t,
            Self::Ref(t) => t,
        }
    }
}

impl<'a, T> Deref for OwnedOrRef<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<'a, T> From<&'a T> for OwnedOrRef<'a, T> {
    fn from(value: &'a T) -> Self {
        Self::Ref(value)
    }
}

impl<'a, T> From<T> for OwnedOrRef<'a, T> {
    fn from(value: T) -> Self {
        Self::Owned(value)
    }
}

pub enum BytesOrMapping<'a> {
    Bytes(Cow<'a, [u8]>),
    Mapping(Mmap),
}

impl<'a> AsRef<[u8]> for BytesOrMapping<'a> {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Bytes(bytes) => bytes.as_ref(),
            Self::Mapping(mapping) => mapping.as_ref(),
        }
    }
}

impl<'a> Deref for BytesOrMapping<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<'a, T> From<T> for BytesOrMapping<'a>
where
    T: Into<Cow<'a, [u8]>> + 'a,
{
    fn from(value: T) -> Self {
        Self::Bytes(value.into())
    }
}

impl<'a> BytesOrMapping<'a> {
    pub fn from_bytes(bytes: impl Into<Cow<'a, [u8]>>) -> Self {
        BytesOrMapping::Bytes(bytes.into())
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, Error> {
        Ok(BytesOrMapping::Mapping(unsafe {
            Mmap::map(&File::open(&path)?)?
        }))
    }

    pub fn into_owned(self) -> BytesOrMapping<'static> {
        match self {
            Self::Bytes(bytes) => BytesOrMapping::Bytes(Cow::Owned(bytes.into_owned())),
            Self::Mapping(mapping) => BytesOrMapping::Mapping(mapping),
        }
    }

    pub fn into_shared(self) -> SharedBytesOrMapping<'a> {
        SharedBytesOrMapping(Arc::new(self))
    }
}

impl<'a> ReadRef<'a> for &'a BytesOrMapping<'_> {
    fn len(self) -> Result<u64, ()> {
        <&'a [u8] as ReadRef<'a>>::len(<[u8]>::as_ref(self))
    }

    fn read_bytes_at(self, offset: u64, size: u64) -> Result<&'a [u8], ()> {
        <&'a [u8] as ReadRef<'a>>::read_bytes_at(<[u8]>::as_ref(self), offset, size)
    }

    fn read_bytes_at_until(self, range: Range<u64>, delimiter: u8) -> Result<&'a [u8], ()> {
        <&'a [u8] as ReadRef<'a>>::read_bytes_at_until(<[u8]>::as_ref(self), range, delimiter)
    }
}

#[derive(Clone)]
#[repr(transparent)]
pub struct SharedBytesOrMapping<'a>(Arc<BytesOrMapping<'a>>);

impl<'a> AsRef<[u8]> for SharedBytesOrMapping<'a> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<'a> Deref for SharedBytesOrMapping<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl<'a, T> From<T> for SharedBytesOrMapping<'a>
where
    T: Into<Cow<'a, [u8]>> + 'a,
{
    fn from(value: T) -> Self {
        BytesOrMapping::Bytes(value.into()).into_shared()
    }
}

impl<'a> From<BytesOrMapping<'a>> for SharedBytesOrMapping<'a> {
    fn from(value: BytesOrMapping<'a>) -> Self {
        value.into_shared()
    }
}
