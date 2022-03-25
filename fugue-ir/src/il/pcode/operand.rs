use std::fmt;

use crate::address::Address;
use crate::disassembly::VarnodeData;
use crate::register::RegisterNames;
use crate::space::AddressSpaceId;
use crate::space_manager::SpaceManager;
use crate::translator::Translator;

use fugue_bv::BitVec;
use ustr::Ustr;

use super::Register;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum Operand {
    // RAM
    Address {
        value: Address,
        size: usize,
    },
    Constant {
        value: u64,
        size: usize,
    },
    Register {
        name: Ustr,
        offset: u64,
        size: usize,
    },
    Variable {
        offset: u64,
        size: usize,
        space: AddressSpaceId,
    },
}

impl<'z> Operand {
    pub fn from_varnode(translator: &Translator, varnode: VarnodeData) -> Operand {
        Self::from_varnodedata(translator.manager(), translator.registers(), varnode)
    }

    #[inline(always)]
    pub(crate) fn from_varnodedata(
        manager: &SpaceManager,
        registers: &RegisterNames,
        vnd: VarnodeData,
    ) -> Operand {
        let offset = vnd.offset;
        let size = vnd.size;
        let space_id = vnd.space;

        if space_id.is_default() {
            // address
            Operand::Address {
                value: Address::new(manager.default_space_ref(), offset),
                size,
            }
        } else if space_id.is_constant() {
            // constant
            Operand::Constant {
                value: offset,
                size,
            }
        } else if space_id.is_register() {
            // register
            let name = registers.unchecked_get(offset, size).clone();

            Operand::Register {
                name,
                offset,
                size,
            }
        } else {
            // variable
            Operand::Variable {
                offset,
                size,
                space: space_id,
            }
        }
    }

    pub fn address(&self) -> Option<Address> {
        if let Self::Address { value, .. } = self {
            Some(*value)
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
        } = self
        {
            Some(Register {
                name: name.clone(),
                offset: *offset,
                size: *size,
            })
        } else {
            None
        }
    }

    pub fn offset(&self) -> u64 {
        match self {
            Operand::Address { value, .. } => value.offset(),
            Operand::Constant { value, .. } => *value,
            Operand::Register { offset, .. } | Operand::Variable { offset, .. } => *offset,
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

impl<'z> fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

impl<'z> AsRef<Operand> for Operand {
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
            Operand::Address { value, .. } => write!(f, "{}", value)?,
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
