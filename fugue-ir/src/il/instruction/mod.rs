use std::fmt;

use crate::Address;

#[derive(Debug, Clone)]
pub struct Instruction<'space> {
    pub address: Address<'space>,
    pub mnemonic: String,
    pub operands: String,
    pub delay_slots: usize,
    pub length: usize,
}

impl<'space> Instruction<'space> {
    pub fn address(&self) -> Address<'space> {
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

    pub fn display<'insn>(&'insn self) -> InstructionFormatter<'insn, 'space> {
        InstructionFormatter { insn: self }
    }
}

pub struct InstructionFormatter<'insn, 'space> {
    insn: &'insn Instruction<'space>,
}

impl<'insn, 'space> fmt::Display for InstructionFormatter<'insn, 'space> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{} {}", self.insn.address, self.insn.mnemonic.trim())?;
        if !self.insn.operands.is_empty() {
            write!(f, " {}", self.insn.operands.trim())?;
        }
        Ok(())
    }
}
