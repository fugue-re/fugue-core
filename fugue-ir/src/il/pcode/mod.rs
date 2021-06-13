use std::fmt;
use fnv::FnvHashMap as Map;
use smallvec::{smallvec, SmallVec};

use crate::disassembly::{Opcode, VarnodeData};
use crate::address::Address;
use crate::space::AddressSpace;
use crate::space_manager::SpaceManager;

pub mod operand;
pub use operand::Operand;

pub mod register;
pub use register::Register;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PCodeOp<'space> {
    Copy {
        source: Operand<'space>,
        destination: Operand<'space>,
    },
    Load {
        source: Operand<'space>,
        destination: Operand<'space>,
        space: &'space AddressSpace,
    },
    Store {
        source: Operand<'space>,
        destination: Operand<'space>,
        space: &'space AddressSpace,
    },

    Branch {
        destination: Operand<'space>,
    },
    CBranch {
        destination: Operand<'space>,
        condition: Operand<'space>,
    },
    IBranch {
        destination: Operand<'space>,
    },

    Call {
        destination: Operand<'space>,
    },
    ICall {
        destination: Operand<'space>,
    },
    Intrinsic {
        name: &'space str,
        operands: SmallVec<[Operand<'space>; 4]>,
        result: Option<Operand<'space>>,
    },
    Return {
        destination: Operand<'space>,
    },

    IntEq {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntNotEq {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntLess {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntLessEq {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntSLess {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntSLessEq {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntZExt {
        result: Operand<'space>,
        operand: Operand<'space>,
    },
    IntSExt {
        result: Operand<'space>,
        operand: Operand<'space>,
    },
    IntAdd {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntSub {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntCarry {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntSCarry {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntSBorrow {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntNeg {
        result: Operand<'space>,
        operand: Operand<'space>,
    },
    IntNot {
        result: Operand<'space>,
        operand: Operand<'space>,
    },
    IntXor {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntAnd {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntOr {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntLeftShift {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntRightShift {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntSRightShift {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntMul {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntDiv {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntSDiv {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntRem {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    IntSRem {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },

    BoolNot {
        result: Operand<'space>,
        operand: Operand<'space>,
    },
    BoolXor {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    BoolAnd {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    BoolOr {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },

    FloatEq {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    FloatNotEq {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    FloatLess {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    FloatLessEq {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },

    FloatIsNaN {
        result: Operand<'space>,
        operand: Operand<'space>,
    },

    FloatAdd {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    FloatDiv {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    FloatMul {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    FloatSub {
        result: Operand<'space>,
        operands: [Operand<'space>; 2],
    },
    FloatNeg {
        result: Operand<'space>,
        operand: Operand<'space>,
    },
    FloatAbs {
        result: Operand<'space>,
        operand: Operand<'space>,
    },
    FloatSqrt {
        result: Operand<'space>,
        operand: Operand<'space>,
    },

    FloatOfInt {
        result: Operand<'space>,
        operand: Operand<'space>,
    },
    FloatOfFloat {
        result: Operand<'space>,
        operand: Operand<'space>,
    },
    FloatTruncate {
        result: Operand<'space>,
        operand: Operand<'space>,
    },
    FloatCeiling {
        result: Operand<'space>,
        operand: Operand<'space>,
    },
    FloatFloor {
        result: Operand<'space>,
        operand: Operand<'space>,
    },
    FloatRound {
        result: Operand<'space>,
        operand: Operand<'space>,
    },

    Subpiece {
        result: Operand<'space>,
        operand: Operand<'space>,
        amount: Operand<'space>,
    },
    PopCount {
        result: Operand<'space>,
        operand: Operand<'space>,
    },

    Skip,
}

impl<'space> fmt::Display for PCodeOp<'space> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Copy { destination, source } => write!(f, "{} ← {}", destination, source)?,
            Self::Load { destination, source, .. } => write!(f, "{} ← *{}", destination, source)?,
            Self::Store { destination, source, .. } => write!(f, "*{} ← {}", destination, source)?,

            Self::Branch { destination } => write!(f, "goto {}", destination)?,
            Self::CBranch { destination, condition } => write!(f, "goto {} if {} == 0x1", destination, condition)?,
            Self::IBranch { destination } => write!(f, "goto [{}]", destination)?,

            Self::Call { destination } => write!(f, "call {}", destination)?,
            Self::ICall { destination } => write!(f, "call [{}]", destination)?,
            Self::Return { destination } => write!(f, "return [{}]", destination)?,

            Self::Intrinsic { name, operands, result } => {
                if let Some(result) = result {
                    write!(f, "{} ← ", result)?;
                }
                write!(f, "{}(", name.to_lowercase())?;
                if operands.len() > 0 {
                    write!(f, "{}", operands[0])?;
                    for oper in &operands[1..] {
                        write!(f, ", {}", oper)?;
                    }
                }
                write!(f, ")")?;
            },

            Self::IntEq { result, operands } => write!(f, "{} ← {} == {}", result, operands[0], operands[1])?,
            Self::IntNotEq { result, operands } => write!(f, "{} ← {} != {}", result, operands[0], operands[1])?,
            Self::IntSLess { result, operands } => write!(f, "{} ← {} s< {}", result, operands[0], operands[1])?,
            Self::IntSLessEq { result, operands } => write!(f, "{} ← {} s<= {}", result, operands[0], operands[1])?,
            Self::IntLess { result, operands } => write!(f, "{} ← {} < {}", result, operands[0], operands[1])?,
            Self::IntLessEq { result, operands } => write!(f, "{} ← {} <= {}", result, operands[0], operands[1])?,

            Self::IntZExt { result, operand } => write!(f, "{} ← zext({}, {})", result, operand, result.size() * 8)?,
            Self::IntSExt { result, operand } => write!(f, "{} ← sext({}, {})", result, operand, result.size() * 8)?,

            Self::IntAdd { result, operands } => write!(f, "{} ← {} + {}", result, operands[0], operands[1])?,
            Self::IntSub { result, operands } => write!(f, "{} ← {} - {}", result, operands[0], operands[1])?,
            Self::IntCarry { result, operands } => write!(f, "{} ← carry({}, {})", result, operands[0], operands[1])?,
            Self::IntSCarry { result, operands } => write!(f, "{} ← scarry({}, {})", result, operands[0], operands[1])?,
            Self::IntSBorrow { result, operands } => write!(f, "{} ← sborrow({}, {})", result, operands[0], operands[1])?,

            Self::IntNeg { result, operand } => write!(f, "{} ← -{}", result, operand)?,
            Self::IntNot { result, operand } => write!(f, "{} ← ~{}", result, operand)?,

            Self::IntXor { result, operands } => write!(f, "{} ← {} ^ {}", result, operands[0], operands[1])?,
            Self::IntAnd { result, operands } => write!(f, "{} ← {} & {}", result, operands[0], operands[1])?,
            Self::IntOr { result, operands } => write!(f, "{} ← {} | {}", result, operands[0], operands[1])?,
            Self::IntLeftShift { result, operands } => write!(f, "{} ← {} << {}", result, operands[0], operands[1])?,
            Self::IntRightShift { result, operands } => write!(f, "{} ← {} >> {}", result, operands[0], operands[1])?,
            Self::IntSRightShift { result, operands } => write!(f, "{} ← {} s>> {}", result, operands[0], operands[1])?,

            Self::IntMul { result, operands } => write!(f, "{} ← {} * {}", result, operands[0], operands[1])?,
            Self::IntDiv { result, operands } => write!(f, "{} ← {} / {}", result, operands[0], operands[1])?,
            Self::IntSDiv { result, operands } => write!(f, "{} ← {} s/ {}", result, operands[0], operands[1])?,
            Self::IntRem { result, operands } => write!(f, "{} ← {} % {}", result, operands[0], operands[1])?,
            Self::IntSRem { result, operands } => write!(f, "{} ← {} s% {}", result, operands[0], operands[1])?,

            Self::BoolNot { result, operand } => write!(f, "{} ← !{}", result, operand)?,
            Self::BoolXor { result, operands } => write!(f, "{} ← {} ^ {}", result, operands[0], operands[1])?,
            Self::BoolAnd { result, operands } => write!(f, "{} ← {} & {}", result, operands[0], operands[1])?,
            Self::BoolOr { result, operands } => write!(f, "{} ← {} | {}", result, operands[0], operands[1])?,

            Self::FloatEq { result, operands } => write!(f, "{} ← {} f== {}", result, operands[0], operands[1])?,
            Self::FloatNotEq { result, operands } => write!(f, "{} ← {} f!= {}", result, operands[0], operands[1])?,
            Self::FloatLess { result, operands } => write!(f, "{} ← {} f< {}", result, operands[0], operands[1])?,
            Self::FloatLessEq { result, operands } => write!(f, "{} ← {} f<= {}", result, operands[0], operands[1])?,

            Self::FloatIsNaN { result, operand } => write!(f, "{} ← nan({})", result, operand)?,

            Self::FloatAdd { result, operands } => write!(f, "{} ← {} f+ {}", result, operands[0], operands[1])?,
            Self::FloatDiv { result, operands } => write!(f, "{} ← {} f/ {}", result, operands[0], operands[1])?,
            Self::FloatMul { result, operands } => write!(f, "{} ← {} f* {}", result, operands[0], operands[1])?,
            Self::FloatSub { result, operands } => write!(f, "{} ← {} f- {}", result, operands[0], operands[1])?,

            Self::FloatNeg { result, operand } => write!(f, "{} ← f-{}", result, operand)?,
            Self::FloatAbs { result, operand } => write!(f, "{} ← abs({})", result, operand)?,
            Self::FloatSqrt { result, operand } => write!(f, "{} ← sqrt({})", result, operand)?,

            Self::FloatOfInt { result, operand } => write!(f, "{} ← float-of-int{}({})", result.size() * 8, result, operand)?,
            Self::FloatOfFloat { result, operand } => write!(f, "{} ← float-of-float{}({})", result.size() * 8, result, operand)?,
            Self::FloatTruncate { result, operand } => write!(f, "{} ← truncate({}, {})", result, operand, result.size() * 8)?,
            Self::FloatCeiling { result, operand } => write!(f, "{} ← ceiling({})", result, operand)?,
            Self::FloatFloor { result, operand } => write!(f, "{} ← floor({})", result, operand)?,
            Self::FloatRound { result, operand } => write!(f, "{} ← round({})", result, operand)?,

            Self::Subpiece { result, operand, amount } => write!(f, "{} ← subpiece({}, {})", result, operand, amount)?,
            Self::PopCount { result, operand } => write!(f, "{} ← popcount({})", result, operand)?,
            Self::Skip => write!(f, "skip")?,
        }
        Ok(())
    }
}

impl<'space> PCodeOp<'space> {
    pub fn from_parts(
        manager: &'space SpaceManager,
        registers: &'space Map<(u64, usize), &'space str>,
        user_ops: &'space [&'space str],
        opcode: Opcode,
        inputs: SmallVec<[VarnodeData<'space>; 16]>,
        output: Option<VarnodeData<'space>>,
    ) -> Self {
        let mut inputs = inputs.into_iter();
        let spaces = manager.spaces();
        match opcode {
            Opcode::Copy => PCodeOp::Copy {
                destination: Operand::from_varnodedata(manager, registers, output.unwrap()),
                source: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
            },
            Opcode::Load => {
                let space = &spaces[inputs.next().unwrap().offset() as usize];
                let destination = output.unwrap();
                let source = inputs.next().unwrap();

                PCodeOp::Load {
                    destination: Operand::from_varnodedata(manager, registers, destination),
                    source: Operand::from_varnodedata(manager, registers, source),
                    space,
                }
            },
            Opcode::Store => {
                let space = &spaces[inputs.next().unwrap().offset() as usize];
                let destination = inputs.next().unwrap();
                let source = inputs.next().unwrap();

                PCodeOp::Store {
                    destination: Operand::from_varnodedata(manager, registers, destination),
                    source: Operand::from_varnodedata(manager, registers, source),
                    space,
                }
            },
            Opcode::Branch => PCodeOp::Branch {
                destination: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
            },
            Opcode::CBranch => PCodeOp::CBranch {
                destination: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                condition: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
            },
            Opcode::IBranch => PCodeOp::IBranch {
                destination: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
            },
            Opcode::Call => PCodeOp::Call {
                destination: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
            },
            Opcode::ICall => PCodeOp::ICall {
                destination: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
            },
            Opcode::CallOther => {
                let name = user_ops[inputs.next().unwrap().offset() as usize];
                let result = output.map(|output| Operand::from_varnodedata(manager, registers, output));

                PCodeOp::Intrinsic {
                    name,
                    operands: inputs.into_iter().map(|vnd| Operand::from_varnodedata(manager, registers, vnd)).collect(),
                    result,
                }
            },
            Opcode::Return => PCodeOp::Return {
                destination: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
            },
            Opcode::Subpiece => PCodeOp::Subpiece {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                amount: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::PopCount => PCodeOp::PopCount {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::BoolNot => PCodeOp::BoolNot {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::BoolAnd => PCodeOp::BoolAnd {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::BoolOr => PCodeOp::BoolOr {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::BoolXor => PCodeOp::BoolXor {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntNeg => PCodeOp::IntNeg {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntNot => PCodeOp::IntNot {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntSExt => PCodeOp::IntSExt {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntZExt => PCodeOp::IntZExt {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntEq => PCodeOp::IntEq {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntNotEq => PCodeOp::IntNotEq {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntLess => PCodeOp::IntLess {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntLessEq => PCodeOp::IntLessEq {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntSLess => PCodeOp::IntSLess {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntSLessEq => PCodeOp::IntSLessEq {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntCarry => PCodeOp::IntCarry {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntSCarry => PCodeOp::IntSCarry {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntSBorrow => PCodeOp::IntSBorrow {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntAdd => PCodeOp::IntAdd {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntSub => PCodeOp::IntSub {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntDiv => PCodeOp::IntDiv {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntSDiv => PCodeOp::IntSDiv {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntMul => PCodeOp::IntMul {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntRem => PCodeOp::IntRem {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntSRem => PCodeOp::IntSRem {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntLShift => PCodeOp::IntLeftShift {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntRShift => PCodeOp::IntRightShift {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntSRShift => PCodeOp::IntSRightShift {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntAnd => PCodeOp::IntAnd {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntOr => PCodeOp::IntOr {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::IntXor => PCodeOp::IntXor {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatIsNaN => PCodeOp::FloatIsNaN {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatAbs => PCodeOp::FloatAbs {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatNeg => PCodeOp::FloatNeg {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatSqrt => PCodeOp::FloatSqrt {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatFloor => PCodeOp::FloatFloor {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatCeiling => PCodeOp::FloatCeiling {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatRound => PCodeOp::FloatRound {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatEq => PCodeOp::FloatEq {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatNotEq => PCodeOp::FloatNotEq {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatLess => PCodeOp::FloatLess {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatLessEq => PCodeOp::FloatLessEq {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatAdd => PCodeOp::FloatAdd {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatSub => PCodeOp::FloatSub {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatDiv => PCodeOp::FloatDiv {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatMul => PCodeOp::FloatMul {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatOfFloat => PCodeOp::FloatOfFloat {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatOfInt => PCodeOp::FloatOfInt {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::FloatTruncate => PCodeOp::FloatTruncate {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unwrap()),
            },
            Opcode::Build
            | Opcode::CrossBuild
            | Opcode::CPoolRef
            | Opcode::Piece
            | Opcode::Extract
            | Opcode::DelaySlot
            | Opcode::New
            | Opcode::Insert
            | Opcode::Cast
            | Opcode::Label
            | Opcode::SegmentOp => {
                panic!("unimplemented due to spec.")
            }
        }
    }

    pub fn skip() -> Self {
        PCodeOp::Skip
    }
}

#[derive(Debug, Clone)]
pub struct PCode<'space> {
    pub address: Address<'space>,
    pub operations: SmallVec<[PCodeOp<'space>; 16]>,
    pub delay_slots: usize,
    pub length: usize,
}

impl<'space> PCode<'space> {
    pub fn nop(address: Address<'space>, length: usize) -> Self {
        Self {
            address,
            operations: smallvec![PCodeOp::skip()],
            delay_slots: 0,
            length,
        }
    }

    pub fn address(&self) -> Address<'space> {
        self.address.clone()
    }

    pub fn operations(&self) -> &[PCodeOp<'space>] {
        self.operations.as_ref()
    }

    pub fn delay_slots(&self) -> usize {
        self.delay_slots
    }

    pub fn length(&self) -> usize {
        self.length
    }

    pub fn display<'pcode>(&'pcode self) -> PCodeFormatter<'pcode, 'space> {
        PCodeFormatter { pcode: self }
    }
}

pub struct PCodeFormatter<'pcode, 'space> {
    pcode: &'pcode PCode<'space>,
}

impl<'pcode, 'space> fmt::Display for PCodeFormatter<'pcode, 'space> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let len =  self.pcode.operations.len();
        if len > 0 {
            for (i, op) in self.pcode.operations.iter().enumerate() {
                write!(f, "{}.{:02}: {}{}", self.pcode.address, i,
                       op,
                       if i == len - 1 { "" } else { "\n" })?;
            }
            Ok(())
        } else {
            write!(f, "{}.00: skip", self.pcode.address)
        }
    }
}
