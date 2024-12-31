use fugue_ir::disassembly::Opcode;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use thiserror::Error;

use crate::runtime::pcode::Op;

#[derive(Debug, Error)]
#[error("{0:?} cannot be directly represented as an Op")]
pub struct NotUnrepresentable(Opcode);

impl TryFrom<Opcode> for Op {
    type Error = NotUnrepresentable;

    fn try_from(value: Opcode) -> Result<Self, Self::Error> {
        Ok(match value {
            Opcode::Copy => Op::Copy,
            Opcode::Branch => Op::Branch,
            Opcode::CBranch => Op::CBranch,
            Opcode::IBranch => Op::IBranch,
            Opcode::Call => Op::Call,
            Opcode::ICall => Op::ICall,
            Opcode::Return => Op::Return,
            Opcode::IntEq => Op::IntEq,
            Opcode::IntNotEq => Op::IntNotEq,
            Opcode::IntSLess => Op::IntSignedLess,
            Opcode::IntSLessEq => Op::IntSignedLessEq,
            Opcode::IntLess => Op::IntLess,
            Opcode::IntLessEq => Op::IntLessEq,
            Opcode::IntZExt => Op::ZeroExt,
            Opcode::IntSExt => Op::SignExt,
            Opcode::IntNeg => Op::IntNeg,
            Opcode::IntNot => Op::IntNot,
            Opcode::IntAdd => Op::IntAdd,
            Opcode::IntSub => Op::IntSub,
            Opcode::IntMul => Op::IntMul,
            Opcode::IntDiv => Op::IntDiv,
            Opcode::IntSDiv => Op::IntSignedDiv,
            Opcode::IntRem => Op::IntRem,
            Opcode::IntSRem => Op::IntSignedRem,
            Opcode::IntCarry => Op::IntCarry,
            Opcode::IntSCarry => Op::IntSignedCarry,
            Opcode::IntSBorrow => Op::IntSignedBorrow,
            Opcode::IntAnd => Op::IntAnd,
            Opcode::IntOr => Op::IntOr,
            Opcode::IntXor => Op::IntXor,
            Opcode::IntLShift => Op::IntLeftShift,
            Opcode::IntRShift => Op::IntRightShift,
            Opcode::IntSRShift => Op::IntSignedRightShift,
            Opcode::BoolNot => Op::BoolNot,
            Opcode::BoolAnd => Op::BoolAnd,
            Opcode::BoolOr => Op::BoolOr,
            Opcode::BoolXor => Op::BoolXor,
            Opcode::FloatEq => Op::FloatEq,
            Opcode::FloatNotEq => Op::FloatNotEq,
            Opcode::FloatLess => Op::FloatLess,
            Opcode::FloatLessEq => Op::FloatLessEq,
            Opcode::FloatIsNaN => Op::FloatIsNaN,
            Opcode::FloatAdd => Op::FloatAdd,
            Opcode::FloatSub => Op::FloatSub,
            Opcode::FloatMul => Op::FloatMul,
            Opcode::FloatDiv => Op::FloatDiv,
            Opcode::FloatNeg => Op::FloatNeg,
            Opcode::FloatAbs => Op::FloatAbs,
            Opcode::FloatSqrt => Op::FloatSqrt,
            Opcode::FloatOfInt => Op::IntToFloat,
            Opcode::FloatOfFloat => Op::FloatToFloat,
            Opcode::FloatTruncate => Op::FloatTruncate,
            Opcode::FloatCeiling => Op::FloatCeiling,
            Opcode::FloatFloor => Op::FloatFloor,
            Opcode::FloatRound => Op::FloatRound,
            Opcode::Subpiece => Op::Subpiece,
            Opcode::PopCount => Op::CountOnes,
            Opcode::LZCount => Op::CountZeros,
            op => {
                return Err(NotUnrepresentable(op));
            }
        })
    }
}

