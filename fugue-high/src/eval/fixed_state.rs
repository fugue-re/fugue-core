use fugue_bv::BitVec;
use fugue_bytes::{Endian, Order};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FixedStateError {
    #[error("out-of-bounds read access; {size} bytes at {offset:#x}")]
    OOBRead { offset: usize, size: usize },
    #[error("out-of-bounds write access; {size} bytes at {offset:#x}")]
    OOBWrite { offset: usize, size: usize },
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct FixedState {
    pub(crate) backing: Box<[u8]>,
}

impl AsRef<Self> for FixedState {
    #[inline(always)]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl AsMut<Self> for FixedState {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl From<Vec<u8>> for FixedState {
    fn from(backing: Vec<u8>) -> Self {
        Self {
            backing: backing.into_boxed_slice(),
        }
    }
}

impl FixedState {
    pub fn new(size: usize) -> Self {
        Self {
            backing: vec![0u8; size].into_boxed_slice(),
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.backing.len()
    }

    #[inline(always)]
    pub fn read_val<O: Order>(
        &self,
        offset: impl Into<usize>,
        size: usize,
    ) -> Result<BitVec, FixedStateError> {
        let view = self.view_bytes(offset, size)?;

        Ok(if O::ENDIAN.is_big() {
            BitVec::from_le_bytes(view)
        } else {
            BitVec::from_le_bytes(view)
        })
    }

    #[inline(always)]
    pub fn read_val_with(
        &self,
        offset: impl Into<usize>,
        size: usize,
        endian: Endian,
    ) -> Result<BitVec, FixedStateError> {
        let view = self.view_bytes(offset, size)?;

        Ok(if endian.is_big() {
            BitVec::from_le_bytes(view)
        } else {
            BitVec::from_le_bytes(view)
        })
    }

    #[inline(always)]
    pub fn write_val<O: Order>(
        &mut self,
        offset: impl Into<usize>,
        value: &BitVec,
    ) -> Result<(), FixedStateError> {
        debug_assert_eq!(value.bits() % 8, 0);

        let size = value.bits() / 8;
        let view = self.view_bytes_mut(offset, size)?;

        if O::ENDIAN.is_big() {
            value.to_be_bytes(view)
        } else {
            value.to_le_bytes(view)
        }

        Ok(())
    }

    #[inline(always)]
    pub fn write_val_with(
        &mut self,
        offset: impl Into<usize>,
        value: &BitVec,
        endian: Endian,
    ) -> Result<(), FixedStateError> {
        debug_assert_eq!(value.bits() % 8, 0);

        let size = value.bits() / 8;
        let view = self.view_bytes_mut(offset, size)?;

        if endian.is_big() {
            value.to_be_bytes(view)
        } else {
            value.to_le_bytes(view)
        }

        Ok(())
    }

    #[inline(always)]
    pub fn read_bytes(
        &self,
        offset: impl Into<usize>,
        values: &mut [u8],
    ) -> Result<(), FixedStateError> {
        let offset = offset.into();
        let size = values.len();

        let end = offset
            .checked_add(size)
            .ok_or(FixedStateError::OOBRead { offset, size })?;

        if end > self.backing.len() {
            return Err(FixedStateError::OOBRead { offset, size });
        }

        values[..].copy_from_slice(&self.backing[offset..end]);

        Ok(())
    }

    #[inline(always)]
    pub fn view_bytes(
        &self,
        offset: impl Into<usize>,
        size: usize,
    ) -> Result<&[u8], FixedStateError> {
        let offset = offset.into();

        let end = offset
            .checked_add(size)
            .ok_or(FixedStateError::OOBRead { offset, size })?;

        if end > self.backing.len() {
            return Err(FixedStateError::OOBRead { offset, size });
        }

        Ok(&self.backing[offset..end])
    }

    #[inline(always)]
    pub fn view_bytes_mut(
        &mut self,
        offset: impl Into<usize>,
        size: usize,
    ) -> Result<&mut [u8], FixedStateError> {
        let offset = offset.into();

        let end = offset
            .checked_add(size)
            .ok_or(FixedStateError::OOBWrite { offset, size })?;

        if end > self.backing.len() {
            return Err(FixedStateError::OOBWrite { offset, size });
        }

        Ok(&mut self.backing[offset..end])
    }

    #[inline(always)]
    pub fn write_bytes(
        &mut self,
        offset: impl Into<usize>,
        values: &[u8],
    ) -> Result<(), FixedStateError> {
        let offset = offset.into();
        let size = values.len();

        let end = offset
            .checked_add(size)
            .ok_or(FixedStateError::OOBWrite { offset, size })?;

        if end > self.backing.len() {
            return Err(FixedStateError::OOBWrite { offset, size });
        }

        self.backing[offset..end].copy_from_slice(values);

        Ok(())
    }
}
