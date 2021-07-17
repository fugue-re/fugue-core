use std::borrow::Borrow;
use std::fmt;
use std::sync::Arc;
use fnv::FnvHashMap as Map;

use crate::address::AddressValue;
use crate::disassembly::VarnodeData;
use crate::space::AddressSpace;
use crate::space_manager::SpaceManager;
use crate::translator::Translator;

use fugue_bv::BitVec;

use super::Register;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum Operand {
    // RAM
    Address {
        value: AddressValue,
        size: usize,
    },
    Constant {
        value: u64,
        size: usize,
        space: Arc<AddressSpace>,
    },
    Register {
        name: Arc<str>,
        offset: u64,
        size: usize,
        space: Arc<AddressSpace>,
    },
    // Unique address space
    Variable {
        offset: u64,
        size: usize,
        space: Arc<AddressSpace>,
    },
}

impl Operand {
    pub fn from_varnode(translator: &Translator, varnode: &VarnodeData) -> Operand {
        Self::from_varnodedata(translator.manager(), translator.registers(), varnode)
    }

    pub(crate) fn from_varnodedata<V>(manager: &SpaceManager, registers: &Map<(u64, usize), Arc<str>>, vnd: V) -> Operand
    where V: Borrow<VarnodeData> {
        let vnd = vnd.borrow();
        let offset = vnd.offset();
        let size = vnd.size();
        let space = vnd.space();

        if space == manager.default_space() { // address
            Operand::Address {
                value: AddressValue::new(space, offset),
                size,
            }
        } else if space == manager.constant_space() { // constant
            Operand::Constant {
                value: offset,
                space,
                size,
            }
        } else if space == manager.register_space() { // register
            Operand::Register {
                name: registers[&(offset, size)].clone(),
                offset,
                size,
                space,
            }
        } else { // variable
            Operand::Variable {
                offset,
                size,
                space,
            }
        }
    }

    pub fn address(&self) -> Option<AddressValue> {
        if let Self::Address { value, .. } = self {
            Some(value.clone())
        } else {
            None
        }
    }

    pub fn as_bitvec(&self) -> Option<BitVec> {
        if let Self::Constant { value, size, .. } = self {
            Some(BitVec::from_u64(*value, size * 8))
        } else {
            None
        }
    }

    pub fn register(&self) -> Option<Register> {
        if let Self::Register {
            name,
            offset,
            size,
            space,
        } = self
        {
            Some(Register { name: name.clone(), space: space.clone(), offset: *offset, size: *size })
        } else {
            None
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Operand::Address { size, .. }
            | Operand::Constant { size, .. }
            | Operand::Register { size, .. }
            | Operand::Variable { size, .. } => *size,
        }
    }

    pub fn display(&self) -> OperandFormatter {
        OperandFormatter::new(self)
    }
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

impl AsRef<Operand> for Operand {
    fn as_ref(&self) -> &Operand {
        self
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperandCase {
    Default,
    Lower,
    Upper,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperandSize {
    Default,
    AsBits,
    AsBytes,
}

impl Default for OperandSize {
    fn default() -> Self {
        Self::Default
    }
}

pub struct OperandFormatter<'operand> {
    operand: &'operand Operand,
    signed: bool,
    sizes: OperandSize,
    case: OperandCase,
}

impl Default for OperandCase {
    fn default() -> Self {
        Self::Default
    }
}

impl<'operand> OperandFormatter<'operand> {
    pub fn new(operand: &'operand Operand) -> Self {
        Self {
            operand,
            signed: false,
            sizes: OperandSize::default(),
            case: OperandCase::default(),
        }
    }

    pub fn signed(self, signed: bool) -> Self {
        Self { signed, ..self }
    }

    pub fn case(self, case: OperandCase) -> Self {
        Self { case, ..self }
    }

    pub fn sizes(self, sizes: OperandSize) -> Self {
        Self { sizes, ..self }
    }
}

impl<'operand> fmt::Debug for OperandFormatter<'operand> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.operand)
    }
}

impl<'operand> fmt::Display for OperandFormatter<'operand> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.operand {
            Operand::Address { value, .. } => {
                write!(f, "{}", value)?
            },
            Operand::Constant { value, size, .. } => {
                if !self.signed {
                    match size {
                        1 => write!(f, "{:#x}", *value as u8)?,
                        2 => write!(f, "{:#x}", *value as u16)?,
                        4 => write!(f, "{:#x}", *value as u32)?,
                        _ => write!(f, "{:#x}", value)?,
                    }
                } else {
                    match size {
                        1 => {
                            let i = *value as u8 as i8;
                            write!(f, "{}{:#x}", if i < 0 { "-" } else { "" }, i.abs())?
                        }
                        2 => {
                            let i = *value as u16 as i16;
                            write!(f, "{}{:#x}", if i < 0 { "-" } else { "" }, i.abs())?
                        }
                        4 => {
                            let i = *value as u32 as i32;
                            write!(f, "{}{:#x}", if i < 0 { "-" } else { "" }, i.abs())?
                        }
                        _ => {
                            let i = *value as u64 as i64;
                            write!(f, "{}{:#x}", if i < 0 { "-" } else { "" }, i.abs())?
                        }
                    }
                }
            }
            Operand::Register { name, .. } => match self.case {
                OperandCase::Default => write!(f, "{}", name)?,
                OperandCase::Lower => write!(f, "{}", name.to_lowercase())?,
                OperandCase::Upper => write!(f, "{}", name.to_uppercase())?,
            },
            Operand::Variable { offset, .. } => write!(
                f,
                "{}{:04x}",
                if matches!(self.case, OperandCase::Upper) {
                    "VAR"
                } else {
                    "var"
                },
                offset
            )?,
        }
        match self.sizes {
            OperandSize::Default => (),
            OperandSize::AsBits => write!(f, ":{}", self.operand.size() * 8)?,
            OperandSize::AsBytes => write!(f, ":{}", self.operand.size())?,
        }
        Ok(())
    }
}
