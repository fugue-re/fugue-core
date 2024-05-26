use fugue_bv::BitVec;
use fugue_ir::disassembly::{Opcode, PCodeData};
use fugue_ir::{Address, AddressSpace, Translator, VarnodeData};

use thiserror::Error;

use crate::ir::Location;
use crate::lifter::Lifter;

pub mod dummy;

pub mod fixed_state;
use self::fixed_state::FixedState;

#[derive(Debug, Error)]
pub enum EvaluatorError {
    #[error("invalid address: {0:x}")]
    Address(BitVec),
    #[error("division by zero")]
    DivideByZero,
    #[error("{0}")]
    Lift(fugue_ir::error::Error),
    #[error("{0}")]
    State(anyhow::Error),
    #[error("unsupported opcode: {0:?}")]
    Unsupported(Opcode),
}

impl EvaluatorError {

    /// used to generate a generic State EvaluatorError
    pub fn state<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::State(anyhow::Error::new(e))
    }

    /// used to generate a generic State EvaluatorError with custom message
    pub fn state_with<M>(msg: M) -> Self
    where
        M: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static,
    {
        Self::State(anyhow::Error::msg(msg))
    }
}

/// Provided to the Evaluator struct to implement reads and writes to varnodes
/// 
/// Varnodes don't have explicitly associated memory, so a Context struct is required
/// to implement concrete reads and writes. See DummyContext for a simple example of how 
/// this might be done
pub trait EvaluatorContext {
    /// read as "read varnode"
    /// given a varnode, returns a bitvec containing the varnode's associated data
    fn read_vnd(&mut self, var: &VarnodeData) -> Result<BitVec, EvaluatorError>;

    /// read as "write varnode"
    /// given a varnode and bitvec value to write, stores the bitvec data for later retrieval
    fn write_vnd(&mut self, var: &VarnodeData, val: &BitVec) -> Result<(), EvaluatorError>;
}

pub struct Evaluator<'a, 'b, C>
where
    C: EvaluatorContext,
{
    default_space: &'a AddressSpace,
    #[allow(unused)]
    translator: &'a Translator,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvaluatorTarget {
    Branch(Location),
    Call(Location),
    Fall,
    Return(Location),
}

fn bv2addr(bv: BitVec) -> Result<Address, EvaluatorError> {
    bv.to_u64()
        .map(Address::from)
        .ok_or_else(|| EvaluatorError::Address(bv))
}

fn bool2bv(val: bool) -> BitVec {
    BitVec::from(if val { 1u8 } else { 0u8 })
}

impl<'a> Evaluator<'a> {
    pub fn new(translator: &'a Translator) -> Self {
        let spaces = translator.manager();
        Self {
            default_space: spaces.default_space_ref(),
            translator,
        }
    }

