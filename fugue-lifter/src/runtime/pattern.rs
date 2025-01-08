use crate::runtime::constructor::{Constructor, ConstructorResolver};
use crate::runtime::input::{BREADCRUMBS, INVALID_HANDLE};
use crate::runtime::pcode::LiftingContextState;
use crate::runtime::{byte_swap, sign_extend, zero_extend};

#[derive(Copy, Clone)]
pub enum OperandOffset {
    Relative(u8),
    Operand(u8),
}

#[derive(Clone)]
pub enum PatternExpression {
    TokenField {
        big_endian: bool,
        sign_bit: bool,
        bit_start: usize,
        bit_end: usize,
        byte_start: usize,
        byte_end: usize,
        shift: u32,
    },
    ContextField {
        sign_bit: bool,
        bit_start: usize,
        bit_end: usize,
        byte_start: usize,
        byte_end: usize,
        shift: u32,
    },
    Constant {
        value: i64,
    },
    Operand {
        constructor: &'static Constructor,
        offset: OperandOffset,
        value: &'static Self,
    },
    StartInstruction,
    EndInstruction,
    Next2Instruction,
    Plus(&'static Self, &'static Self),
    Sub(&'static Self, &'static Self),
    Mult(&'static Self, &'static Self),
    LeftShift(&'static Self, &'static Self),
    RightShift(&'static Self, &'static Self),
    And(&'static Self, &'static Self),
    Or(&'static Self, &'static Self),
    Xor(&'static Self, &'static Self),
    Div(&'static Self, &'static Self),
    Minus(&'static Self),
    Not(&'static Self),
}

impl PatternExpression {
    #[inline]
    pub fn resolve<R: ConstructorResolver>(
        &self,
        input: &mut LiftingContextState<'_>,
    ) -> Option<i64> {
        match self {
            Self::Constant { value } => Some(*value),
            Self::StartInstruction => Some(input.address() as i64),
            Self::EndInstruction => Some(input.next_address() as i64),
            Self::Next2Instruction => input.next2_address().map_or_else(
                || {
                    let mut ninput = input.next_input()?;
                    R::resolve(&mut ninput)?;
                    Some(ninput.next_address() as i64)
                },
                |v| Some(v as i64),
            ),
            Self::And(lhs, rhs) => {
                let lhs = lhs.resolve::<R>(input)?;
                let rhs = rhs.resolve::<R>(input)?;
                Some(lhs & rhs)
            }
            Self::Or(lhs, rhs) => {
                let lhs = lhs.resolve::<R>(input)?;
                let rhs = rhs.resolve::<R>(input)?;
                Some(lhs | rhs)
            }
            Self::Xor(lhs, rhs) => {
                let lhs = lhs.resolve::<R>(input)?;
                let rhs = rhs.resolve::<R>(input)?;
                Some(lhs ^ rhs)
            }
            Self::Plus(lhs, rhs) => {
                let lhs = lhs.resolve::<R>(input)?;
                let rhs = rhs.resolve::<R>(input)?;
                Some(lhs.wrapping_add(rhs))
            }
            Self::Sub(lhs, rhs) => {
                let lhs = lhs.resolve::<R>(input)?;
                let rhs = rhs.resolve::<R>(input)?;
                Some(lhs.wrapping_sub(rhs))
            }
            Self::Div(lhs, rhs) => {
                let lhs = lhs.resolve::<R>(input)?;
                let rhs = rhs.resolve::<R>(input)?;
                (rhs == 0).then(|| lhs.wrapping_div(rhs))
            }
            Self::Mult(lhs, rhs) => {
                let lhs = lhs.resolve::<R>(input)?;
                let rhs = rhs.resolve::<R>(input)?;
                Some(lhs.wrapping_mul(rhs))
            }
            Self::LeftShift(lhs, rhs) => {
                let lhs = lhs.resolve::<R>(input)?;
                let rhs = rhs.resolve::<R>(input)?;
                Some(lhs.checked_shl(rhs as u8 as u32).unwrap_or(0))
            }
            Self::RightShift(lhs, rhs) => {
                let lhs = lhs.resolve::<R>(input)?;
                let rhs = rhs.resolve::<R>(input)?;
                Some(
                    lhs.checked_shr(rhs as u8 as u32)
                        .unwrap_or(if lhs < 0 { -1 } else { 0 }),
                )
            }
            Self::Not(rhs) => {
                let rhs = rhs.resolve::<R>(input)?;
                Some(!rhs)
            }
            Self::Minus(rhs) => {
                let rhs = rhs.resolve::<R>(input)?;
                Some(-rhs)
            }
            Self::TokenField {
                big_endian,
                sign_bit,
                bit_start,
                bit_end,
                byte_start,
                byte_end,
                shift,
            } => {
                let size = byte_end - byte_start + 1;
                let mut res = 0i64;
                let mut start = *byte_start as isize;
                let mut tsize = size as isize;

                while tsize >= size_of::<u32>() as isize {
                    let tmp = input
                        .input()
                        .instruction_bytes(start as usize, size_of::<u32>())?;
                    res = res.checked_shl(8 * size_of::<u32>() as u32).unwrap_or(0);
                    res = (res as u64 | tmp as u64) as i64;
                    start += size_of::<u32>() as isize;
                    tsize = (*byte_end as isize) - start + 1;
                }
                if tsize > 0 {
                    let tmp = input
                        .input()
                        .instruction_bytes(start as usize, tsize as usize)?;
                    res = res.checked_shl(8 * tsize as u32).unwrap_or(0);
                    res = (res as u64 | tmp as u64) as i64;
                }

                res = if !big_endian {
                    byte_swap(res, size)
                } else {
                    res
                };
                res = res
                    .checked_shr(*shift)
                    .unwrap_or(if res < 0 { -1 } else { 0 });

                Some(if *sign_bit {
                    sign_extend(res, bit_end - bit_start)
                } else {
                    zero_extend(res, bit_end - bit_start)
                })
            }
            Self::ContextField {
                sign_bit,
                bit_start,
                bit_end,
                byte_start,
                byte_end,
                shift,
            } => {
                let mut res = 0i64;
                let mut size = (*byte_end as isize) - (*byte_start as isize) + 1;
                let mut start = *byte_start as isize;

                while size >= size_of::<u32>() as isize {
                    let tmp = input
                        .input()
                        .context_bytes(start as usize, size_of::<u32>());
                    res = res.checked_shl(8 * size_of::<u32>() as u32).unwrap_or(0);
                    res = (res as u64 | tmp as u64) as i64;
                    start += size_of::<u32>() as isize;
                    size = (*byte_end as isize) - start + 1;
                }
                if size > 0 {
                    let tmp = input.input().context_bytes(start as usize, size as usize);
                    res = res.checked_shl(8 * size as u32).unwrap_or(0);
                    res = (res as u64 | tmp as u64) as i64;
                }

                res = res
                    .checked_shr(*shift)
                    .unwrap_or(if res < 0 { -1 } else { 0 });

                Some(if *sign_bit {
                    sign_extend(res, bit_end - bit_start)
                } else {
                    zero_extend(res, bit_end - bit_start)
                })
            }
            Self::Operand {
                constructor,
                offset,
                value,
            } => {
                let mut cur_depth = input.inputs.input.depth;
                let mut point =
                    &input.inputs.input.context.constructors[input.inputs.input.point as usize];

                let ctor_id = constructor.id;

                while point.constructor.map(|ctor| ctor.id) != Some(ctor_id) {
                    if cur_depth <= 0 {
                        let old_point = input.inputs.input.point;
                        let old_depth = std::mem::take(&mut input.inputs.input.depth);
                        let old_breadcrumb = std::mem::replace(
                            &mut input.inputs.input.breadcrumb,
                            [0u8; BREADCRUMBS],
                        );

                        input.inputs.input.point = input.inputs.input.context.alloc;
                        {
                            let cstate = &mut input.inputs.input.context.constructors
                                [input.inputs.input.point as usize];

                            cstate.constructor = Some(*constructor);
                            cstate.handle = None;
                            cstate.parent = INVALID_HANDLE;
                            cstate.operands = INVALID_HANDLE;
                            cstate.offset = 0;
                            cstate.length = 0;
                        }

                        // compute the value in the modified context
                        let value = value.resolve::<R>(input)?;

                        // restore old state
                        {
                            let cstate = &mut input.inputs.input.context.constructors
                                [input.inputs.input.point as usize];

                            cstate.constructor = None;
                            cstate.handle = None;
                            cstate.parent = INVALID_HANDLE;
                            cstate.operands = INVALID_HANDLE;
                            cstate.offset = 0;
                            cstate.length = 0;
                        }

                        input.inputs.input.point = old_point;
                        input.inputs.input.depth = old_depth;
                        input.inputs.input.breadcrumb = old_breadcrumb;

                        return Some(value);
                    }

                    cur_depth -= 1;
                    point = &input.inputs.input.context.constructors[point.parent as usize];
                }

                // if we reach here, we've resolved the ctor in the current tree
                let offset = match offset {
                    OperandOffset::Relative(offset) => point.offset + *offset,
                    OperandOffset::Operand(index) => {
                        input.inputs.input.context.constructors
                            [point.operands as usize + *index as usize]
                            .offset
                    }
                };
                let length = point.length;

                // preserve old and init new state
                let old_point = input.inputs.input.point;
                let old_depth = std::mem::take(&mut input.inputs.input.depth);
                let old_breadcrumb =
                    std::mem::replace(&mut input.inputs.input.breadcrumb, [0u8; BREADCRUMBS]);

                input.inputs.input.point = input.inputs.input.context.alloc;
                {
                    let cstate = &mut input.inputs.input.context.constructors
                        [input.inputs.input.point as usize];

                    cstate.constructor = Some(*&constructor);
                    cstate.handle = None;
                    cstate.parent = INVALID_HANDLE;
                    cstate.operands = INVALID_HANDLE;
                    cstate.offset = offset;
                    cstate.length = length;
                }

                // compute the value in the modified context
                let value = value.resolve::<R>(input)?;

                // restore old state
                {
                    let cstate = &mut input.inputs.input.context.constructors
                        [input.inputs.input.point as usize];

                    cstate.constructor = None;
                    cstate.handle = None;
                    cstate.parent = INVALID_HANDLE;
                    cstate.operands = INVALID_HANDLE;
                    cstate.offset = 0;
                    cstate.length = 0;
                }

                input.inputs.input.point = old_point;
                input.inputs.input.depth = old_depth;
                input.inputs.input.breadcrumb = old_breadcrumb;

                Some(value)
            }
        }
    }
}
