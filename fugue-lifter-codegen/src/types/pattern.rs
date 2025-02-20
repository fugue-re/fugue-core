use fugue_ir::disassembly::{PatternExpression, Symbol};
use fugue_ir::Translator;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

use crate::LifterGenerator;

pub struct PatternExpressionAdaptor<'a> {
    translator: &'a Translator,
    expression: &'a PatternExpression,
}

impl<'a> PatternExpressionAdaptor<'a> {
    pub fn new(translator: &'a Translator, expression: &'a PatternExpression) -> Self {
        Self {
            translator,
            expression,
        }
    }

    pub fn wrap(&self, expression: &'a PatternExpression) -> Self {
        Self {
            translator: self.translator,
            expression,
        }
    }
}

impl<'a> ToTokens for PatternExpressionAdaptor<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use PatternExpression as E;

        let value = match self.expression {
            E::StartInstruction => {
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::StartInstruction
                }
            }
            E::EndInstruction => {
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::EndInstruction
                }
            }
            E::Next2Instruction => {
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::Next2Instruction
                }
            }
            E::Constant { value } => {
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::Constant { value: #value }
                }
            }
            E::And(lhs, rhs) => {
                let lhs = self.wrap(lhs);
                let rhs = self.wrap(rhs);
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::And(&#lhs, &#rhs)
                }
            }
            E::Or(lhs, rhs) => {
                let lhs = self.wrap(lhs);
                let rhs = self.wrap(rhs);
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::Or(&#lhs, &#rhs)
                }
            }
            E::Xor(lhs, rhs) => {
                let lhs = self.wrap(lhs);
                let rhs = self.wrap(rhs);
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::Xor(&#lhs, &#rhs)
                }
            }
            E::Plus(lhs, rhs) => {
                let lhs = self.wrap(lhs);
                let rhs = self.wrap(rhs);
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::Plus(&#lhs, &#rhs)
                }
            }
            E::Sub(lhs, rhs) => {
                let lhs = self.wrap(lhs);
                let rhs = self.wrap(rhs);
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::Sub(&#lhs, &#rhs)
                }
            }
            E::Mult(lhs, rhs) => {
                let lhs = self.wrap(lhs);
                let rhs = self.wrap(rhs);
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::Mult(&#lhs, &#rhs)
                }
            }
            E::Div(lhs, rhs) => {
                let lhs = self.wrap(lhs);
                let rhs = self.wrap(rhs);
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::Div(&#lhs, &#rhs)
                }
            }
            E::LeftShift(lhs, rhs) => {
                let lhs = self.wrap(lhs);
                let rhs = self.wrap(rhs);
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::LeftShift(&#lhs, &#rhs)
                }
            }
            E::RightShift(lhs, rhs) => {
                let lhs = self.wrap(lhs);
                let rhs = self.wrap(rhs);
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::RightShift(&#lhs, &#rhs)
                }
            }
            E::Minus(rhs) => {
                let rhs = self.wrap(rhs);
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::Minus(&#rhs)
                }
            }
            E::Not(rhs) => {
                let rhs = self.wrap(rhs);
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::Not(&#rhs)
                }
            }
            E::TokenField {
                big_endian,
                sign_bit,
                bit_start,
                bit_end,
                byte_start,
                byte_end,
                shift,
            } => {
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::TokenField {
                        big_endian: #big_endian,
                        sign_bit: #sign_bit,
                        bit_start: #bit_start,
                        bit_end: #bit_end,
                        byte_start: #byte_start,
                        byte_end: #byte_end,
                        shift: #shift,
                    }
                }
            }
            E::ContextField {
                sign_bit,
                bit_start,
                bit_end,
                byte_start,
                byte_end,
                shift,
            } => {
                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::ContextField {
                        sign_bit: #sign_bit,
                        bit_start: #bit_start,
                        bit_end: #bit_end,
                        byte_start: #byte_start,
                        byte_end: #byte_end,
                        shift: #shift,
                    }
                }
            }
            E::Operand {
                index,
                table_id,
                constructor_id,
            } => {
                let symbols = self.translator.symbol_table();
                let table = symbols.symbol(*table_id).unwrap();
                let Symbol::Subtable {
                    constructors,
                    scope,
                    ..
                } = table
                else {
                    unreachable!("this state should not be reachable");
                };
                let ctor = &constructors[*constructor_id];

                let Symbol::Operand {
                    def_expr,
                    subsym_id,
                    ..
                } = symbols.symbol(ctor.operand(*index)).unwrap()
                else {
                    unreachable!("this state should not be reachable");
                };

                let pexpr = if let Some(def_expr) = def_expr.as_ref() {
                    def_expr
                } else if let Some(subsym_id) = subsym_id.as_ref() {
                    let sym = symbols.symbol(*subsym_id).unwrap();
                    sym.pattern_value()
                } else {
                    quote! {
                        fugue_lifter::runtime::pattern::PatternExpression::Constant { value: 0i64 }
                    }
                    .to_tokens(tokens);
                    return;
                };

                let index = *index;
                let symbol = ctor.operand(index);
                let operand = self.translator.symbol_table().symbol(symbol).unwrap();

                let ctor_vname = LifterGenerator::ctor_vname(*table_id, *scope, *constructor_id);
                let value = self.wrap(pexpr);

                let rel_offset = operand.relative_offset() as u8;
                let offset = if operand.offset_base().is_none() {
                    quote! {
                        fugue_lifter::runtime::pattern::OperandOffset::Relative(#rel_offset)
                    }
                } else {
                    let index = index as u8;
                    quote! {
                        fugue_lifter::runtime::pattern::OperandOffset::Operand(#index)
                    }
                };

                quote! {
                    fugue_lifter::runtime::pattern::PatternExpression::Operand {
                        constructor: &#ctor_vname,
                        offset: #offset,
                        value: &#value,
                    }
                }
            }
        };
        value.to_tokens(tokens)
    }
}
