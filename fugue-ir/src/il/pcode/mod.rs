use std::fmt;
use std::sync::Arc;

use crate::disassembly::{Opcode, VarnodeData};
use crate::address::AddressValue;
use crate::space::AddressSpaceId;
use crate::space_manager::SpaceManager;

use crate::register::RegisterNames;

pub mod operand;
pub use operand::Operand;

pub mod register;
pub use register::Register;

use smallvec::SmallVec;

use unsafe_unwrap::UnsafeUnwrap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum PCodeOp {
    Copy {
        source: Operand,
        destination: Operand,
    },
    Load {
        source: Operand,
        destination: Operand,
        space: AddressSpaceId,
    },
    Store {
        source: Operand,
        destination: Operand,
        space: AddressSpaceId,
    },

    Branch {
        destination: Operand,
    },
    CBranch {
        destination: Operand,
        condition: Operand,
    },
    IBranch {
        destination: Operand,
    },

    Call {
        destination: Operand,
    },
    ICall {
        destination: Operand,
    },
    Intrinsic {
        name: Arc<str>,
        operands: SmallVec<[Operand; 4]>,
        result: Option<Operand>,
    },
    Return {
        destination: Operand,
    },

    IntEq {
        result: Operand,
        operands: [Operand; 2],
    },
    IntNotEq {
        result: Operand,
        operands: [Operand; 2],
    },
    IntLess {
        result: Operand,
        operands: [Operand; 2],
    },
    IntLessEq {
        result: Operand,
        operands: [Operand; 2],
    },
    IntSLess {
        result: Operand,
        operands: [Operand; 2],
    },
    IntSLessEq {
        result: Operand,
        operands: [Operand; 2],
    },
    IntZExt {
        result: Operand,
        operand: Operand,
    },
    IntSExt {
        result: Operand,
        operand: Operand,
    },
    IntAdd {
        result: Operand,
        operands: [Operand; 2],
    },
    IntSub {
        result: Operand,
        operands: [Operand; 2],
    },
    IntCarry {
        result: Operand,
        operands: [Operand; 2],
    },
    IntSCarry {
        result: Operand,
        operands: [Operand; 2],
    },
    IntSBorrow {
        result: Operand,
        operands: [Operand; 2],
    },
    IntNeg {
        result: Operand,
        operand: Operand,
    },
    IntNot {
        result: Operand,
        operand: Operand,
    },
    IntXor {
        result: Operand,
        operands: [Operand; 2],
    },
    IntAnd {
        result: Operand,
        operands: [Operand; 2],
    },
    IntOr {
        result: Operand,
        operands: [Operand; 2],
    },
    IntLeftShift {
        result: Operand,
        operands: [Operand; 2],
    },
    IntRightShift {
        result: Operand,
        operands: [Operand; 2],
    },
    IntSRightShift {
        result: Operand,
        operands: [Operand; 2],
    },
    IntMul {
        result: Operand,
        operands: [Operand; 2],
    },
    IntDiv {
        result: Operand,
        operands: [Operand; 2],
    },
    IntSDiv {
        result: Operand,
        operands: [Operand; 2],
    },
    IntRem {
        result: Operand,
        operands: [Operand; 2],
    },
    IntSRem {
        result: Operand,
        operands: [Operand; 2],
    },

    BoolNot {
        result: Operand,
        operand: Operand,
    },
    BoolXor {
        result: Operand,
        operands: [Operand; 2],
    },
    BoolAnd {
        result: Operand,
        operands: [Operand; 2],
    },
    BoolOr {
        result: Operand,
        operands: [Operand; 2],
    },

    FloatEq {
        result: Operand,
        operands: [Operand; 2],
    },
    FloatNotEq {
        result: Operand,
        operands: [Operand; 2],
    },
    FloatLess {
        result: Operand,
        operands: [Operand; 2],
    },
    FloatLessEq {
        result: Operand,
        operands: [Operand; 2],
    },

    FloatIsNaN {
        result: Operand,
        operand: Operand,
    },

    FloatAdd {
        result: Operand,
        operands: [Operand; 2],
    },
    FloatDiv {
        result: Operand,
        operands: [Operand; 2],
    },
    FloatMul {
        result: Operand,
        operands: [Operand; 2],
    },
    FloatSub {
        result: Operand,
        operands: [Operand; 2],
    },
    FloatNeg {
        result: Operand,
        operand: Operand,
    },
    FloatAbs {
        result: Operand,
        operand: Operand,
    },
    FloatSqrt {
        result: Operand,
        operand: Operand,
    },

    FloatOfInt {
        result: Operand,
        operand: Operand,
    },
    FloatOfFloat {
        result: Operand,
        operand: Operand,
    },
    FloatTruncate {
        result: Operand,
        operand: Operand,
    },
    FloatCeiling {
        result: Operand,
        operand: Operand,
    },
    FloatFloor {
        result: Operand,
        operand: Operand,
    },
    FloatRound {
        result: Operand,
        operand: Operand,
    },

    Subpiece {
        result: Operand,
        operand: Operand,
        amount: Operand,
    },
    PopCount {
        result: Operand,
        operand: Operand,
    },

    Skip,
}

