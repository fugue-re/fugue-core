use std::mem;

use arrayvec::ArrayVec;

use crate::utils::input::{ParserInput, INVALID_HANDLE};

use super::calculate_mask;

pub const MAX_LABELS: usize = 192;
pub const MAX_INPUTS_SPILL: usize = 8;
pub const MAX_DELAY_CTXTS: usize = 8;

const INVALID_LABEL: i16 = -1;

pub struct PCodeBuilderContext {
    pub inputs: Inputs,
    pub inputs_count: u8,
    pub inputs_spill: ArrayVec<Inputs, MAX_INPUTS_SPILL>,
    pub label_base: u8,
    pub label_count: u8,
    pub labels: [i16; MAX_LABELS],
    pub label_refs: ArrayVec<RelativeRecord, MAX_LABELS>,
    pub issued: Vec<PCodeOp>,
    pub unique_mask: u64, // this is constant from the translator
}

pub struct PCodeBuilder<'a> {
    pub context: &'a mut PCodeBuilderContext,
    pub input: &'a mut ParserInput,
    pub delay_slots: &'a mut [ParserInput],
    pub unique_offset: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelativeRecord {
    pub operation: u8,
    pub index: u8,
}

impl PCodeBuilderContext {
    pub fn new(unique_mask: u64) -> Self {
        Self {
            inputs: Default::default(),
            inputs_count: 0,
            inputs_spill: Default::default(),
            label_base: 0,
            label_count: 0,
            labels: [INVALID_LABEL; MAX_LABELS],
            label_refs: Default::default(),
            issued: Default::default(),
            unique_mask,
        }
    }

    pub fn emit(&mut self) -> Vec<PCodeOp> {
        self.resolve_relatives();
        mem::take(&mut self.issued)
    }

    fn resolve_relatives(&mut self) -> Option<()> {
        for rel in self.label_refs.iter() {
            // we need to recalculate the operation number since we emit args
            // spilled as Op::Arg(_) when we have > 2 inputs.

            let op_index = rel.operation + (rel.index >> 2);
            let in_index = rel.index & 1;

            let varnode = &mut self.issued[op_index as usize].inputs.0[in_index as usize];

            let label_index = varnode.offset as usize;
            let label = *self.labels.get(label_index)?;

            if label == INVALID_LABEL {
                return None;
            } else {
                let label = label as u64;
                let fixed = label.wrapping_sub(rel.operation as u64)
                    & calculate_mask(varnode.size as usize);
                varnode.offset = fixed;
            }
        }

        Some(())
    }

    #[inline]
    pub fn push_input(&mut self, vnd: Varnode) {
        if self.inputs_count < 2 {
            self.inputs.set_input(self.inputs_count as _, vnd);
        } else if self.inputs_count % 2 == 0 {
            self.inputs_spill.push(Inputs::one(vnd));
        } else {
            self.inputs_spill.last_mut().unwrap().set_input(1, vnd);
        }
        self.inputs_count += 1;
    }

    #[inline]
    pub fn issue(&mut self, op: Op, output: Varnode) {
        let pcode = PCodeOp {
            op,
            inputs: mem::take(&mut self.inputs),
            output,
        };

        self.issued.reserve(1 + self.inputs_spill.len());
        self.issued.push(pcode);
        self.issued
            .extend(self.inputs_spill.drain(..).map(|inputs| PCodeOp {
                op: Op::Arg(if inputs.is_full() { 2 } else { 1 }),
                output: Varnode::INVALID,
                inputs,
            }));

        self.inputs_count = 0;
    }

    #[inline]
    pub fn issue_with(&mut self, op: Op, output: Varnode, inputs: Inputs) {
        self.issued.push(PCodeOp { op, inputs, output });
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Varnode {
    pub space: u8,
    pub offset: u64,
    pub size: u8,
}

impl Varnode {
    pub const INVALID: Varnode = Varnode::new(INVALID_HANDLE, 0, 0);

    #[inline]
    pub const fn new(space: u8, offset: u64, size: u8) -> Self {
        Self {
            space,
            offset,
            size,
        }
    }

    #[inline]
    pub const fn constant(value: u64, size: u8) -> Self {
        Self::new(0, value, size)
    }

    #[inline]
    pub const fn is_constant(&self) -> bool {
        self.space == 0
    }

    #[inline]
    pub const fn is_invalid(&self) -> bool {
        self.space == INVALID_HANDLE
    }
}

impl Default for Varnode {
    fn default() -> Self {
        Self::INVALID
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Inputs([Varnode; 2]);

impl Default for Inputs {
    fn default() -> Self {
        Self([Varnode::INVALID, Varnode::INVALID])
    }
}

impl Inputs {
    pub fn one(vnd: Varnode) -> Self {
        Self([vnd, Varnode::INVALID])
    }

    pub fn two(vnd1: Varnode, vnd2: Varnode) -> Self {
        Self([vnd1, vnd2])
    }

    pub fn set_input(&mut self, index: usize, vnd: Varnode) {
        self.0[index] = vnd;
    }

    pub fn is_full(&self) -> bool {
        self.0[1].is_invalid()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Op {
    Copy,
    Load(u8),
    Store(u8),

    IntAdd,
    IntSub,
    IntXor,
    IntOr,
    IntAnd,
    IntMul,
    IntDiv,
    IntSignedDiv,
    IntRem,
    IntSignedRem,

    IntLeftShift,
    IntRightShift,
    IntSignedRightShift,

    IntEq,
    IntNotEq,
    IntLess,
    IntSignedLess,
    IntLessEq,
    IntSignedLessEq,
    IntCarry,
    IntSignedCarry,
    IntSignedBorrow,

    IntNot,
    IntNeg,

    CountOnes,
    CountZeros,

    ZeroExt,
    SignExt,

    IntToFloat,

    BoolAnd,
    BoolOr,
    BoolXor,
    BoolNot,

    FloatAdd,
    FloatSub,
    FloatMul,
    FloatDiv,

    FloatNeg,
    FloatAbs,
    FloatSqrt,
    FloatCeiling,
    FloatFloor,
    FloatRound,
    FloatTruncate,
    FloatIsNaN,

    FloatEq,
    FloatNotEq,
    FloatLess,
    FloatLessEq,

    FloatToInt,
    FloatToFloat,

    Branch,
    CBranch,
    IBranch,

    Call,
    ICall,

    Return,

    Subpiece,

    Arg(u16),

    // This is an index into the lifter structure; we have an entry for each
    // at compile time.
    UserOp(u16),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PCodeOp {
    pub op: Op,
    pub inputs: Inputs,
    pub output: Varnode,
}
