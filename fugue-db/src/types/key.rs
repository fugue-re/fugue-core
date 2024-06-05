use std::ops::Deref;

use arrayvec::ArrayVec;
use fugue_ir::Address;
use thiserror::Error;

// Prefixes

const PREFIX_ADDR: u8 = b'.';
const PREFIX_NAME: u8 = b'$';

// Addressable (. prefix)

// basic block encoding
const BASIC_BLOCK: u8 = b'B';

// function encoding
const FUNCTION: u8 = b'F';

// points to disassembly structure
const DISASSEMBLY: u8 = b'I';

// x-ref kinds
const XREF_CODE_FROM: u8 = b'C';
const XREF_CODE_TO: u8 = b'c';

const XREF_DATA_FROM: u8 = b'D';
const XREF_DATA_TO: u8 = b'd';

#[repr(transparent)]
pub struct KeyBuilder {
    inner: ArrayVec<u8, 64>,
}

#[repr(transparent)]
pub struct Key {
    inner: ArrayVec<u8, 64>,
}

#[derive(Debug, Error)]
pub enum KeyError {
    #[error("cannot build an empty key")]
    Empty,
    #[error("cannot build a named key with no name")]
    NoName,
}

impl KeyBuilder {
    pub fn new() -> Self {
        Self {
            inner: ArrayVec::new(),
        }
    }

    pub fn named(name: impl AsRef<str>) -> Result<Self, KeyError> {
        let name = name.as_ref();
        if name.is_empty() {
            return Err(KeyError::NoName);
        }

        let mut slf = Self::new();
        slf.inner.push(PREFIX_NAME);
        slf.inner.extend(name.bytes());

        Ok(slf)
    }

    pub fn addressable(addr: impl Into<Address>) -> Self {
        let mut slf = Self::new();
        slf.inner.push(PREFIX_ADDR);
        slf.push_address(addr);
        slf
    }

    pub fn basic_block(addr: impl Into<Address>) -> Self {
        let mut slf = Self::addressable(addr);
        slf.inner.push(BASIC_BLOCK);
        slf
    }

    pub fn function(addr: impl Into<Address>) -> Self {
        let mut slf = Self::addressable(addr);
        slf.inner.push(FUNCTION);
        slf
    }

    pub fn disassembly(addr: impl Into<Address>) -> Self {
        let mut slf = Self::addressable(addr);
        slf.inner.push(DISASSEMBLY);
        slf
    }

    pub fn code_ref_from(to: impl Into<Address>, from: impl Into<Address>) -> Self {
        let mut slf = Self::addressable(to);
        slf.inner.push(XREF_CODE_FROM);
        slf.push_address(from);
        slf
    }

    pub fn code_ref_to(from: impl Into<Address>, to: impl Into<Address>) -> Self {
        let mut slf = Self::addressable(from);
        slf.inner.push(XREF_CODE_TO);
        slf.push_address(to);
        slf
    }

    pub fn data_ref_from(to: impl Into<Address>, from: impl Into<Address>) -> Self {
        let mut slf = Self::addressable(to);
        slf.inner.push(XREF_DATA_FROM);
        slf.push_address(from);
        slf
    }

    pub fn data_ref_to(from: impl Into<Address>, to: impl Into<Address>) -> Self {
        let mut slf = Self::addressable(from);
        slf.inner.push(XREF_DATA_TO);
        slf.push_address(to);
        slf
    }

    pub fn push_address(&mut self, addr: impl Into<Address>) {
        self.inner.extend(addr.into().offset().to_le_bytes());
    }

    pub fn into_key(self) -> Result<Key, KeyError> {
        if self.inner.is_empty() {
            return Err(KeyError::Empty);
        }

        Ok(Key { inner: self.inner })
    }
}

impl Key {
    pub fn basic_block(addr: impl Into<Address>) -> Self {
        KeyBuilder::basic_block(addr).into_key().unwrap()
    }

    pub fn function(addr: impl Into<Address>) -> Self {
        KeyBuilder::function(addr).into_key().unwrap()
    }

    pub fn disassembly(addr: impl Into<Address>) -> Self {
        KeyBuilder::disassembly(addr).into_key().unwrap()
    }

    pub fn code_ref_from(to: impl Into<Address>, from: impl Into<Address>) -> Self {
        KeyBuilder::code_ref_from(to, from).into_key().unwrap()
    }

    pub fn code_ref_to(from: impl Into<Address>, to: impl Into<Address>) -> Self {
        KeyBuilder::code_ref_to(from, to).into_key().unwrap()
    }

    pub fn data_ref_from(to: impl Into<Address>, from: impl Into<Address>) -> Self {
        KeyBuilder::data_ref_from(to, from).into_key().unwrap()
    }

    pub fn data_ref_to(from: impl Into<Address>, to: impl Into<Address>) -> Self {
        KeyBuilder::data_ref_to(from, to).into_key().unwrap()
    }
}

impl AsRef<[u8]> for Key {
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}

impl Deref for Key {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}
