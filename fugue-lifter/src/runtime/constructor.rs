use std::fmt;
use std::fmt::Debug;

use crate::runtime::context::{ContextPostAction, ContextPreAction};
use crate::runtime::input::{ContextCommit, FixedHandle, INVALID_HANDLE};
use crate::runtime::pattern::PatternExpression;
use crate::runtime::pcode::LiftingContextState;
use crate::runtime::symbol::Symbol;
use crate::runtime::template::{ConstructTpl, HandleTpl};

pub type ContextActionSet = fn(&mut LiftingContextState<'_>) -> Option<()>;

pub enum OperandResolver {
    None,
    Constructor(fn(&mut LiftingContextState<'_>) -> Option<&'static Constructor>),
    Filter(&'static OperandFilter),
}

pub struct OperandFilter {
    pub pattern: PatternExpression,
    pub indices: &'static [usize],
    pub limit: usize,
}

impl OperandFilter {
    #[inline]
    pub fn validate<R: ConstructorResolver>(
        &self,
        input: &mut LiftingContextState,
    ) -> Option<()> {
        let index = self.pattern.resolve::<R>(input)? as usize;
        if index >= self.limit || self.indices.contains(&index) {
            None
        } else {
            Some(())
        }
    }
}

pub enum OperandHandleResolver {
    None,
    Symbol(&'static Symbol),
    Expression(PatternExpression),
}

pub struct Operand {
    pub resolver: OperandResolver,
    pub handle_resolver: OperandHandleResolver,
    pub offset_base: Option<usize>,
    pub offset_rela: usize,
    pub minimum_length: usize,
}

pub trait ConstructorResolver {
    const ADDRESS_SIZE: usize;
    const DEFAULT_SPACE: u8;
    const UNIQUE_SPACE: u8;

    fn resolve(input: &mut LiftingContextState) -> Option<&'static Constructor>;
    fn resolve_upper_bound(space: u8) -> u64;
    fn resolve_word_size(space: u8) -> usize;
    fn resolve_location_offset(unique_offset: u64, space: u8, offset: u64, size: u16) -> u64;
}

pub type ConstructorResult = fn(&mut LiftingContextState<'_>) -> FixedHandle;

pub type PCodeBuildAction = fn(&mut LiftingContextState<'_>) -> Option<()>;

pub struct Constructor {
    pub id: u32,
    pub context_pre_actions: &'static [ContextPreAction],
    pub context_post_actions: &'static [ContextPostAction],
    pub operands: &'static [Operand],
    pub result: Option<HandleTpl>,
    pub build_action: Option<ConstructTpl>,
    pub print_pieces: &'static [PrintPiece],
    pub delay_slot_length: usize,
    pub minimum_length: usize,
}

pub enum PrintPiece {
    Operand(usize),
    Token(&'static str),
}

// print pieces:
// - Symbol(operand index)
// - Token


impl Debug for Constructor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let p0 = self.id & 0xff;
        let p1 = self.id >> 16;
        write!(f, "Constructor{p0}In{p1}")
    }
}

impl PartialEq for Constructor {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Constructor {}

impl Constructor {
    #[inline]
    pub fn apply_context_actions<R: ConstructorResolver>(
        &'static self,
        state: &mut LiftingContextState,
    ) -> Option<()> {
        for action in self.context_pre_actions {
            action.apply::<R>(state)?;
        }

        for action in self.context_post_actions {
            let value = action.extract(state);
            let commit = ContextCommit {
                action,
                point: state.input().point,
                value,
            };
            state.input().register_context_commit(commit);
        }

        Some(())
    }

    #[inline]
    pub fn resolve_operands<R: ConstructorResolver>(
        &'static self,
        state: &mut LiftingContextState,
    ) -> Option<()> {
        state.input().set_constructor(self);

        self.apply_context_actions::<R>(state)?;

        if self.operands.is_empty() {
            state
                .input()
                .calculate_length(self.minimum_length, self.operands.len());
            state.input().pop_operand();

            if self.delay_slot_length > 0 {
                state.input().set_delay_slot_length(self.delay_slot_length);
            }

            return Some(());
        }

        state.input().allocate_operands(self.operands.len())?;

        'outer: while !state.input().resolved() {
            let ctor = state.input().constructor();
            let opid = state.input().operand();

            for (i, opnd) in ctor.operands.iter().enumerate().skip(opid) {
                let offset = opnd
                    .offset_base
                    .map(|n| state.input().offset_for_operand(n))
                    .unwrap_or_else(|| state.input().offset())
                    + opnd.offset_rela;

                state.input().push_operand(i);
                state.input().set_offset(offset);

                match opnd.resolver {
                    OperandResolver::None => (),
                    OperandResolver::Filter(filter) => {
                        filter.validate::<R>(state)?;
                    }
                    OperandResolver::Constructor(resolver) => {
                        let ctor = (resolver)(state)?;

                        state.input().set_constructor(ctor);

                        ctor.apply_context_actions::<R>(state)?;

                        if !ctor.operands.is_empty() {
                            state.input().allocate_operands(ctor.operands.len())?;
                        }

                        continue 'outer;
                    }
                }

                state.input().set_current_length(opnd.minimum_length);
                state.input().pop_operand();
            }

            state
                .input()
                .calculate_length(ctor.minimum_length, ctor.operands.len());
            state.input().pop_operand();

            if ctor.delay_slot_length > 0 {
                state.input().set_delay_slot_length(ctor.delay_slot_length);
            }
        }

        Some(())
    }

    #[inline]
    pub fn resolve_handles<R: ConstructorResolver>(
        &'static self,
        state: &mut LiftingContextState,
    ) -> Option<()> {
        state.input().base_state();

        'outer: while !state.input().resolved() {
            let ctor = state.input().constructor();
            let opid = state.input().operand();

            for (i, opnd) in ctor.operands.iter().enumerate().skip(opid) {
                state.input().push_operand(i);

                match opnd.handle_resolver {
                    OperandHandleResolver::None => {
                        continue 'outer;
                    }
                    OperandHandleResolver::Symbol(ref symbol) => {
                        let handle = symbol.resolve_handle::<R>(state)?;
                        state.input().set_parent_handle(handle);
                    }
                    OperandHandleResolver::Expression(ref expr) => {
                        let offset = expr.resolve::<R>(state)? as u64;

                        if let Some(handle) = state.input().parent_handle_mut() {
                            handle.space = 0;
                            handle.offset_space = INVALID_HANDLE;
                            handle.offset_offset = offset;
                            handle.size = 0;
                        } else {
                            state.input().set_parent_handle(FixedHandle {
                                space: 0,
                                offset_offset: offset,
                                ..Default::default()
                            });
                        }
                    }
                }

                state.input().pop_operand();
            }

            if let Some(tmpl) = &ctor.result {
                let handle = tmpl.build::<R>(state)?;
                state.input().set_parent_handle(handle);
            }

            state.input().pop_operand();
        }

        Some(())
    }

    pub fn format<R: ConstructorResolver>(
        &self,
        state: &mut LiftingContextState<'_>,
        fmt: &mut fmt::Formatter,
    ) -> Result<(), fmt::Error> {
        for p in self.print_pieces {
            match p {
                PrintPiece::Operand(index) => {
                    state.input().push_operand(*index);
                    match &self.operands[*index].handle_resolver {
                        OperandHandleResolver::None => {
                            state.input().constructor().format::<R>(state, fmt)?;
                        },
                        OperandHandleResolver::Symbol(symbol) => {
                            symbol.format::<R>(state, fmt)?;
                        }
                        OperandHandleResolver::Expression(expr) => {
                            expr.format::<R>(state, fmt)?;
                        }
                    }
                    state.input().pop_operand();
                }
                PrintPiece::Token(token) => {
                    fmt.write_str(token)?;
                }
            }
        }
        Ok(())
    }
}
