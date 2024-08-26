//! concrete evaluator
//! 
//! an evaluator for concrete execution on BitVec

use std::any::Any;

use nohash_hasher::IntMap;
use thiserror::Error;

use fugue_bv::BitVec;
use fugue_ir::disassembly::{
    Opcode, PCodeData, IRBuilderArena,
};
use fugue_ir::{Address, VarnodeData};
use fugue_core::ir::Location;

use crate::eval;
use crate::eval::traits::{ Evaluator, EvaluatorContext };
use crate::eval::traits::observer::*;
use crate::context::traits::{ VarnodeContext, MemoryMapContext };
use crate::context::concrete::ConcreteContext;

pub mod observer;

/// error types specific to concrete evaluator
/// 
/// these are made into runtime errors in eval::Error
#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("invalid address: {0:x}")]
    InvalidAddress(BitVec),
    #[error("division by zero @ {0:#x?}")]
    DivideByZero(Address),
    #[error("unsupported opcode: {0:?}")]
    Unsupported(Opcode),
}

impl Into<eval::Error> for Error {
    fn into(self) -> eval::Error {
        eval::Error::runtime(self)
    }
}

/// concrete evaluator
/// 
/// note that the evaluator's pc keeps track of the pcode-relative location
/// while context's pc tracks the actual runtime value of the pc register
/// as a BitVec
pub struct ConcreteEvaluator {
    pc: Location,
    pcode_observers: Vec<Box<dyn PCodeObserver>>,
    insn_observers: Vec<Box<dyn InsnObserver>>,
    breakpoints: IntMap<u64, Vec<Box<dyn FnMut(&mut ConcreteContext) -> Result<(), eval::Error> + 'static>>>,
}

/// helper function to convert BitVec to Address
fn bv2addr(bv: BitVec) -> Result<Address, Error> {
    bv.to_u64()
        .map(Address::from)
        .ok_or_else(|| Error::InvalidAddress(bv))
}

/// helper function to convert boolean to bitvector
fn bool2bv(val: bool) -> BitVec {
    BitVec::from(if val { 1u8 } else { 0u8 })
}

impl ConcreteEvaluator {

    pub fn new() -> Self {
        Self {
            pc: Location::default(),
            pcode_observers: Vec::new(),
            insn_observers: Vec::new(),
            breakpoints: IntMap::default(),
        }
    }

    /// register pcode observer
    pub fn register_observer(
        &mut self,
        observer: Observer,
    ) -> Result<(), eval::Error> {
        match observer {
            Observer::PCode(obs) => {
                self.pcode_observers.push(obs);
            },
            Observer::Insn(obs) => {
                self.insn_observers.push(obs);
            },
            _ => { },
        }
        Ok(())
    }

    /// register breakpoint with optional callback
    pub fn register_breakpoint(
        &mut self,
        breakpoint: &Address,
        callback: impl FnMut(&mut ConcreteContext) -> Result<(), eval::Error> + 'static,
    ) -> Result<(), eval::Error> {
        if !self.breakpoints.contains_key(&breakpoint.offset()) {
            self.breakpoints.insert(breakpoint.offset(), vec![Box::new(callback)]);
        } else {
            self.breakpoints.get_mut(&breakpoint.offset()).unwrap()
                .push(Box::new(callback))
        }
        Ok(())
    }

    /// get shared reference to pc
    pub fn pc(&self) -> &Location {
        &self.pc
    }

    /// set pc
    pub fn set_pc(&mut self, address: Address) {
        self.pc.address = address;
    }
}

impl<'irb> Evaluator<'irb> for ConcreteEvaluator {
    type Data = BitVec;
    type Context = ConcreteContext<'irb>;

