use std::sync::Arc;
use super::operand::Operand;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Register {
    pub(crate) name: Arc<str>,
    pub(crate) offset: u64,
    pub(crate) size: usize,
}

impl From<Register> for Operand {
    fn from(src: Register) -> Operand {
        Operand::Register {
            offset: src.offset(),
            size: src.size(),
            name: src.name,
        }
    }
}

impl Register {
    pub fn new<N>(name: N, offset: u64, size: usize) -> Self
    where N: Into<Arc<str>> {
        Self {
            name: name.into(),
            offset,
            size,
        }
    }

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
}
