use std::fmt::Debug;

use crate::utils::input::{FixedHandle, ParserInput};

pub type ContextActionSet = fn(&mut ParserInput) -> Option<()>;

pub enum OperandResolver {
    None,
    Constructor(fn(&mut ParserInput) -> Option<&'static Constructor>),
    Filter(fn(&mut ParserInput) -> Option<()>),
}

pub type OperandHandleResolver = fn(&mut ParserInput) -> Option<()>;

pub struct Operand {
    pub resolver: OperandResolver,
    pub handle_resolver: Option<OperandHandleResolver>,
    pub offset_base: Option<usize>,
    pub offset_rela: usize,
    pub minimum_length: usize,
}

pub type ConstructorResult = fn(&mut ParserInput) -> FixedHandle;

pub struct Constructor {
    pub id: u32,
    pub context_actions: Option<ContextActionSet>,
    pub operands: &'static [Operand],
    pub result: Option<ConstructorResult>,
    pub print_pieces: &'static [&'static str],
    pub delay_slots: usize,
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
    pub fn resolve_operands(&'static self, input: &mut ParserInput) -> Option<()> {
        input.set_constructor(self);
        if let Some(actions) = self.context_actions {
            (actions)(input)?;
        }

        if self.operands.is_empty() {
            input.calculate_length(self.minimum_length, self.operands.len());
            input.pop_operand();

            if self.delay_slots > 0 {
                input.set_delay_slot(self.delay_slots);
            }

            return Some(());
        }

        input.allocate_operands(self.operands.len());

        'outer: while !input.resolved() {
            let ctor = input.constructor();
            let opid = input.operand();

            for (i, opnd) in ctor.operands.iter().enumerate().skip(opid) {
                let offset = opnd
                    .offset_base
                    .map(|n| input.offset_for_operand(n))
                    .unwrap_or_else(|| input.offset())
                    + opnd.offset_rela;

                input.push_operand(i);
                input.set_offset(offset);

                match opnd.resolver {
                    OperandResolver::None => (),
                    OperandResolver::Filter(filter) => {
                        (filter)(input)?;
                    }
                    OperandResolver::Constructor(resolver) => {
                        let ctor = (resolver)(input)?;

                        input.set_constructor(ctor);
                        if let Some(actions) = ctor.context_actions {
                            (actions)(input)?;
                        }

                        if !ctor.operands.is_empty() {
                            input.allocate_operands(ctor.operands.len());
                        }

                        continue 'outer;
                    }
                }

                input.set_current_length(opnd.minimum_length);
                input.pop_operand();
            }

            input.calculate_length(ctor.minimum_length, ctor.operands.len());
            input.pop_operand();

            if ctor.delay_slots > 0 {
                input.set_delay_slot(ctor.delay_slots);
            }
        }

        Some(())
    }

    #[inline]
    pub fn resolve_handles(&'static self, input: &mut ParserInput) -> Option<()> {
        input.base_state();

        'outer: while !input.resolved() {
            let ctor = input.constructor();
            let opid = input.operand();

            for (i, opnd) in ctor.operands.iter().enumerate().skip(opid) {
                input.push_operand(i);

                if let Some(resolver) = opnd.handle_resolver {
                    (resolver)(input)?;
                } else {
                    continue 'outer;
                }

                input.pop_operand();
            }

            if let Some(resolver) = ctor.result {
                let handle = (resolver)(input);
                input.set_parent_handle(handle);
            }

            input.pop_operand();
        }

        Some(())
    }
}
