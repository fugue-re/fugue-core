use crate::runtime::constructor::ConstructorResolver;
use crate::runtime::input::{FixedHandle, INVALID_HANDLE};
use crate::runtime::{wrap_offset, LiftingContextState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Op {
    Copy,
    Load,
    Store,
    Branch,
    CBranch,
    IBranch,
    Call,
    ICall,
    CallOther,
    Return,
    IntEq,
    IntNotEq,
    IntSLess,
    IntSLessEq,
    IntLess,
    IntLessEq,
    IntZExt,
    IntSExt,
    IntNeg,
    IntNot,
    IntAdd,
    IntSub,
    IntMul,
    IntDiv,
    IntSDiv,
    IntRem,
    IntSRem,
    IntCarry,
    IntSCarry,
    IntSBorrow,
    IntAnd,
    IntOr,
    IntXor,
    IntLShift,
    IntRShift,
    IntSRShift,
    BoolNot,
    BoolAnd,
    BoolOr,
    BoolXor,
    FloatEq,
    FloatNotEq,
    FloatLess,
    FloatLessEq,
    FloatIsNaN,
    FloatAdd,
    FloatSub,
    FloatMul,
    FloatDiv,
    FloatNeg,
    FloatAbs,
    FloatSqrt,
    FloatOfInt,
    FloatOfFloat,
    FloatTruncate,
    FloatCeiling,
    FloatFloor,
    FloatRound,
    Build,
    DelaySlot,
    Piece,
    Subpiece,
    Cast,
    Label,
    CrossBuild,
    SegmentOp,
    CPoolRef,
    New,
    Insert,
    Extract,
    PopCount,
    LZCount,
}

pub struct ConstructTpl {
    pub delay_slot: usize,
    pub labels: u8,
    pub result: Option<HandleTpl>,
    pub operations: &'static [OpTpl],
}

impl ConstructTpl {
    #[inline]
    pub fn build<R: ConstructorResolver>(&self, input: &mut LiftingContextState<'_>) -> Option<()> {
        todo!()
    }
}

pub enum HandleKind {
    Space,
    Offset,
    Size,
    OffsetPlus(u64),
}

pub enum ConstTpl {
    Real(u64),
    Handle(usize, HandleKind),
    Start,
    Next,
    Next2,
    CurrentSpace,
    CurrentSpaceSize,
    SpaceId(u8),
    Relative(u64),
}

impl ConstTpl {
    #[inline]
    pub fn update_offset<R: ConstructorResolver>(
        &self,
        input: &mut LiftingContextState<'_>,
        handle: &mut FixedHandle,
    ) -> Option<()> {
        match self {
            Self::Handle(index, _) => {
                let h = unsafe { input.operand_handle_mut(*index) };

                handle.offset_space = h.offset_space;
                handle.offset_offset = h.offset_offset;
                handle.offset_size = h.offset_size;
                handle.temporary_space = h.temporary_space;
                handle.temporary_offset = h.temporary_offset;
            }
            _ => {
                let value = self.value::<R>(input)?;

                handle.offset_space = INVALID_HANDLE;
                handle.offset_offset = wrap_offset(R::resolve_upper_bound(handle.space), value);
            }
        }
        Some(())
    }

    #[inline]
    pub fn space<R: ConstructorResolver>(&self, input: &mut LiftingContextState<'_>) -> u8 {
        match self {
            Self::CurrentSpace => R::DEFAULT_SPACE,
            Self::Handle(index, HandleKind::Space) => {
                let handle = input.input().operand_handle(*index);
                if handle.offset_space == INVALID_HANDLE {
                    handle.space
                } else {
                    handle.temporary_space
                }
            }
            Self::SpaceId(id) => *id,
            _ => unreachable!("state should be unreachable via generated code"),
        }
    }

    #[inline]
    pub fn value<R: ConstructorResolver>(
        &self,
        input: &mut LiftingContextState<'_>,
    ) -> Option<u64> {
        let value = match self {
            ConstTpl::Start => input.address(),
            ConstTpl::Next => input.next_address(),
            ConstTpl::Next2 => {
                if let Some(next2_address) = input.next2_address() {
                    next2_address
                } else {
                    let mut ninput = input.next_input()?;
                    R::resolve(&mut ninput)?;
                    ninput.next_address()
                }
            }
            ConstTpl::CurrentSpaceSize => R::ADDRESS_SIZE,
            ConstTpl::CurrentSpace => R::DEFAULT_SPACE as u64,
            ConstTpl::Relative(value) | ConstTpl::Real(value) => *value,
            ConstTpl::SpaceId(space) => *space as u64,
            ConstTpl::Handle(index, kind) => {
                let handle = input.input().operand_handle(*index);
                match kind {
                    HandleKind::Space => {
                        if handle.offset_space == INVALID_HANDLE {
                            handle.space as u64
                        } else {
                            handle.temporary_space as u64
                        }
                    }
                    HandleKind::Offset => {
                        if handle.offset_space == INVALID_HANDLE {
                            handle.offset_offset
                        } else {
                            handle.temporary_offset
                        }
                    }
                    HandleKind::Size => handle.size as u64,
                    HandleKind::OffsetPlus(value) => {
                        let value = *value;
                        let value_short = value & 0xffff;
                        let value_shift = 8 * (value >> 16) as u32;

                        if handle.space == 0 {
                            // constant space
                            let val = if handle.offset_space == INVALID_HANDLE {
                                handle.offset_offset
                            } else {
                                handle.temporary_offset
                            };
                            val.checked_shr(value_shift).unwrap_or(0)
                        } else {
                            if handle.offset_space == INVALID_HANDLE {
                                handle.offset_offset + value_short
                            } else {
                                handle.temporary_offset + value_short
                            }
                        }
                    }
                }
            }
        };
        Some(value)
    }
}

pub struct OpTpl {
    pub op: Op,
    pub inputs: &'static [VarnodeTpl],
    pub output: Option<VarnodeTpl>,
}

pub struct HandleTpl {
    pub space: ConstTpl,
    pub size: ConstTpl,
    pub ptr_space: ConstTpl,
    pub ptr_offset: ConstTpl,
    pub ptr_size: ConstTpl,
    pub tmp_space: ConstTpl,
    pub tmp_offset: ConstTpl,
}

pub struct VarnodeTpl {
    pub space: ConstTpl,
    pub offset: ConstTpl,
    pub size: ConstTpl,
}