    /// evaluates a single pcode operation
    fn evaluate(&self,
        operation: &PCodeData,
        context: &mut Self::Context,
    ) -> Result<eval::Target, eval::Error> {
        let loc = self.pc.clone();
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
                        Err(Error::DivideByZero(loc.address()).into())
                    } else {
                        Ok(lhs / rhs)
                    }
                }, context)?;
            }
            Opcode::IntSDiv => {
                self.lift_signed_int2(operation, |lhs, rhs| {
                    if rhs.is_zero() {
                        Err(Error::DivideByZero(loc.address()).into())
                    } else {
                        Ok(lhs / rhs)
                    }
                }, context)?;
            }
            Opcode::IntRem => {
                self.lift_unsigned_int2(operation, |lhs, rhs| {
                    if rhs.is_zero() {
                        Err(Error::DivideByZero(loc.address()).into())
                    } else {
                        Ok(lhs % rhs)
                    }
                }, context)?;
            }
            Opcode::IntSRem => {
                self.lift_signed_int2(operation, |lhs, rhs| {
                    if rhs.is_zero() {
                        Err(Error::DivideByZero(loc.address()).into())
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
                return Ok(eval::Target::Branch(locn));
            }
            Opcode::CBranch => {
                if self.read_bool(&operation.inputs[1], context)? {
                    let locn =
                        Location::absolute_from(loc.address(), operation.inputs[0], loc.position());
                    return Ok(eval::Target::Branch(locn));
                }
            }
            Opcode::IBranch => {
                let addr = self.read_addr(&operation.inputs[0], context)?;
                return Ok(eval::Target::Branch(addr.into()));
            }
            Opcode::Call => {
                let locn =
                    Location::absolute_from(loc.address(), operation.inputs[0], loc.position());
                return Ok(eval::Target::Call(locn));
            }
            Opcode::ICall => {
                let addr = self.read_addr(&operation.inputs[0], context)?;
                return Ok(eval::Target::Call(addr.into()));
            }
            Opcode::Return => {
                let addr = self.read_addr(&operation.inputs[0], context)?;
                return Ok(eval::Target::Return(addr.into()));
            }
            op => {
                println!("{}", operation.display(context.language().translator()));
                return Err(Error::Unsupported(op).into())
            }
        }

        Ok(eval::Target::Fall)
    }

    fn step(
        &mut self,
        irb: &'irb IRBuilderArena,
        context: &mut Self::Context,
    ) -> Result<(), eval::Error> {
        // let mut pc = Location::from(bv2addr(context.get_pc()?).into()?);

        let addr = self.pc.address();

        if let Some(callbacks) = self.breakpoints.get_mut(&addr.offset()) {
            for cb in callbacks.iter_mut() {
                cb(context)?;
            }
        }

        // try to fetch. if not in translation cache, lift new block.
        let mut fetch_result = context.fetch(addr);
        if let Err(eval::Error::TranslationCache(_)) = fetch_result {
            #[allow(unused)]
            let tb = context.lift_block(addr, irb);
            // todo: add block observer update here
            // note: because we are checking if the _instruction_ is in the translation
            // cache and not whether the block at the current address has been lifted,
            // this means that blocks that are contained within blocks that have been
            // already lifted will not be lifted again, and the observer will not be
            // updated for them. to change this behavior, need to refactor to check
            // against lifted blocks, not lifted instructions. (create a TranslationCache
            // instead of just using a RwLock<IntMap> in context)
            fetch_result = context.fetch(addr);
        }

        let pcode = fetch_result?;

        // call insn observers
        if self.insn_observers.len() > 0 {
            let insn_bytes = context.read_bytes(&addr, pcode.len()).unwrap();
            for observer in self.insn_observers.iter_mut() {
                observer.update(&addr, insn_bytes)?;
            }
        }

        let op_count = pcode.operations.len() as u32;
        let mut target = eval::Target::Fall;
        while addr == self.pc.address() && self.pc.position() < op_count {
            let pos = self.pc.position() as usize;
            let op = &pcode.operations()[pos];
            target = self.evaluate(op, context)?;

            // call pcode observers
            if self.pcode_observers.len() > 0 {
                let out: Option<BitVec> = op.output
                    .map(|vnd| context.read_vnd(&vnd).unwrap());
                let ins: Vec<BitVec> = op.inputs.iter()
                    .map(|vnd| context.read_vnd(vnd).unwrap())
                    .collect();
                for observer in self.pcode_observers.iter_mut() {
                    observer.update(op, &ins, &out)?;
                }
            }

            match target {
                eval::Target::Branch(loc) |
                eval::Target::Call(loc) |
                eval::Target::Return(loc) => {
                    self.pc = loc;
                },
                eval::Target::Fall => {
                    self.pc.position += 1u32;
                },
            }
        }

        // set pc to new location
        // (only needs to be done for fall, since would've been done
        // implicitly in the while loop for branches)
        // todo: update edge observer here (give reference to target?)
        match target {
            eval::Target::Fall => {
                self.pc = Location::from(addr + pcode.len());
            },
            _ => { }
        }

        // write-back pc?
        let pc_val = BitVec::from(self.pc.address().offset());
        context.set_pc(&pc_val)?;

        Ok(())
    }
}

