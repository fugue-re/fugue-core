use fugue_bv::BitVec;
use fugue_bytes::Endian;
use fugue_ir::{Address, VarnodeData};

use crate::lifter::Lifter;

use super::{EvaluatorContext, EvaluatorError, FixedState};

pub struct DummyContext {
    base: Address,
    endian: Endian,
    memory: FixedState,
    registers: FixedState,
    temporaries: FixedState,
}

impl DummyContext {
    pub fn new(lifter: &Lifter, base: impl Into<Address>, size: usize) -> Self {
        let t = lifter.translator();

        Self {
            base: base.into(),
            endian: if t.is_big_endian() {
                Endian::Big
            } else {
                Endian::Little
            },
            memory: FixedState::new(size),
            registers: FixedState::new(t.register_space_size()),
            temporaries: FixedState::new(t.unique_space_size()),
        }
    }

    fn translate(&self, addr: u64) -> Result<usize, EvaluatorError> {
        let addr = addr
            .checked_sub(self.base.into())
            .ok_or(EvaluatorError::state_with(
                "address translation out-of-bounds",
            ))?;

        Ok(addr as usize)
    }
}

impl EvaluatorContext for DummyContext {
    fn read_vnd(&mut self, var: &VarnodeData) -> Result<BitVec, EvaluatorError> {
        let spc = var.space();
        if spc.is_constant() {
            Ok(BitVec::from_u64(var.offset(), var.size() * 8))
        } else if spc.is_register() {
            self.registers
                .read_val_with(var.offset() as usize, var.size(), self.endian)
                .map_err(EvaluatorError::state)
        } else if spc.is_unique() {
            self.temporaries
                .read_val_with(var.offset() as usize, var.size(), self.endian)
                .map_err(EvaluatorError::state)
        } else {
            let addr = self.translate(var.offset())?;
            self.memory
                .read_val_with(addr, var.size(), self.endian)
                .map_err(EvaluatorError::state)
        }
    }

    fn write_vnd(&mut self, var: &VarnodeData, val: &BitVec) -> Result<(), EvaluatorError> {
        let spc = var.space();
        if spc.is_register() {
            self.registers
                .write_val_with(var.offset() as usize, val, self.endian)
                .map_err(EvaluatorError::state)
        } else if spc.is_unique() {
            self.temporaries
                .write_val_with(var.offset() as usize, val, self.endian)
                .map_err(EvaluatorError::state)
        } else if spc.is_default() {
            let addr = self.translate(var.offset())?;
            self.memory
                .write_val_with(addr, val, self.endian)
                .map_err(EvaluatorError::state)
        } else {
            panic!("cannot write to constant Varnode")
        }
    }
}

#[cfg(test)]
mod test {
    use crate::eval::Evaluator;
    use crate::prelude::*;

    use super::*;

    #[test]
    #[ignore]
    fn test_single_step() -> anyhow::Result<()> {
        let lbuilder = LanguageBuilder::new("data")?;
        let language = lbuilder.build("ARM:LE:32:v7", "default")?;

        let memory = &[
            0x03, 0x00, 0x51, 0xE3, 0x0A, 0x00, 0x00, 0x9A, 0x00, 0x30, 0xA0, 0xE3, 0x01, 0x10,
            0x80, 0xE0, 0x03, 0x00, 0x80, 0xE2, 0x01, 0x20, 0xD0, 0xE4, 0x02, 0x30, 0x83, 0xE0,
            0x01, 0x00, 0x50, 0xE1, 0xFF, 0x30, 0x03, 0xE2, 0xFA, 0xFF, 0xFF, 0x1A, 0x00, 0x00,
            0x63, 0xE2, 0xFF, 0x00, 0x00, 0xE2, 0x1E, 0xFF, 0x2F, 0xE1, 0x00, 0x00, 0xA0, 0xE3,
            0x1E, 0xFF, 0x2F, 0xE1,
        ];

        let mut lifter = language.lifter();
        let irb = lifter.irb(1024);

        let pcode = lifter.lift(&irb, 0x15e38u32, memory)?;

        let mut context = DummyContext::new(&lifter, 0x15e38u32, 0x1000);
        let mut evaluator = Evaluator::new(&lifter, &mut context);

        evaluator.context.memory.write_bytes(0usize, memory)?;

        for op in pcode.operations() {
            evaluator.step(0x15e38u32, &op)?;
        }

        Ok(())
    }
}
