use std::cell::{Cell, RefCell};

use fugue_ir::disassembly::IRBuilderArena;
use fugue_ir::Address;

use thiserror::Error;

use yaxpeax_arch::*;
use yaxpeax_arm::armv8::a64::Opcode;

pub use yaxpeax_arm::armv8::a64::{
    DecodeError as AArch64DecoderError, InstDecoder as AArch64InstDecoder, Instruction as AArch64Instruction,
};

use crate::ir::PCode;
use crate::lifter::{InsnLifter, LiftedInsn, LiftedInsnProperties, Lifter};

pub struct AArch64InsnLifter {
    decoder: AArch64InstDecoder,
}

#[derive(Debug, Error)]
pub enum AArch64LifterError {
    #[error(transparent)]
    Decoder(#[from] AArch64DecoderError),
    #[error(transparent)]
    Lifter(#[from] fugue_ir::error::Error),
}

impl AArch64InsnLifter {
    pub fn new() -> Self {
        Self::new_with(AArch64InstDecoder::default())
    }

    pub fn new_with(decoder: AArch64InstDecoder) -> Self {
        Self { decoder }
    }
}

fn should_lift(insn: &AArch64Instruction) -> bool {
    match insn.opcode {
        Opcode::B
        | Opcode::BL
        | Opcode::CBZ
        | Opcode::CBNZ
        | Opcode::SVC => true,
        _ => false,
    }
}

impl<'a> InsnLifter<'a, AArch64Instruction> for AArch64InsnLifter {
    type Error = AArch64LifterError;

    fn properties<'b>(
        &mut self,
        lifter: &mut Lifter,
        irb: &'a IRBuilderArena,
        address: Address,
        bytes: &'b [u8],
    ) -> Result<LiftedInsn<'a, 'b, AArch64Instruction>, Self::Error> {
        let mut reader = yaxpeax_arch::U8Reader::new(bytes);
        let insn = self.decoder.decode(&mut reader)?;
        let size = insn.len().to_const() as u8;

        if should_lift(&insn) {
            let PCode {
                address,
                operations,
                delay_slots,
                length,
            } = lifter.lift(irb, address, bytes)?;

            Ok(LiftedInsn {
                address,
                bytes,
                properties: Cell::new(LiftedInsnProperties::default()),
                operations: RefCell::new(Some(operations)),
                delay_slots,
                length,
                data: insn,
            })
        } else {
            Ok(LiftedInsn {
                address,
                bytes,
                properties: Cell::new(LiftedInsnProperties::default()),
                operations: RefCell::new(None),
                delay_slots: 0,
                length: size,
                data: insn,
            })
        }
    }
}
