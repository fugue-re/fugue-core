use fugue_ir::disassembly::IRBuilderArena;
use fugue_ir::Address;

use yaxpeax_arch::*;
use yaxpeax_arm::armv7::{Opcode, Operand, Reg};

pub use yaxpeax_arm::armv7::{
    DecodeError as ARMDecoderError, InstDecoder as ARMInstDecoder, Instruction as ARMInstruction,
};

use crate::lifter::{InsnLifter, LiftedInsn, Lifter, LifterError};

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

        let props = if should_lift(&insn) {
            LiftedInsn::new_lifted(lifter, irb, address, bytes)?
        } else {
            LiftedInsn::new_lazy(address, bytes, size)
        };

        Ok(props)
    }
}
