use crate::space::AddressSpace;

use super::operand::Operand;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Register<'space> {
    pub(crate) name: &'space str,
    pub(crate) space: &'space AddressSpace,
    pub(crate) offset: u64,
    pub(crate) size: usize,
}

impl<'space> From<Register<'space>> for Operand<'space> {
    fn from(src: Register<'space>) -> Operand<'space> {
        Operand::Register {
            offset: src.offset(),
            size: src.size(),
            name: src.name(),
            space: src.space(),
        }
    }
}

impl<'space> Register<'space> {
    #[inline]
    pub fn name(&self) -> &'space str {
        &self.name
    }

    #[inline]
    pub fn offset(&self) -> u64 {
        self.offset
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }

    #[inline]
    pub fn space(&self) -> &'space AddressSpace {
        self.space
    }
}
