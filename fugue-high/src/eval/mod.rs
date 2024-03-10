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
    #[error("division by zero")]
    DivideByZero,
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
    #[allow(unused)]
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

fn bool2bv(val: bool) -> BitVec {
    BitVec::from(if val { 1u8 } else { 0u8 })
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
            Opcode::IntAdd => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs + rhs))?;
            }
            Opcode::IntSub => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs - rhs))?;
            }
            Opcode::IntMul => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs * rhs))?;
            }
            Opcode::IntDiv => {
                self.lift_unsigned_int2(operation, |lhs, rhs| {
                    if rhs.is_zero() {
                        Err(EvaluatorError::DivideByZero)
                    } else {
                        Ok(lhs / rhs)
                    }
                })?;
            }
            Opcode::IntSDiv => {
                self.lift_signed_int2(operation, |lhs, rhs| {
                    if rhs.is_zero() {
                        Err(EvaluatorError::DivideByZero)
                    } else {
                        Ok(lhs / rhs)
                    }
                })?;
            }
            Opcode::IntRem => {
                self.lift_unsigned_int2(operation, |lhs, rhs| {
                    if rhs.is_zero() {
                        Err(EvaluatorError::DivideByZero)
                    } else {
                        Ok(lhs % rhs)
                    }
                })?;
            }
            Opcode::IntSRem => {
                self.lift_signed_int2(operation, |lhs, rhs| {
                    if rhs.is_zero() {
                        Err(EvaluatorError::DivideByZero)
                    } else {
                        Ok(lhs % rhs)
                    }
                })?;
            }
            Opcode::IntLShift => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs << rhs))?;
            }
            Opcode::IntRShift => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs >> rhs))?;
            }
            Opcode::IntSRShift => {
                self.lift_signed_int2(operation, |lhs, rhs| Ok(lhs >> rhs))?;
            }
            Opcode::IntAnd => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs & rhs))?;
            }
            Opcode::IntOr => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs | rhs))?;
            }
            Opcode::IntXor => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs ^ rhs))?;
            }
            Opcode::IntCarry => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(bool2bv(lhs.carry(&rhs))))?;
            }
            Opcode::IntSCarry => {
                self.lift_signed_int2(operation, |lhs, rhs| Ok(bool2bv(lhs.signed_carry(&rhs))))?;
            }
            Opcode::IntSBorrow => {
                self.lift_signed_int2(operation, |lhs, rhs| Ok(bool2bv(lhs.signed_borrow(&rhs))))?;
            }
            Opcode::IntEq => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(bool2bv(lhs == rhs)))?;
            }
            Opcode::IntNotEq => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(bool2bv(lhs != rhs)))?;
            }
            Opcode::IntLess => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(bool2bv(lhs < rhs)))?;
            }
            Opcode::IntSLess => {
                self.lift_signed_int2(operation, |lhs, rhs| Ok(bool2bv(lhs < rhs)))?;
            }
            Opcode::IntLessEq => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(bool2bv(lhs <= rhs)))?;
            }
            Opcode::IntSLessEq => {
                self.lift_signed_int2(operation, |lhs, rhs| Ok(bool2bv(lhs <= rhs)))?;
            }
            Opcode::IntSExt => {
                self.lift_signed_int1(operation, |val| Ok(val))?;
            }
            Opcode::IntZExt => {
                self.lift_unsigned_int1(operation, |val| Ok(val))?;
            }
            Opcode::IntNeg => {
                self.lift_signed_int1(operation, |val| Ok(-val))?;
            }
            Opcode::IntNot => {
                self.lift_unsigned_int1(operation, |val| Ok(!val))?;
            }
            Opcode::BoolNot => {
                self.lift_bool1(operation, |val| Ok(!val))?;
            }
            Opcode::BoolAnd => {
                self.lift_bool2(operation, |lhs, rhs| Ok(lhs & rhs))?;
            }
            Opcode::BoolOr => {
                self.lift_bool2(operation, |lhs, rhs| Ok(lhs | rhs))?;
            }
            Opcode::BoolXor => {
                self.lift_bool2(operation, |lhs, rhs| Ok(lhs ^ rhs))?;
            }
            Opcode::LZCount => self.lift_unsigned_int1(operation, |val| {
                Ok(BitVec::from_u32(val.leading_zeros(), val.bits()))
            })?,
            Opcode::PopCount => self.lift_unsigned_int1(operation, |val| {
                Ok(BitVec::from_u32(val.count_ones(), val.bits()))
            })?,
            op => return Err(EvaluatorError::Unsupported(op)),
        }

        Ok(EvaluatorTarget::Fall)
    }

    fn lift_signed_int2<F>(&mut self, operation: &PCodeData, op: F) -> Result<(), EvaluatorError>
    where
        F: FnOnce(BitVec, BitVec) -> Result<BitVec, EvaluatorError>,
    {
        self.lift_int2(operation, |val, bits| val.signed().cast(bits), op)
    }

    fn lift_unsigned_int2<F>(&mut self, operation: &PCodeData, op: F) -> Result<(), EvaluatorError>
    where
        F: FnOnce(BitVec, BitVec) -> Result<BitVec, EvaluatorError>,
    {
        self.lift_int2(operation, |val, bits| val.unsigned().cast(bits), op)
    }

    fn lift_int2<F, G>(
        &mut self,
        operation: &PCodeData,
        cast: F,
        op: G,
    ) -> Result<(), EvaluatorError>
    where
        F: Fn(BitVec, usize) -> BitVec,
        G: FnOnce(BitVec, BitVec) -> Result<BitVec, EvaluatorError>,
    {
        let lhs = self.context.read_vnd(&operation.inputs[0])?;
        let rhs = self.context.read_vnd(&operation.inputs[1])?;
        let dst = operation.output.as_ref().unwrap();

        let siz = lhs.bits().max(rhs.bits());
        let val = op(cast(lhs, siz), cast(rhs, siz))?;

        self.assign(dst, val.cast(dst.size() * 8))
    }

    fn lift_signed_int1<F>(&mut self, operation: &PCodeData, op: F) -> Result<(), EvaluatorError>
    where
        F: FnOnce(BitVec) -> Result<BitVec, EvaluatorError>,
    {
        self.lift_int1(operation, |val| val.signed(), op)
    }

    fn lift_unsigned_int1<F>(&mut self, operation: &PCodeData, op: F) -> Result<(), EvaluatorError>
    where
        F: FnOnce(BitVec) -> Result<BitVec, EvaluatorError>,
    {
        self.lift_int1(operation, |val| val.unsigned(), op)
    }

    fn lift_int1<F, G>(
        &mut self,
        operation: &PCodeData,
        cast: F,
        op: G,
    ) -> Result<(), EvaluatorError>
    where
        F: Fn(BitVec) -> BitVec,
        G: FnOnce(BitVec) -> Result<BitVec, EvaluatorError>,
    {
        let rhs = self.context.read_vnd(&operation.inputs[0])?;
        let dst = operation.output.as_ref().unwrap();

        let val = op(cast(rhs))?;

        self.assign(dst, val.cast(dst.size() * 8))
    }

    fn lift_bool2<F>(&mut self, operation: &PCodeData, op: F) -> Result<(), EvaluatorError>
    where
        F: FnOnce(bool, bool) -> Result<bool, EvaluatorError>,
    {
        let lhs = self.context.read_vnd(&operation.inputs[0])?;
        let rhs = self.context.read_vnd(&operation.inputs[1])?;
        let dst = operation.output.as_ref().unwrap();

        let val = bool2bv(op(!lhs.is_zero(), !rhs.is_zero())?);

        self.assign(dst, val.cast(dst.size() * 8))
    }

    fn lift_bool1<F>(&mut self, operation: &PCodeData, op: F) -> Result<(), EvaluatorError>
    where
        F: FnOnce(bool) -> Result<bool, EvaluatorError>,
    {
        let rhs = self.context.read_vnd(&operation.inputs[0])?;
        let dst = operation.output.as_ref().unwrap();

        let val = bool2bv(op(!rhs.is_zero())?);

        self.assign(dst, val.cast(dst.size() * 8))
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
