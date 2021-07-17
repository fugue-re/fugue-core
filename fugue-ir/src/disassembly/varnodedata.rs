use std::fmt;
use std::sync::Arc;

use crate::address::AddressValue;
use crate::space::AddressSpace;
use crate::Translator;

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct VarnodeData {
    space: Arc<AddressSpace>,
    pub(crate) offset: u64,
    pub(crate) size: usize,
}

pub struct VarnodeDataFormatter<'a> {
    varnode: &'a VarnodeData,
    translator: &'a Translator,
}

impl<'a> VarnodeDataFormatter<'a> {
    fn new(varnode: &'a VarnodeData, translator: &'a Translator) -> Self {
        Self {
            varnode,
            translator,
        }
    }
}

impl<'a> fmt::Display for VarnodeDataFormatter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if self.varnode.space.is_register() {
            if let Some(name) = self
                .translator
                .registers()
                .get(&(self.varnode.offset, self.varnode.size))
            {
                write!(f, "Register(name={}, size={})", name, self.varnode.size)?;
                return Ok(());
            }
        } else if self.varnode.space.is_constant() {
            write!(
                f,
                "Constant(value={:#x}, size={})",
                self.varnode.offset, self.varnode.size
            )?;
            return Ok(());
        }

        write!(
            f,
            "Varnode(space={}, offset={:#x}, size={})",
            self.varnode.space.name(),
            self.varnode.offset,
            self.varnode.size
        )
    }
}

impl VarnodeData {
    pub fn display<'a>(&'a self, translator: &'a Translator) -> VarnodeDataFormatter<'a> {
        VarnodeDataFormatter::new(self, translator)
    }

    pub fn new(space: Arc<AddressSpace>, offset: u64, size: usize) -> Self {
        Self {
            space,
            offset,
            size,
        }
    }

    pub fn address(&self) -> AddressValue {
        AddressValue::new(self.space.clone(), self.offset)
    }

    pub fn space(&self) -> Arc<AddressSpace> {
        self.space.clone()
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn size(&self) -> usize {
        self.size
    }
}
