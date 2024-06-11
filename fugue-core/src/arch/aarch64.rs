use std::cell::{Cell, RefCell};

use fugue_ir::disassembly::IRBuilderArena;
use fugue_ir::Address;

use yaxpeax_arch::*;
use yaxpeax_arm::armv8::a64::Opcode;

pub use yaxpeax_arm::armv8::a64::{
    DecodeError as AArch64DecoderError, InstDecoder as AArch64InstDecoder,
    Instruction as AArch64Instruction,
};

use crate::ir::PCode;
use crate::lifter::{InsnLifter, LiftedInsn, LiftedInsnProperties, Lifter, LifterError};

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

        if should_lift(&insn) {
            let PCode {
                address,
                operations,
                delay_slots,
                length,
            } = lifter
                .lift(irb, address, bytes)
                .map_err(LifterError::lift)?;

            Ok(LiftedInsn {
                address,
                bytes,
                properties: Cell::new(LiftedInsnProperties::default()),
                operations: RefCell::new(Some(operations)),
                delay_slots,
                length,
            })
        } else {
            Ok(LiftedInsn {
                address,
                bytes,
                properties: Cell::new(LiftedInsnProperties::default()),
                operations: RefCell::new(None),
                delay_slots: 0,
                length: size,
            })
        }
    }
}
