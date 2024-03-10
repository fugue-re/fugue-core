use fugue_bv::BitVec;

use fugue_ir::disassembly::{Opcode, PCodeData};
use fugue_ir::il::Location;
use fugue_ir::{Address, AddressSpace, Translator, VarnodeData};

use thiserror::Error;

use crate::lifter::Lifter;

#[derive(Debug, Error)]
pub enum EvaluatorError {
    #[error("invalid address: {0:x}")]
    Address(BitVec),
    #[error("{0}")]
    Lift(fugue_ir::error::Error),
    #[error("unsupported opcode: {0:?}")]
    Unsupported(Opcode),
}

pub trait EvaluatorContext {
    fn read_vnd(&mut self, var: &VarnodeData) -> Result<BitVec, EvaluatorError>;
    fn write_vnd(&mut self, var: &VarnodeData, val: &BitVec) -> Result<(), EvaluatorError>;
}

pub struct DummyContext;

impl EvaluatorContext for DummyContext {
    fn read_vnd(&mut self, var: &VarnodeData) -> Result<BitVec, EvaluatorError> {
        let spc = var.space();
        if spc.is_constant() {
            Ok(BitVec::from_u64(var.offset(), var.size() * 8))
        } else if spc.is_register() {
            todo!("read a register")
        } else if spc.is_unique() {
            todo!("read a temporary")
        } else {
            todo!("read memory")
        }
    }

    fn write_vnd(&mut self, var: &VarnodeData, val: &BitVec) -> Result<(), EvaluatorError> {
        let spc = var.space();
        if spc.is_register() {
            todo!("write a register: {val}")
        } else if spc.is_unique() {
            todo!("write a temporary: {val}")
        } else if spc.is_default() {
            todo!("write memory: {val}")
        } else {
            panic!("cannot write to constant Varnode")
        }
    }
}

pub struct Evaluator<'a, 'b, C>
where
    C: EvaluatorContext,
{
    context: &'b mut C,
    default_space: &'a AddressSpace,
    translator: &'a Translator,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvaluatorTarget {
    Branch(Location),
    Fall,
}

fn bv2addr(bv: BitVec) -> Result<Address, EvaluatorError> {
    bv.to_u64()
        .map(Address::from)
        .ok_or_else(|| EvaluatorError::Address(bv))
}

impl<'a, 'b, C> Evaluator<'a, 'b, C>
where
    C: EvaluatorContext,
{
    pub fn new(lifter: &'a Lifter, context: &'b mut C) -> Self {
        let translator = lifter.translator();
        let spaces = translator.manager();
        Self {
            context,
            default_space: spaces.default_space_ref(),
            translator,
        }
    }

    pub fn step(&mut self, operation: &PCodeData) -> Result<EvaluatorTarget, EvaluatorError> {
        match operation.opcode {
            Opcode::Copy => {
                let val = self.context.read_vnd(&operation.inputs[0])?;
                self.assign(operation.output.as_ref().unwrap(), val)?;
            }
            Opcode::Load => {
                let dst = operation.output.as_ref().unwrap();
                let src = &operation.inputs[1];
                let lsz = dst.size();

                let loc = self.read_addr(src)?;
                let val = self.read_mem(loc, lsz)?;

                self.assign(dst, val)?;
            }
            Opcode::Store => {
                let dst = &operation.inputs[1];
                let src = &operation.inputs[2];

                let val = self.context.read_vnd(&src)?;
                let loc = self.read_addr(dst)?;

                self.write_mem(loc, &val)?;
            }
            op => return Err(EvaluatorError::Unsupported(op)),
        }

        Ok(EvaluatorTarget::Fall)
    }

    fn read_addr(&mut self, var: &VarnodeData) -> Result<Address, EvaluatorError> {
        bv2addr(self.context.read_vnd(var)?)
    }

    fn read_mem(&mut self, addr: Address, sz: usize) -> Result<BitVec, EvaluatorError> {
        let mem = VarnodeData::new(self.default_space, addr.offset(), sz);
        self.context.read_vnd(&mem)
    }

    fn write_mem(&mut self, addr: Address, val: &BitVec) -> Result<(), EvaluatorError> {
        let mem = VarnodeData::new(self.default_space, addr.offset(), val.bits() / 8);
        self.context.write_vnd(&mem, val)
    }

    fn assign(&mut self, var: &VarnodeData, val: BitVec) -> Result<(), EvaluatorError> {
        self.context.write_vnd(var, &val.cast(var.size() * 8))
    }
}
