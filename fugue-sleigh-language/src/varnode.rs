use std::fmt;

use crate::language::Language;
use crate::spaces::{AddressSpace, AddressSpaceId};

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, serde::Deserialize, serde::Serialize,
)]
pub struct VarnodeData {
    pub(crate) space: AddressSpaceId,
    pub(crate) offset: u64,
    pub(crate) size: u32,
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
    language: &'a Language,
}

impl<'a> VarnodeDataFormatter<'a> {
    fn new(varnode: &'a VarnodeData, language: &'a Language) -> Self {
        Self { varnode, language }
    }
}

impl<'a> fmt::Display for VarnodeDataFormatter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let space = self.language.spaces().space_by_id(self.varnode.space);
        if space.is_register() {
            let name = self
                .language
                .registers()
                .get(self.varnode.offset(), self.varnode.size())
                .unwrap();
            write!(f, "Register(name={}, size={})", name, self.varnode.size)?;
            return Ok(());
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
    pub fn display<'a>(&'a self, language: &'a Language) -> VarnodeDataFormatter<'a> {
        VarnodeDataFormatter::new(self, language)
    }

    pub fn new(space: &AddressSpace, offset: u64, size: usize) -> Self {
        Self {
            space: space.id(),
            offset,
            size: size as _,
        }
    }

    pub fn space(&self) -> AddressSpaceId {
        self.space.clone()
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn size(&self) -> usize {
        self.size as _
    }

    pub fn bits(&self) -> u32 {
        self.size * 8
    }
}
