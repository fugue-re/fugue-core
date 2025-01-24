use std::fmt;

use crate::runtime::constructor::ConstructorResolver;
use crate::runtime::input::FixedHandle;
use crate::runtime::pattern::PatternExpression;
use crate::runtime::pcode::LiftingContextState;

#[derive(Clone)]
pub enum Symbol {
    Epsilon,
    Value {
        pattern_value: PatternExpression,
    },
    ValueMap {
        pattern_value: PatternExpression,
        value_table: &'static [Option<i64>],
    },
    ValueMapFilled {
        pattern_value: PatternExpression,
        value_table: &'static [i64],
    },
    Name {
        pattern_value: PatternExpression,
        symbol_table: &'static [Option<&'static str>],
    },
    Varnode {
        name: &'static str,
        space: u8,
        offset: u64,
        size: u16,
    },
    VarnodeList {
        pattern_value: PatternExpression,
        varnode_table: &'static [Option<&'static Self>],
        symbol_table: &'static [Option<&'static str>],
    },
    VarnodeListFilled {
        pattern_value: PatternExpression,
        varnode_table: &'static [&'static Self],
        symbol_table: &'static [&'static str],
    },
    Operand {
        handle_index: usize,
    },
    Start {
        space: u8,
        size: u16,
    },
    End {
        space: u8,
        size: u16,
    },
    Next2 {
        space: u8,
        size: u16,
    },
}

impl Symbol {
    pub fn format<R: ConstructorResolver>(
        &self,
        state: &mut LiftingContextState<'_>,
        fmt: &mut fmt::Formatter,
    ) -> Result<(), fmt::Error> {
        match self {
            Self::Varnode { name, .. } => {
                fmt.write_str(name)?;
            }
            Self::Name {
                pattern_value,
                symbol_table,
            }
            | Self::VarnodeList {
                pattern_value,
                symbol_table,
                ..
            } => {
                let index = pattern_value.resolve::<R>(state).expect("resolved");
                if let Some(name) = symbol_table.get(index as usize).copied().flatten() {
                    fmt.write_str(name)?;
                }
            }
            Self::VarnodeListFilled {
                pattern_value,
                symbol_table,
                ..
            } => {
                let index = pattern_value.resolve::<R>(state).expect("resolved");
                if let Some(name) = symbol_table.get(index as usize).copied() {
                    fmt.write_str(name)?;
                }
            }
            Self::ValueMap {
                pattern_value,
                value_table,
            } => {
                let index = pattern_value.resolve::<R>(state).expect("resolved");
                if let Some(value) = value_table.get(index as usize).copied().flatten() {
                    if value < 0 {
                        write!(fmt, "-{:#x}", -(value as i128))?;
                    } else {
                        write!(fmt, "{:#x}", value)?;
                    }
                }
            }
            Self::Start { .. } => {
                write!(fmt, "{:#x}", state.address())?;
            }
            Self::End { .. } => {
                write!(fmt, "{:#x}", state.next_address())?;
            }
            Self::Next2 { .. } => {
                write!(fmt, "{:#x}", state.next2_address().expect("resolved"))?;
            }
            _ => unreachable!("this state should not be reachable"),
        }
        Ok(())
    }

    pub fn resolve_handle<R: ConstructorResolver>(
        &self,
        input: &mut LiftingContextState<'_>,
    ) -> Option<FixedHandle> {
        Some(match self {
            Symbol::Epsilon => FixedHandle {
                space: 0,
                ..Default::default()
            },
            Symbol::Name { pattern_value, .. } | Symbol::Value { pattern_value } => {
                let value = pattern_value.resolve::<R>(input)?;
                FixedHandle {
                    space: 0,
                    offset_offset: value as u64,
                    ..Default::default()
                }
            }
            Symbol::Varnode {
                space,
                offset,
                size,
                ..
            } => FixedHandle {
                space: *space,
                size: *size,
                offset_offset: *offset,
                ..Default::default()
            },
            Symbol::Operand { handle_index } => unsafe {
                input.input().unchecked_operand_handle(*handle_index)
            },
            Symbol::Start { space, size } => FixedHandle {
                space: *space,
                size: *size,
                offset_offset: input.address(),
                ..Default::default()
            },
            Symbol::End { space, size } => FixedHandle {
                space: *space,
                size: *size,
                offset_offset: input.next_address(),
                ..Default::default()
            },
            Symbol::Next2 { space, size } => FixedHandle {
                space: *space,
                size: *size,
                offset_offset: if let Some(next2_address) = input.next2_address() {
                    next2_address
                } else {
                    let mut ninput = input.next_input()?;
                    R::resolve(&mut ninput)?;
                    ninput.next_address()
                },
                ..Default::default()
            },
            Symbol::VarnodeList {
                pattern_value,
                varnode_table,
                ..
            } => {
                let index = pattern_value.resolve::<R>(input)? as usize;
                let symbol = varnode_table.get(index)?.as_ref()?;
                symbol.resolve_handle::<R>(input)?
            }
            Symbol::VarnodeListFilled {
                pattern_value,
                varnode_table,
                ..
            } => {
                let index = pattern_value.resolve::<R>(input)? as usize;
                let symbol = varnode_table.get(index)?;
                symbol.resolve_handle::<R>(input)?
            }
            Symbol::ValueMap {
                pattern_value,
                value_table,
            } => {
                let index = pattern_value.resolve::<R>(input)? as usize;
                let value = *value_table.get(index)?.as_ref()? as u64;

                FixedHandle {
                    space: 0,
                    offset_offset: value,
                    ..Default::default()
                }
            }
            Symbol::ValueMapFilled {
                pattern_value,
                value_table,
            } => {
                let index = pattern_value.resolve::<R>(input)? as usize;
                let value = *value_table.get(index)? as u64;

                FixedHandle {
                    space: 0,
                    offset_offset: value,
                    ..Default::default()
                }
            }
        })
    }
}
