use std::fmt::Debug;

use crate::runtime::input::FixedHandle;
use crate::runtime::pcode::LiftingContextState;

pub type ContextActionSet = fn(&mut LiftingContextState<'_>) -> Option<()>;

pub enum OperandResolver {
    None,
    Constructor(fn(&mut LiftingContextState<'_>) -> Option<&'static Constructor>),
    Filter(fn(&mut LiftingContextState<'_>) -> Option<()>),
}

pub type OperandHandleResolver = fn(&mut LiftingContextState) -> Option<()>;

pub struct Operand {
    pub resolver: OperandResolver,
    pub handle_resolver: Option<OperandHandleResolver>,
    pub offset_base: Option<usize>,
    pub offset_rela: usize,
    pub minimum_length: usize,
}

pub type ConstructorResult = fn(&mut LiftingContextState<'_>) -> FixedHandle;
pub type PCodeBuildAction = fn(&mut LiftingContextState<'_>) -> Option<()>;

pub struct Constructor {
    pub id: u32,
    pub context_actions: Option<ContextActionSet>,
    pub operands: &'static [Operand],
    pub result: Option<ConstructorResult>,
    pub build_action: Option<PCodeBuildAction>,
    pub print_pieces: &'static [&'static str],
    pub delay_slot_length: usize,
    pub minimum_length: usize,
}

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
    pub fn resolve_operands(&'static self, state: &mut LiftingContextState) -> Option<()> {
        state.input().set_constructor(self);
        if let Some(actions) = self.context_actions {
            (actions)(state)?;
        }

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
                        (filter)(state)?;
                    }
                    OperandResolver::Constructor(resolver) => {
                        let ctor = (resolver)(state)?;

                        state.input().set_constructor(ctor);
                        if let Some(actions) = ctor.context_actions {
                            (actions)(state)?;
                        }

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
    pub fn resolve_handles(&'static self, state: &mut LiftingContextState) -> Option<()> {
        state.input().base_state();

        'outer: while !state.input().resolved() {
            let ctor = state.input().constructor();
            let opid = state.input().operand();

            for (i, opnd) in ctor.operands.iter().enumerate().skip(opid) {
                state.input().push_operand(i);

                if let Some(resolver) = opnd.handle_resolver {
                    (resolver)(state)?;
                } else {
                    continue 'outer;
                }

                state.input().pop_operand();
            }

            if let Some(resolver) = ctor.result {
                let handle = (resolver)(state);
                state.input().set_parent_handle(handle);
            }

            state.input().pop_operand();
        }

        Some(())
    }
}
