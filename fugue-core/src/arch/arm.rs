use std::cell::{Cell, RefCell};

use fugue_ir::disassembly::IRBuilderArena;
use fugue_ir::Address;

use thiserror::Error;

use yaxpeax_arch::*;
use yaxpeax_arm::armv7::{Opcode, Operand, Reg};

pub use yaxpeax_arm::armv7::{
    DecodeError as ARMDecoderError, InstDecoder as ARMInstDecoder, Instruction as ARMInstruction,
};

use crate::ir::PCode;
use crate::lifter::{InsnLifter, LiftedInsn, LiftedInsnProperties, Lifter};

pub struct ARMInsnLifter {
    decoder: ARMInstDecoder,
}

#[derive(Debug, Error)]
pub enum ARMLifterError {
    #[error(transparent)]
    Decoder(#[from] ARMDecoderError),
    #[error(transparent)]
    Lifter(#[from] fugue_ir::error::Error),
}

impl ARMInsnLifter {
    pub fn new() -> Self {
        Self::new_with(ARMInstDecoder::armv7())
    }

    pub fn new_with(decoder: ARMInstDecoder) -> Self {
        Self { decoder }
    }
}

fn should_lift(insn: &ARMInstruction) -> bool {
    let pc = Reg::from_u8(15);

    match insn.opcode {
        Opcode::B
        | Opcode::BL
        | Opcode::BX
        | Opcode::CBZ
        | Opcode::CBNZ
        | Opcode::SVC
        | Opcode::BKPT => true,
        Opcode::MOV => insn.operands[0] == Operand::Reg(pc),
        _ => false,
    }
}

impl<'a> InsnLifter<'a, ARMInstruction> for ARMInsnLifter {
    type Error = ARMLifterError;

    fn properties<'b>(
        &mut self,
        lifter: &mut Lifter,
        irb: &'a IRBuilderArena,
        address: Address,
        bytes: &'b [u8],
    ) -> Result<LiftedInsn<'a, 'b, ARMInstruction>, Self::Error> {
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
