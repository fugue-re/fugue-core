use std::fmt;
use bumpalo::collections::String as BString;

use crate::AddressValue;

#[derive(Debug, Clone)]
pub struct Instruction<'z> {
    pub address: AddressValue,
    pub mnemonic: BString<'z>,
    pub operands: BString<'z>,
    pub delay_slots: usize,
    pub length: usize,
}

impl<'z> Instruction<'z> {
    pub fn address(&self) -> AddressValue {
        self.address.clone()
    }

    pub fn mnemonic(&self) -> &str {
        &self.mnemonic
    }

    pub fn operands(&self) -> &str {
        &self.operands
    }

    pub fn delay_slots(&self) -> usize {
        self.delay_slots
    }

    pub fn length(&self) -> usize {
        self.length
    }

    pub fn display<'insn>(&'insn self) -> InstructionFormatter<'insn> {
        InstructionFormatter { insn: self }
    }
}

pub struct InstructionFormatter<'insn> {
    insn: &'insn Instruction<'insn>,
}

impl<'insn> fmt::Display for InstructionFormatter<'insn> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{} {}", self.insn.address, self.insn.mnemonic.trim())?;
        if !self.insn.operands.is_empty() {
            write!(f, " {}", self.insn.operands.trim())?;
        }
        Ok(())
    }
}
