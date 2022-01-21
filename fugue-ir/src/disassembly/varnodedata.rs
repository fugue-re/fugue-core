use std::fmt;

use crate::address::AddressValue;
use crate::space::{AddressSpace, AddressSpaceId};
use crate::Translator;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct VarnodeData {
    pub(crate) space: AddressSpaceId,
    pub(crate) offset: u64,
    pub(crate) size: usize,
}

impl Default for VarnodeData {
    fn default() -> Self {
        Self {
            space: AddressSpaceId::constant_id(0usize),
            offset: 0,
            size: 0,
        }
    }
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
        let space = self.translator.manager().unchecked_space_by_id(self.varnode.space);
        if space.is_register() {
            let name = self.translator.registers()
                .get(self.varnode.offset, self.varnode.size)
                .unwrap();
            write!(f, "Register(name={}, size={})", name, self.varnode.size)?;
            return Ok(())
        } else if space.is_constant() {
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
            space.name(),
            self.varnode.offset,
            self.varnode.size
        )
    }
}

impl VarnodeData {
    pub fn display<'a>(&'a self, translator: &'a Translator) -> VarnodeDataFormatter<'a> {
        VarnodeDataFormatter::new(self, translator)
    }

    pub fn new(space: &AddressSpace, offset: u64, size: usize) -> Self {
        Self {
            space: space.id(),
            offset,
            size,
        }
    }

    pub fn address(&self) -> AddressValue {
        todo!()
        //AddressValue::new(self.space.clone(), self.offset)
    }

    pub fn space(&self) -> AddressSpaceId {
        self.space.clone()
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn size(&self) -> usize {
        self.size
    }
}