impl ConcreteEvaluator {

    fn subpiece(
        &self,
        operation: &PCodeData,
        context: &mut ConcreteContext,
    ) -> Result<(), eval::Error> {
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

    fn lift_signed_int2<F>(
        &self,
        operation: &PCodeData,
        op: F,
        context: &mut ConcreteContext,
    ) -> Result<(), eval::Error>
    where
        F: FnOnce(BitVec, BitVec) -> Result<BitVec, eval::Error>,
    {
        self.lift_int2(operation, |val, bits| val.signed().cast(bits), op, context)
    }

    fn lift_unsigned_int2<F>(
        &self,
        operation: &PCodeData,
        op: F,
        context: &mut ConcreteContext,
    ) -> Result<(), eval::Error>
    where
        F: FnOnce(BitVec, BitVec) -> Result<BitVec, eval::Error>,
    {
        self.lift_int2(operation, |val, bits| val.unsigned().cast(bits), op, context)
    }

    fn lift_int2<F, G>(
        &self,
        operation: &PCodeData,
        cast: F,
        op: G,
        context: &mut ConcreteContext,
    ) -> Result<(), eval::Error>
    where
        F: Fn(BitVec, u32) -> BitVec,
        G: FnOnce(BitVec, BitVec) -> Result<BitVec, eval::Error>,
    {
        let lhs = context.read_vnd(&operation.inputs[0])?;
        let rhs = context.read_vnd(&operation.inputs[1])?;
        let dst = operation.output.as_ref().unwrap();

        let siz = lhs.bits().max(rhs.bits());
        let val = op(cast(lhs, siz), cast(rhs, siz))?;

        self.assign(dst, val.cast(dst.bits()), context)
    }

    fn lift_signed_int1<F>(
        &self,
        operation: &PCodeData,
        op: F,
        context: &mut ConcreteContext,
    ) -> Result<(), eval::Error>
    where
        F: FnOnce(BitVec) -> Result<BitVec, eval::Error>,
    {
        self.lift_int1(operation, |val| val.signed(), op, context)
    }

    fn lift_unsigned_int1<F>(
        &self,
        operation: &PCodeData,
        op: F,
        context: &mut ConcreteContext,
    ) -> Result<(), eval::Error>
    where
        F: FnOnce(BitVec) -> Result<BitVec, eval::Error>,
    {
        self.lift_int1(operation, |val| val.unsigned(), op, context)
    }

    fn lift_int1<F, G>(
        &self,
        operation: &PCodeData,
        cast: F,
        op: G,
        context: &mut ConcreteContext,
    ) -> Result<(), eval::Error>
    where
        F: Fn(BitVec) -> BitVec,
        G: FnOnce(BitVec) -> Result<BitVec, eval::Error>,
    {
        let rhs = context.read_vnd(&operation.inputs[0])?;
        let dst = operation.output.as_ref().unwrap();

        let val = op(cast(rhs))?;

        self.assign(dst, val.cast(dst.bits()), context)
    }

    fn lift_bool2<F>(
        &self,
        operation: &PCodeData,
        op: F,
        context: &mut ConcreteContext,
    ) -> Result<(), eval::Error>
    where
        F: FnOnce(bool, bool) -> Result<bool, eval::Error>,
    {
        let lhs = context.read_vnd(&operation.inputs[0])?;
        let rhs = context.read_vnd(&operation.inputs[1])?;
        let dst = operation.output.as_ref().unwrap();

        let val = bool2bv(op(!lhs.is_zero(), !rhs.is_zero())?);

        self.assign(dst, val.cast(dst.bits()), context)
    }

    fn lift_bool1<F>(
        &self,
        operation: &PCodeData,
        op: F,
        context: &mut ConcreteContext,
    ) -> Result<(), eval::Error>
    where
        F: FnOnce(bool) -> Result<bool, eval::Error>,
    {
        let rhs = context.read_vnd(&operation.inputs[0])?;
        let dst = operation.output.as_ref().unwrap();

        let val = bool2bv(op(!rhs.is_zero())?);

        self.assign(dst, val.cast(dst.bits()), context)
    }

    fn read_bool(&self, var: &VarnodeData, context: &mut ConcreteContext) -> Result<bool, eval::Error> {
        let val = context.read_vnd(var)?;
        Ok(!val.is_zero())
    }

    fn read_addr(&self, var: &VarnodeData, context: &mut ConcreteContext) -> Result<Address, eval::Error> {
        bv2addr(context.read_vnd(var)?)
            .map_err(Error::into)
    }

    fn read_mem(&self, addr: Address, sz: usize, context: &mut ConcreteContext) -> Result<BitVec, eval::Error> {
        let mem = VarnodeData::new(context.default_space(), addr.offset(), sz);
        context.read_vnd(&mem)
            .map_err(eval::Error::from)
    }

    fn write_mem(&self, addr: Address, val: &BitVec, context: &mut ConcreteContext) -> Result<(), eval::Error> {
        let mem = VarnodeData::new(context.default_space(), addr.offset(), val.bytes());
        context.write_vnd(&mem, val)
            .map_err(eval::Error::from)
    }

    fn assign(&self, var: &VarnodeData, val: BitVec, context: &mut ConcreteContext) -> Result<(), eval::Error> {
        context.write_vnd(var, &val.cast(var.bits()))
            .map_err(eval::Error::from)
    }
}


#[cfg(test)]
mod test {
    use fugue_core::language::LanguageBuilder;
    use fugue_bytes::Endian;
    use crate::context::traits::*;
    use crate::tests::TEST_PROGRAM;
    use super::*;
    use super::observer::{ PCodeStdoutLogger, InsnStdoutLogger };

