use fugue_ir::disassembly::Symbol;
use fugue_ir::disassembly::symbol::sub_table::Context;
use fugue_ir::Translator;

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};

use crate::types::pattern::PatternExpressionAdaptor;

pub struct ContextAdaptor<'a> {
    translator: &'a Translator,
    context: &'a Context,
}

impl<'a> ContextAdaptor<'a> {
    pub fn new(translator: &'a Translator, context: &'a Context) -> Self {
        Self {
            translator,
            context,
        }
    }
}

impl<'a> ToTokens for ContextAdaptor<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use Context as C;

        let value = match self.context {
            C::Operator {
                num,
                shift,
                mask,
                pattern_value,
            } => {
                let num = *num;
                let shift = *shift;
                let mask = *mask;
                let value = PatternExpressionAdaptor::new(&self.translator, pattern_value);

                quote! {
                    fugue_lifter::runtime::context::ContextPreAction {
                        num: #num,
                        shift: #shift,
                        mask: #mask,
                        value: #value,
                    }
                }
            }
            C::Commit {
                symbol_id,
                num,
                mask,
                flow,
            } => {
                let symbol = self
                    .translator
                    .symbol_table()
                    .symbol(*symbol_id)
                    .expect("valid symbol");

                let handle = if let Symbol::Operand { handle_index, .. } = symbol {
                    let opid = *handle_index;
                    quote! { fugue_lifter::runtime::context::ContextPostActionHandle::Operand(#opid) }
                } else {
                    let ident = format_ident!("__SYM{symbol_id}");
                    quote! { fugue_lifter::runtime::context::ContextPostActionHandle::Symbol(&#ident) }
                };

                let space = self.translator.manager().default_space_ref();
                let word_size = space.word_size() as u64;
                let highest = space.highest_offset();
                let flow = *flow;

                // NOTE: when we apply the pre-context actions, we perform extraction operations
                // based on post actions.
                quote! {
                    fugue_lifter::runtime::context::ContextPostAction {
                        handle: #handle,
                        num: #num,
                        mask: #mask,
                        highest: #highest,
                        word_size: #word_size,
                        flow: #flow,
                    }
                }
            }
        };

        value.to_tokens(tokens)
    }
}
