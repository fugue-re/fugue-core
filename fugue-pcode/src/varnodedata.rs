use std::fmt;

use crate::address::Address;
use crate::space::AddressSpace;
use crate::Translator;

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct VarnodeData<'a> {
    space: &'a AddressSpace,
    pub (crate) offset: u64,
    pub (crate) size: usize,
}

pub struct VarnodeDataFormatter<'a> {
    varnode: &'a VarnodeData<'a>,
    translator: &'a Translator,
}

impl<'a> VarnodeDataFormatter<'a> {
    fn new(varnode: &'a VarnodeData<'a>, translator: &'a Translator) -> Self {
        Self {
            varnode,
            translator,
        }
    }
}

impl<'a> fmt::Display for VarnodeDataFormatter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if self.varnode.space.is_register() {
            if let Some(name) = self.translator.registers().get(&(self.varnode.offset, self.varnode.size)) {
                write!(f, "Register(name={}, size={})", name, self.varnode.size)?;
                return Ok(())
            }
        } else if self.varnode.space.is_constant() {
            write!(f, "Constant(value={:#x}, size={})",
                   self.varnode.offset,
                   self.varnode.size)?;
            return Ok(())
        }

        write!(f, "Varnode(space={}, offset={:#x}, size={})",
               self.varnode.space.name(),
               self.varnode.offset,
               self.varnode.size)
    }
}

impl<'a> VarnodeData<'a> {
    pub fn display(&'a self, translator: &'a Translator) -> VarnodeDataFormatter<'a> {
        VarnodeDataFormatter::new(self, translator)
    }

    pub fn new(space: &'a AddressSpace, offset: u64, size: usize) -> Self {
        Self {
            space,
            offset,
            size,
        }
    }

    pub fn address(&self) -> Address<'a> {
        Address::new(self.space, self.offset)
    }

    pub fn space(&self) -> &'a AddressSpace {
        self.space
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn size(&self) -> usize {
        self.size
    }
}