impl ToTokens for Op {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let value = match self {
            Self::Copy => quote! { fugue_lifter::runtime::pcode::Op::Copy },
            Self::Load(v) => quote! { fugue_lifter::runtime::pcode::Op::Load(#v) },
            Self::Store(v) => quote! { fugue_lifter::runtime::pcode::Op::Store(#v) },
            Self::IntAdd => quote! { fugue_lifter::runtime::pcode::Op::IntAdd },
            Self::IntSub => quote! { fugue_lifter::runtime::pcode::Op::IntSub },
            Self::IntXor => quote! { fugue_lifter::runtime::pcode::Op::IntXor },
            Self::IntOr => quote! { fugue_lifter::runtime::pcode::Op::IntOr },
            Self::IntAnd => quote! { fugue_lifter::runtime::pcode::Op::IntAnd },
            Self::IntMul => quote! { fugue_lifter::runtime::pcode::Op::IntMul },
            Self::IntDiv => quote! { fugue_lifter::runtime::pcode::Op::IntDiv },
            Self::IntSignedDiv => quote! { fugue_lifter::runtime::pcode::Op::IntSignedDiv },
            Self::IntRem => quote! { fugue_lifter::runtime::pcode::Op::IntRem },
            Self::IntSignedRem => quote! { fugue_lifter::runtime::pcode::Op::IntSignedRem },
            Self::IntLeftShift => quote! { fugue_lifter::runtime::pcode::Op::IntLeftShift },
            Self::IntRightShift => quote! { fugue_lifter::runtime::pcode::Op::IntRightShift },
            Self::IntSignedRightShift => {
                quote! { fugue_lifter::runtime::pcode::Op::IntSignedRightShift }
            }
            Self::IntEq => quote! { fugue_lifter::runtime::pcode::Op::IntEq },
            Self::IntNotEq => quote! { fugue_lifter::runtime::pcode::Op::IntNotEq },
            Self::IntLess => quote! { fugue_lifter::runtime::pcode::Op::IntLess },
            Self::IntSignedLess => quote! { fugue_lifter::runtime::pcode::Op::IntSignedLess },
            Self::IntLessEq => quote! { fugue_lifter::runtime::pcode::Op::IntLessEq },
            Self::IntSignedLessEq => quote! { fugue_lifter::runtime::pcode::Op::IntSignedLessEq },
            Self::IntCarry => quote! { fugue_lifter::runtime::pcode::Op::IntCarry },
            Self::IntSignedCarry => quote! { fugue_lifter::runtime::pcode::Op::IntSignedCarry },
            Self::IntSignedBorrow => quote! { fugue_lifter::runtime::pcode::Op::IntSignedBorrow },
            Self::IntNot => quote! { fugue_lifter::runtime::pcode::Op::IntNot },
            Self::IntNeg => quote! { fugue_lifter::runtime::pcode::Op::IntNeg },
            Self::CountOnes => quote! { fugue_lifter::runtime::pcode::Op::CountOnes },
            Self::CountZeros => quote! { fugue_lifter::runtime::pcode::Op::CountZeros },
            Self::ZeroExt => quote! { fugue_lifter::runtime::pcode::Op::ZeroExt },
            Self::SignExt => quote! { fugue_lifter::runtime::pcode::Op::SignExt },
            Self::IntToFloat => quote! { fugue_lifter::runtime::pcode::Op::IntToFloat },
            Self::BoolAnd => quote! { fugue_lifter::runtime::pcode::Op::BoolAnd },
            Self::BoolOr => quote! { fugue_lifter::runtime::pcode::Op::BoolOr },
            Self::BoolXor => quote! { fugue_lifter::runtime::pcode::Op::BoolXor },
            Self::BoolNot => quote! { fugue_lifter::runtime::pcode::Op::BoolNot },
            Self::FloatAdd => quote! { fugue_lifter::runtime::pcode::Op::FloatAdd },
            Self::FloatSub => quote! { fugue_lifter::runtime::pcode::Op::FloatSub },
            Self::FloatMul => quote! { fugue_lifter::runtime::pcode::Op::FloatMul },
            Self::FloatDiv => quote! { fugue_lifter::runtime::pcode::Op::FloatDiv },
            Self::FloatNeg => quote! { fugue_lifter::runtime::pcode::Op::FloatNeg },
            Self::FloatAbs => quote! { fugue_lifter::runtime::pcode::Op::FloatAbs },
            Self::FloatSqrt => quote! { fugue_lifter::runtime::pcode::Op::FloatSqrt },
            Self::FloatCeiling => quote! { fugue_lifter::runtime::pcode::Op::FloatCeiling },
            Self::FloatFloor => quote! { fugue_lifter::runtime::pcode::Op::FloatFloor },
            Self::FloatRound => quote! { fugue_lifter::runtime::pcode::Op::FloatRound },
            Self::FloatTruncate => quote! { fugue_lifter::runtime::pcode::Op::FloatTruncate },
            Self::FloatIsNaN => quote! { fugue_lifter::runtime::pcode::Op::FloatIsNaN },
            Self::FloatEq => quote! { fugue_lifter::runtime::pcode::Op::FloatEq },
            Self::FloatNotEq => quote! { fugue_lifter::runtime::pcode::Op::FloatNotEq },
            Self::FloatLess => quote! { fugue_lifter::runtime::pcode::Op::FloatLess },
            Self::FloatLessEq => quote! { fugue_lifter::runtime::pcode::Op::FloatLessEq },
            Self::FloatToInt => quote! { fugue_lifter::runtime::pcode::Op::FloatToInt },
            Self::FloatToFloat => quote! { fugue_lifter::runtime::pcode::Op::FloatToFloat },
            Self::Branch => quote! { fugue_lifter::runtime::pcode::Op::Branch },
            Self::CBranch => quote! { fugue_lifter::runtime::pcode::Op::CBranch },
            Self::IBranch => quote! { fugue_lifter::runtime::pcode::Op::IBranch },
            Self::Call => quote! { fugue_lifter::runtime::pcode::Op::Call },
            Self::ICall => quote! { fugue_lifter::runtime::pcode::Op::ICall },
            Self::Return => quote! { fugue_lifter::runtime::pcode::Op::Return },
            Self::Subpiece => quote! { fugue_lifter::runtime::pcode::Op::Subpiece },
            Self::Arg(v) => quote! { fugue_lifter::runtime::pcode::Op::Arg(#v) },
            Self::UserOp(v) => quote! { fugue_lifter::runtime::pcode::Op::UserOp(#v) },
        };
        value.to_tokens(tokens);
    }
}
