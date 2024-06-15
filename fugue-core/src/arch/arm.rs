use std::cell::{Cell, RefCell};

use fugue_ir::disassembly::IRBuilderArena;
use fugue_ir::Address;

use yaxpeax_arch::*;
use yaxpeax_arm::armv7::{Opcode, Operand, Reg};

pub use yaxpeax_arm::armv7::{
    DecodeError as ARMDecoderError, InstDecoder as ARMInstDecoder, Instruction as ARMInstruction,
};

use crate::ir::PCode;
use crate::lifter::{InsnLifter, LiftedInsn, LiftedInsnProperties, Lifter, LifterError};

pub struct ARMInsnLifter {
    decoder: ARMInstDecoder,
}

impl ARMInsnLifter {
    pub fn new() -> Self {
        Self::new_with(ARMInstDecoder::armv7())
    }

    pub fn new_with(decoder: ARMInstDecoder) -> Self {
        Self { decoder }
    }

    pub fn boxed(self) -> Box<dyn InsnLifter> {
        Box::new(self)
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

impl InsnLifter for ARMInsnLifter {
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