    #[test]
    fn test_evaluator() {
        // set up context
        let lang_builder = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");
        let lifter = lang.lifter();
        let irb = lifter.irb(1024);
        let mut context = ConcreteContext::new_with(&lang);

        assert_eq!(
            context.language().convention().name(),
            "default"
        );
        assert_eq!(
            context.language().translator().architecture(),
            &fugue_arch::ArchitectureDef::new("ARM", Endian::Little, 32usize, "Cortex")
        );

        let mem_base = Address::from(0x0u64);
        let aligned_size = 0x2000usize;

        context.map_mem(mem_base, aligned_size)
            .expect("map_mem() failed:");

        // load test program into memory
        context.write_bytes(mem_base, TEST_PROGRAM)
            .expect("write_bytes() failed to write program into memory");

        // initialize registers
        context.set_pc(&BitVec::from_u32(0x0u32, 64))
            .expect("failed to set pc");
        context.set_sp(&BitVec::from_u32(aligned_size as u32, 64))
            .expect("failed to set sp");

        // initialize evaluator
        let mut evaluator = ConcreteEvaluator::new();
        let insn_t = lifter.translator();
        let pcode_t = lifter.translator();
        let insn_logger = InsnStdoutLogger::new_with(insn_t);
        let pcode_logger = PCodeStdoutLogger::new_with(pcode_t);
        evaluator.register_observer(Observer::Insn(Box::new(insn_logger)))
            .expect("failed to register insn observer");
        evaluator.register_observer(Observer::PCode(Box::new(pcode_logger)))
            .expect("failed to register pcode observer");

        // debugging the CMP instruction issue
        evaluator.register_breakpoint(&Address::from(0x28u64), |context| {
            let ng_val = context.read_reg("NG")
                .map_err(eval::Error::from)?;
            println!("NG val: {:?}", ng_val);
            Ok(())
        }).expect("failed to register breakpoint");


        let halt_address = Address::from(0x4u64);
        let mut cycles = 0;
        while evaluator.pc.address() != halt_address {
            println!("pc: {:#x}", evaluator.pc.address().offset());
            if evaluator.pc.address.offset() == 0x26u64 {
                println!("breakpoint");
            }
            evaluator.step(&irb, &mut context)
                .expect("step failed:");
            cycles += 1;
        }

        // should've executed a bunch of instructions
        assert!(cycles > 10, "instructions executed: {}", cycles);

        let retval = context.read_reg("r0")
            .expect("failed to read register r0");

        assert_eq!(retval.to_i32().unwrap(), 6561, "retval: {:?}, cycles: {}", retval, cycles);
    }
}