impl fmt::Display for PCodeOp {
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

impl PCodeOp {
    pub(crate) fn from_parts<I: ExactSizeIterator<Item=VarnodeData>>(
        manager: &SpaceManager,
        registers: &RegisterNames,
        user_ops: &[Arc<str>],
        opcode: Opcode,
        inputs: I,
        output: Option<VarnodeData>,
    ) -> Self {
        let mut inputs = inputs.into_iter();
        unsafe { match opcode {
            Opcode::Copy => PCodeOp::Copy {
                destination: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
                source: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
            },
            Opcode::Load => {
                let space = manager.spaces()[inputs.next().unsafe_unwrap().offset() as usize].id();
                let destination = output.unsafe_unwrap();
                let source = inputs.next().unsafe_unwrap();

                PCodeOp::Load {
                    destination: Operand::from_varnodedata(manager, registers, destination),
                    source: Operand::from_varnodedata(manager, registers, source),
                    space,
                }
            },
            Opcode::Store => {
                let space = manager.spaces()[inputs.next().unsafe_unwrap().offset() as usize].id();
                let destination = inputs.next().unsafe_unwrap();
                let source = inputs.next().unsafe_unwrap();

                PCodeOp::Store {
                    destination: Operand::from_varnodedata(manager, registers, destination),
                    source: Operand::from_varnodedata(manager, registers, source),
                    space,
                }
            },
            Opcode::Branch => PCodeOp::Branch {
                destination: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
            },
            Opcode::CBranch => PCodeOp::CBranch {
                destination: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                condition: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
            },
            Opcode::IBranch => PCodeOp::IBranch {
                destination: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
            },
            Opcode::Call => PCodeOp::Call {
                destination: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
            },
            Opcode::ICall => PCodeOp::ICall {
                destination: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
            },
            Opcode::CallOther => {
                let name = user_ops[inputs.next().unsafe_unwrap().offset() as usize].clone();
                let result = output.map(|output| Operand::from_varnodedata(manager, registers, output));

                let mut operands = SmallVec::with_capacity(inputs.len());
                operands.extend(inputs.into_iter().map(|vnd| Operand::from_varnodedata(manager, registers, vnd)));

                PCodeOp::Intrinsic {
                    name,
                    operands,
                    result,
                }
            },
            Opcode::Return => PCodeOp::Return {
                destination: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
            },
            Opcode::Subpiece => PCodeOp::Subpiece {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                amount: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::PopCount => PCodeOp::PopCount {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::BoolNot => PCodeOp::BoolNot {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::BoolAnd => PCodeOp::BoolAnd {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::BoolOr => PCodeOp::BoolOr {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::BoolXor => PCodeOp::BoolXor {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntNeg => PCodeOp::IntNeg {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntNot => PCodeOp::IntNot {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntSExt => PCodeOp::IntSExt {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntZExt => PCodeOp::IntZExt {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntEq => PCodeOp::IntEq {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntNotEq => PCodeOp::IntNotEq {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntLess => PCodeOp::IntLess {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntLessEq => PCodeOp::IntLessEq {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntSLess => PCodeOp::IntSLess {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntSLessEq => PCodeOp::IntSLessEq {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntCarry => PCodeOp::IntCarry {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntSCarry => PCodeOp::IntSCarry {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntSBorrow => PCodeOp::IntSBorrow {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntAdd => PCodeOp::IntAdd {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntSub => PCodeOp::IntSub {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntDiv => PCodeOp::IntDiv {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntSDiv => PCodeOp::IntSDiv {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntMul => PCodeOp::IntMul {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntRem => PCodeOp::IntRem {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntSRem => PCodeOp::IntSRem {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntLShift => PCodeOp::IntLeftShift {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntRShift => PCodeOp::IntRightShift {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntSRShift => PCodeOp::IntSRightShift {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntAnd => PCodeOp::IntAnd {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntOr => PCodeOp::IntOr {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::IntXor => PCodeOp::IntXor {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatIsNaN => PCodeOp::FloatIsNaN {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatAbs => PCodeOp::FloatAbs {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatNeg => PCodeOp::FloatNeg {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatSqrt => PCodeOp::FloatSqrt {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatFloor => PCodeOp::FloatFloor {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatCeiling => PCodeOp::FloatCeiling {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatRound => PCodeOp::FloatRound {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatEq => PCodeOp::FloatEq {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatNotEq => PCodeOp::FloatNotEq {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatLess => PCodeOp::FloatLess {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatLessEq => PCodeOp::FloatLessEq {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatAdd => PCodeOp::FloatAdd {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatSub => PCodeOp::FloatSub {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatDiv => PCodeOp::FloatDiv {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatMul => PCodeOp::FloatMul {
                operands: [
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                    Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                ],
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatOfFloat => PCodeOp::FloatOfFloat {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatOfInt => PCodeOp::FloatOfInt {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::FloatTruncate => PCodeOp::FloatTruncate {
                operand: Operand::from_varnodedata(manager, registers, inputs.next().unsafe_unwrap()),
                result: Operand::from_varnodedata(manager, registers, output.unsafe_unwrap()),
            },
            Opcode::Label => PCodeOp::Skip,
            Opcode::Build
            | Opcode::CrossBuild
            | Opcode::CPoolRef
            | Opcode::Piece
            | Opcode::Extract
            | Opcode::DelaySlot
            | Opcode::New
            | Opcode::Insert
            | Opcode::Cast
            | Opcode::SegmentOp => {
                panic!("{:?} unimplemented due to spec", opcode)
            }
        } }
    }

    pub fn skip() -> Self {
        PCodeOp::Skip
    }
}

#[derive(Debug, Clone)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct PCode {
    pub address: AddressValue,
    pub operations: SmallVec<[PCodeOp; 8]>,
    pub delay_slots: usize,
    pub length: usize,
}

impl PCode {
    pub fn nop(address: AddressValue, length: usize) -> Self {
        Self {
            address,
            operations: SmallVec::new(),
            delay_slots: 0,
            length,
        }
    }

    pub fn address(&self) -> AddressValue {
        self.address.clone()
    }

    pub fn operations(&self) -> &[PCodeOp] {
        self.operations.as_ref()
    }

    pub fn delay_slots(&self) -> usize {
        self.delay_slots
    }

    pub fn length(&self) -> usize {
        self.length
    }

    pub fn display<'pcode>(&'pcode self) -> PCodeFormatter<'pcode> {
        PCodeFormatter { pcode: self }
    }
}

pub struct PCodeFormatter<'pcode> {
    pcode: &'pcode PCode,
}

impl<'pcode> fmt::Display for PCodeFormatter<'pcode> {
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
