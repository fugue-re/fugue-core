use std::sync::Arc;

use crate::space::AddressSpace;

use super::operand::Operand;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Register {
    pub(crate) name: Arc<str>,
    pub(crate) space: Arc<AddressSpace>,
    pub(crate) offset: u64,
    pub(crate) size: usize,
}

impl From<Register> for Operand {
    fn from(src: Register) -> Operand {
        Operand::Register {
            offset: src.offset(),
            size: src.size(),
            name: src.name.clone(),
            space: src.space(),
        }
    }
}

impl Register {
    #[inline]
    pub fn name(&self) -> &str {
        self.name.as_ref()
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
    pub fn space(&self) -> Arc<AddressSpace> {
        self.space.clone()
    }
}
