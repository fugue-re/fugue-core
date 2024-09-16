use std::mem;

use arrayvec::ArrayVec;

use crate::utils::input::{ParserInput, INVALID_HANDLE};

use super::{calculate_mask, FixedHandle};

pub const MAX_LABELS: usize = 32;
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
    pub unique_mask: u64, // this is constant from the translator
}

pub struct PCodeBuilder<'a> {
    pub context: &'a mut PCodeBuilderContext,
    pub input: &'a mut ParserInput,
    pub delay_slots: &'a mut [ParserInput],
    pub issued: &'a mut Vec<PCodeOp>,
    pub unique_offset: u64,
}

impl<'a> PCodeBuilder<'a> {
    pub fn new(
        context: &'a mut PCodeBuilderContext,
        input: &'a mut ParserInput,
        delay_slots: &'a mut [ParserInput],
        issued: &'a mut Vec<PCodeOp>,
    ) -> Self {
        input.base_state();
        Self {
            unique_offset: (input.address() & context.unique_mask) << 4,
            context,
            input,
            delay_slots,
            issued,
        }
    }

    #[inline]
    pub fn address(&self) -> u64 {
        self.input.address()
    }

    #[inline]
    pub fn next_address(&self) -> u64 {
        self.input.next_address()
    }

    #[inline]
    pub fn delay_slot_length(&self) -> usize {
        self.input.delay_slot_length()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.input.len()
    }

    #[inline]
    pub fn emit(&mut self) {
        self.input.base_state();
        self.issued.clear();

        if let Some(builder) = self.input.constructor().build_action {
            (builder)(self)
        }

        self.resolve_relatives();

        // reset
        self.context.inputs_count = 0;
        self.context.label_count = 0;
        self.context.labels.fill(INVALID_LABEL);
        self.context.label_refs.clear();
    }

    #[doc(hidden)]
    #[inline]
    pub fn emit_delay_slots(&mut self) {
        let unique_offset = self.unique_offset;

        let base_address = self.address();
        let delay_slot_bytes = self.delay_slot_length();

        let mut fall_offset = self.len();
        let mut bytes = 0usize;
        let mut index = 0usize;

        loop {
            let address = base_address + fall_offset as u64;

            self.set_unique_offset(address);

            std::mem::swap(self.input, &mut self.delay_slots[index]);

            let length = self.len();

            self.input.base_state();

            if let Some(builder) = self.input.constructor().build_action {
                (builder)(self)
            }

            std::mem::swap(self.input, &mut self.delay_slots[index]);

            fall_offset += length;
            bytes += length;

            if bytes >= delay_slot_bytes {
                break;
            }

            index += 1;
        }

        self.unique_offset = unique_offset;
    }

    #[doc(hidden)]
    #[inline]
    pub fn operand_handle(&self, index: usize) -> &FixedHandle {
        unsafe {
            let opnds = self
                .input
                .context
                .constructors
                .get_unchecked(self.input.point as usize)
                .operands as usize;

            self.input
                .context
                .constructors
                .get_unchecked(opnds + index)
                .handle
                .as_ref()
                .unwrap_unchecked()
        }
    }

    #[inline]
    #[doc(hidden)]
    pub fn unique_mask(&self) -> u64 {
        self.context.unique_mask
    }

    #[inline]
    #[doc(hidden)]
    pub fn set_unique_offset(&mut self, address: u64) {
        self.unique_offset = (address & self.context.unique_mask) << 4;
    }

    #[inline]
    #[doc(hidden)]
    pub fn push_input(&mut self, vnd: Varnode) {
        unsafe {
            if self.context.inputs_count < 2 {
                self.context
                    .inputs
                    .set_input_unchecked(self.context.inputs_count as _, vnd);
            } else if self.context.inputs_count & 1 == 0 {
                self.context.inputs_spill.push_unchecked(Inputs::one(vnd));
            } else {
                let last_posn = self.context.inputs_spill.len();
                self.context
                    .inputs_spill
                    .get_unchecked_mut(last_posn)
                    .set_input(1, vnd);
            }
            self.context.inputs_count += 1;
        }
    }

    #[inline]
    #[doc(hidden)]
    pub fn issue(&mut self, op: Op, output: Varnode) {
        let pcode = PCodeOp {
            op,
            inputs: mem::take(&mut self.context.inputs),
            output,
        };

        self.issued.reserve(1 + self.context.inputs_spill.len());
        self.issued.push(pcode);
        self.issued
            .extend(self.context.inputs_spill.drain(..).map(|inputs| PCodeOp {
                op: Op::Arg(1 + inputs.is_full() as u16),
                output: Varnode::INVALID,
                inputs,
            }));

        self.context.inputs_count = 0;
    }

    #[inline]
    #[doc(hidden)]
    pub fn issue_with(&mut self, op: Op, inputs: Inputs, output: Varnode) {
        self.issued.push(PCodeOp { op, inputs, output });
    }

    #[inline]
    fn resolve_relatives(&mut self) -> Option<()> {
        for rel in self.context.label_refs.iter() {
            // we need to recalculate the operation number since we emit args
            // spilled as Op::Arg(_) when we have > 2 inputs.

            let op_index = rel.operation + (rel.index >> 2);
            let in_index = rel.index & 1;

            let varnode = &mut self.issued[op_index as usize].inputs.0[in_index as usize];

            let label_index = varnode.offset as usize;
            let label = *self.context.labels.get(label_index)?;

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
            unique_mask,
        }
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
pub struct Inputs(pub [Varnode; 2]);

impl Default for Inputs {
    fn default() -> Self {
        Self([Varnode::INVALID, Varnode::INVALID])
    }
}

impl Inputs {
    #[inline]
    pub fn one(vnd: Varnode) -> Self {
        Self([vnd, Varnode::INVALID])
    }

    #[inline]
    pub fn two(vnd1: Varnode, vnd2: Varnode) -> Self {
        Self([vnd1, vnd2])
    }

    #[inline]
    pub fn set_input(&mut self, index: usize, vnd: Varnode) {
        self.0[index] = vnd;
    }

    #[inline]
    pub unsafe fn set_input_unchecked(&mut self, index: usize, vnd: Varnode) {
        *self.0.get_unchecked_mut(index) = vnd;
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
