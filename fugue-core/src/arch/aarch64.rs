use fugue_ir::disassembly::IRBuilderArena;
use fugue_ir::Address;

use yaxpeax_arch::*;
use yaxpeax_arm::armv8::a64::Opcode;

pub use yaxpeax_arm::armv8::a64::{
    DecodeError as AArch64DecoderError, InstDecoder as AArch64InstDecoder,
    Instruction as AArch64Instruction,
};

use crate::lifter::{InsnLifter, LiftedInsn, Lifter, LifterError};

pub struct AArch64InsnLifter {
    decoder: AArch64InstDecoder,
}

impl AArch64InsnLifter {
    pub fn new() -> Self {
        Self::new_with(AArch64InstDecoder::default())
    }

    pub fn new_with(decoder: AArch64InstDecoder) -> Self {
        Self { decoder }
    }

    pub fn boxed(self) -> Box<dyn InsnLifter> {
        Box::new(self)
    }
}

fn should_lift(insn: &AArch64Instruction) -> bool {
    match insn.opcode {
        Opcode::B | Opcode::BL | Opcode::CBZ | Opcode::CBNZ | Opcode::SVC => true,
        _ => false,
    }
}

impl InsnLifter for AArch64InsnLifter {
    fn properties<'input, 'lifter>(
        &mut self,
        lifter: &mut Lifter,
        irb: &'lifter IRBuilderArena,
        address: Address,
        bytes: &'input [u8],
    ) -> Result<LiftedInsn<'input, 'lifter>, LifterError> {
        let mut reader = yaxpeax_arch::U8Reader::new(bytes);
        let insn = self
            .decoder
            .decode(&mut reader)
            .map_err(LifterError::decode)?;
        let size = insn.len().to_const() as u8;

        let props = if should_lift(&insn) {
            LiftedInsn::new_lifted(lifter, irb, address, bytes)?
        } else {
            LiftedInsn::new_lazy(address, bytes, size)
        };

        Ok(props)
    }
}