    pub fn step(
        &mut self,
        loc: impl Into<Location>,
        operation: &PCodeData,
        context: &mut impl EvaluatorContext,
    ) -> Result<EvaluatorTarget, EvaluatorError> {
        let loc = loc.into();
        match operation.opcode {
            Opcode::Copy => {
                let val = context.read_vnd(&operation.inputs[0])?;
                self.assign(operation.output.as_ref().unwrap(), val, context)?;
            }
            Opcode::Load => {
                let dst = operation.output.as_ref().unwrap();
                let src = &operation.inputs[1];
                let lsz = dst.size();

                let loc = self.read_addr(src, context)?;
                let val = self.read_mem(loc, lsz, context)?;

                self.assign(dst, val, context)?;
            }
            Opcode::Store => {
                let dst = &operation.inputs[1];
                let src = &operation.inputs[2];

                let val = context.read_vnd(&src)?;
                let loc = self.read_addr(dst, context)?;

                self.write_mem(loc, &val, context)?;
            }
            Opcode::IntAdd => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs + rhs), context)?;
            }
            Opcode::IntSub => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs - rhs), context)?;
            }
            Opcode::IntMul => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs * rhs), context)?;
            }
            Opcode::IntDiv => {
                self.lift_unsigned_int2(operation, |lhs, rhs| {
                    if rhs.is_zero() {
                        Err(EvaluatorError::DivideByZero)
                    } else {
                        Ok(lhs / rhs)
                    }
                }, context)?;
            }
            Opcode::IntSDiv => {
                self.lift_signed_int2(operation, |lhs, rhs| {
                    if rhs.is_zero() {
                        Err(EvaluatorError::DivideByZero)
                    } else {
                        Ok(lhs / rhs)
                    }
                }, context)?;
            }
            Opcode::IntRem => {
                self.lift_unsigned_int2(operation, |lhs, rhs| {
                    if rhs.is_zero() {
                        Err(EvaluatorError::DivideByZero)
                    } else {
                        Ok(lhs % rhs)
                    }
                }, context)?;
            }
            Opcode::IntSRem => {
                self.lift_signed_int2(operation, |lhs, rhs| {
                    if rhs.is_zero() {
                        Err(EvaluatorError::DivideByZero)
                    } else {
                        Ok(lhs % rhs)
                    }
                }, context)?;
            }
            Opcode::IntLShift => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs << rhs), context)?;
            }
            Opcode::IntRShift => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs >> rhs), context)?;
            }
            Opcode::IntSRShift => {
                self.lift_signed_int2(operation, |lhs, rhs| Ok(lhs >> rhs), context)?;
            }
            Opcode::IntAnd => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs & rhs), context)?;
            }
            Opcode::IntOr => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs | rhs), context)?;
            }
            Opcode::IntXor => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(lhs ^ rhs), context)?;
            }
            Opcode::IntCarry => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(bool2bv(lhs.carry(&rhs))), context)?;
            }
            Opcode::IntSCarry => {
                self.lift_signed_int2(operation, |lhs, rhs| Ok(bool2bv(lhs.signed_carry(&rhs))), context)?;
            }
            Opcode::IntSBorrow => {
                self.lift_signed_int2(operation, |lhs, rhs| Ok(bool2bv(lhs.signed_borrow(&rhs))), context)?;
            }
            Opcode::IntEq => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(bool2bv(lhs == rhs)), context)?;
            }
            Opcode::IntNotEq => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(bool2bv(lhs != rhs)), context)?;
            }
            Opcode::IntLess => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(bool2bv(lhs < rhs)), context)?;
            }
            Opcode::IntSLess => {
                self.lift_signed_int2(operation, |lhs, rhs| Ok(bool2bv(lhs < rhs)), context)?;
            }
            Opcode::IntLessEq => {
                self.lift_unsigned_int2(operation, |lhs, rhs| Ok(bool2bv(lhs <= rhs)), context)?;
            }
            Opcode::IntSLessEq => {
                self.lift_signed_int2(operation, |lhs, rhs| Ok(bool2bv(lhs <= rhs)), context)?;
            }
            Opcode::IntSExt => {
                self.lift_signed_int1(operation, |val| Ok(val), context)?;
            }
            Opcode::IntZExt => {
                self.lift_unsigned_int1(operation, |val| Ok(val), context)?;
            }
            Opcode::IntNeg => {
                self.lift_signed_int1(operation, |val| Ok(-val), context)?;
            }
            Opcode::IntNot => {
                self.lift_unsigned_int1(operation, |val| Ok(!val), context)?;
            }
            Opcode::BoolNot => {
                self.lift_bool1(operation, |val| Ok(!val), context)?;
            }
            Opcode::BoolAnd => {
                self.lift_bool2(operation, |lhs, rhs| Ok(lhs & rhs), context)?;
            }
            Opcode::BoolOr => {
                self.lift_bool2(operation, |lhs, rhs| Ok(lhs | rhs), context)?;
            }
            Opcode::BoolXor => {
                self.lift_bool2(operation, |lhs, rhs| Ok(lhs ^ rhs), context)?;
            }
            Opcode::LZCount => self.lift_unsigned_int1(operation, |val| {
                Ok(BitVec::from_u32(val.leading_zeros(), val.bits()))
            }, context)?,
            Opcode::PopCount => self.lift_unsigned_int1(operation, |val| {
                Ok(BitVec::from_u32(val.count_ones(), val.bits()))
            }, context)?,
            Opcode::Subpiece => self.subpiece(operation, context)?,
            Opcode::Branch => {
                let locn =
                    Location::absolute_from(loc.address(), operation.inputs[0], loc.position());
                return Ok(EvaluatorTarget::Branch(locn));
            }
            Opcode::CBranch => {
                if self.read_bool(&operation.inputs[1], context)? {
                    let locn =
                        Location::absolute_from(loc.address(), operation.inputs[0], loc.position());
                    return Ok(EvaluatorTarget::Branch(locn));
                }
            }
            Opcode::IBranch => {
                let addr = self.read_addr(&operation.inputs[0], context)?;
                return Ok(EvaluatorTarget::Branch(addr.into()));
            }
            Opcode::Call => {
                let locn =
                    Location::absolute_from(loc.address(), operation.inputs[0], loc.position());
                return Ok(EvaluatorTarget::Call(locn));
            }
            Opcode::ICall => {
                let addr = self.read_addr(&operation.inputs[0], context)?;
                return Ok(EvaluatorTarget::Call(addr.into()));
            }
            Opcode::Return => {
                let addr = self.read_addr(&operation.inputs[0], context)?;
                return Ok(EvaluatorTarget::Return(addr.into()));
            }
            op => return Err(EvaluatorError::Unsupported(op)),
        }

        Ok(EvaluatorTarget::Fall)
    }

    fn subpiece(&mut self, operation: &PCodeData, context: &mut impl EvaluatorContext) -> Result<(), EvaluatorError> {
        let src = context.read_vnd(&operation.inputs[0])?;
        let src_size = src.bits();

        let off = operation.inputs[1].offset() as u32 * 8;

        let dst = operation.output.as_ref().unwrap();
        let dst_size = dst.bits();

        let trun_size = src_size.saturating_sub(off);
        let trun = if dst_size > trun_size {
            // extract high + expand
            if trun_size >= src_size {
                src
            } else {
                src >> (src_size - trun_size) as u32
            }
            .unsigned()
            .cast(trun_size)
            .cast(dst_size)
        } else {
            // extract
            if off > 0 { src >> off as u32 } else { src }
                .unsigned()
                .cast(dst_size)
        };

        self.assign(dst, trun, context)
    }

    fn lift_signed_int2<F>(&mut self, operation: &PCodeData, op: F, context: &mut impl EvaluatorContext) -> Result<(), EvaluatorError>
    where
        F: FnOnce(BitVec, BitVec) -> Result<BitVec, EvaluatorError>,
    {
        self.lift_int2(operation, |val, bits| val.signed().cast(bits), op, context)
    }

    fn lift_unsigned_int2<F>(&mut self, operation: &PCodeData, op: F, context: &mut impl EvaluatorContext) -> Result<(), EvaluatorError>
    where
        F: FnOnce(BitVec, BitVec) -> Result<BitVec, EvaluatorError>,
    {
        self.lift_int2(operation, |val, bits| val.unsigned().cast(bits), op, context)
    }

    fn lift_int2<F, G>(
        &mut self,
        operation: &PCodeData,
        cast: F,
        op: G,
        context: &mut impl EvaluatorContext,
    ) -> Result<(), EvaluatorError>
    where
        F: Fn(BitVec, u32) -> BitVec,
        G: FnOnce(BitVec, BitVec) -> Result<BitVec, EvaluatorError>,
    {
        let lhs = context.read_vnd(&operation.inputs[0])?;
        let rhs = context.read_vnd(&operation.inputs[1])?;
        let dst = operation.output.as_ref().unwrap();

        let siz = lhs.bits().max(rhs.bits());
        let val = op(cast(lhs, siz), cast(rhs, siz))?;

        self.assign(dst, val.cast(dst.bits()), context)
    }

    fn lift_signed_int1<F>(&mut self, operation: &PCodeData, op: F, context: &mut impl EvaluatorContext) -> Result<(), EvaluatorError>
    where
        F: FnOnce(BitVec) -> Result<BitVec, EvaluatorError>,
    {
        self.lift_int1(operation, |val| val.signed(), op, context)
    }

    fn lift_unsigned_int1<F>(&mut self, operation: &PCodeData, op: F, context: &mut impl EvaluatorContext) -> Result<(), EvaluatorError>
    where
        F: FnOnce(BitVec) -> Result<BitVec, EvaluatorError>,
    {
        self.lift_int1(operation, |val| val.unsigned(), op, context)
    }

    fn lift_int1<F, G>(
        &mut self,
        operation: &PCodeData,
        cast: F,
        op: G,
        context: &mut impl EvaluatorContext,
    ) -> Result<(), EvaluatorError>
    where
        F: Fn(BitVec) -> BitVec,
        G: FnOnce(BitVec) -> Result<BitVec, EvaluatorError>,
    {
        let rhs = context.read_vnd(&operation.inputs[0])?;
        let dst = operation.output.as_ref().unwrap();

        let val = op(cast(rhs))?;

        self.assign(dst, val.cast(dst.bits()), context)
    }

    fn lift_bool2<F>(&mut self, operation: &PCodeData, op: F, context: &mut impl EvaluatorContext) -> Result<(), EvaluatorError>
    where
        F: FnOnce(bool, bool) -> Result<bool, EvaluatorError>,
    {
        let lhs = context.read_vnd(&operation.inputs[0])?;
        let rhs = context.read_vnd(&operation.inputs[1])?;
        let dst = operation.output.as_ref().unwrap();

        let val = bool2bv(op(!lhs.is_zero(), !rhs.is_zero())?);

        self.assign(dst, val.cast(dst.bits()), context)
    }

    fn lift_bool1<F>(&mut self, operation: &PCodeData, op: F, context: &mut impl EvaluatorContext) -> Result<(), EvaluatorError>
    where
        F: FnOnce(bool) -> Result<bool, EvaluatorError>,
    {
        let rhs = context.read_vnd(&operation.inputs[0])?;
        let dst = operation.output.as_ref().unwrap();

        let val = bool2bv(op(!rhs.is_zero())?);

        self.assign(dst, val.cast(dst.bits()), context)
    }

    fn read_bool(&mut self, var: &VarnodeData, context: &mut impl EvaluatorContext) -> Result<bool, EvaluatorError> {
        let val = context.read_vnd(var)?;
        Ok(!val.is_zero())
    }

    pub fn read_addr(&mut self, var: &VarnodeData, context: &mut impl EvaluatorContext) -> Result<Address, EvaluatorError> {
        bv2addr(context.read_vnd(var)?)
    }

    pub fn read_mem(&mut self, addr: Address, sz: usize, context: &mut impl EvaluatorContext) -> Result<BitVec, EvaluatorError> {
        let mem = VarnodeData::new(self.default_space, addr.offset(), sz);
        context.read_vnd(&mem)
    }

    pub fn write_mem(&mut self, addr: Address, val: &BitVec, context: &mut impl EvaluatorContext) -> Result<(), EvaluatorError> {
        let mem = VarnodeData::new(self.default_space, addr.offset(), val.bytes());
        context.write_vnd(&mem, val)
    }

    pub fn assign(&mut self, var: &VarnodeData, val: BitVec, context: &mut impl EvaluatorContext) -> Result<(), EvaluatorError> {
        context.write_vnd(var, &val.cast(var.bits()))
    }

    pub fn read_reg<S: AsRef<str>>(&mut self, name: S, context: &mut impl EvaluatorContext) -> Result<BitVec, EvaluatorError> {
        if let Some(reg) = self.translator.register_by_name(name) {
            context.read_vnd(&reg)
        } else {
            Err(EvaluatorError::state_with("register does not exist"))
        }
    }
}
