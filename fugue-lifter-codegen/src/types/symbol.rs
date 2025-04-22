use fugue_sleigh_language::pattern::PatternExpression;
use fugue_sleigh_language::symbol::Symbol;
use fugue_sleigh_language::Language;

use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::Ident;

use crate::types::pattern::PatternExpressionAdaptor;

pub struct SymbolAdaptor<'a> {
    language: &'a Language,
    symbol: &'a Symbol,
}

impl<'a> SymbolAdaptor<'a> {
    pub fn new(language: &'a Language, symbol: &'a Symbol) -> Self {
        Self { language, symbol }
    }

    pub fn wrap(&self, symbol: &'a Symbol) -> Self {
        Self {
            language: self.language,
            symbol,
        }
    }

    pub(crate) fn identifier(&self) -> Ident {
        self.identifier_for(self.symbol.id())
    }

    pub(crate) fn filter_identifier(&self) -> Ident {
        self.filter_identifier_for(self.symbol.id())
    }

    fn filter_identifier_for(&self, id: usize) -> Ident {
        format_ident!("__SYM{id}_FILTER")
    }

    fn identifier_for(&self, id: usize) -> Ident {
        format_ident!("__SYM{id}")
    }

    fn build_filter(
        &self,
        pattern: &PatternExpression,
        indices: impl Iterator<Item = usize>,
        limit: usize,
    ) -> TokenStream {
        let ident = self.filter_identifier();
        let pvalue = PatternExpressionAdaptor::new(&self.language, pattern);
        quote! {
            pub(crate) const #ident: fugue_lifter_runtime::constructor::OperandFilter =
                fugue_lifter_runtime::constructor::OperandFilter {
                    pattern: #pvalue,
                    indices: &[#(#indices),*],
                    limit: #limit,
                };
        }
    }
}

impl<'a> ToTokens for SymbolAdaptor<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use Symbol as S;

        let value = match self.symbol {
            S::Epsilon { .. } => quote! {
                fugue_lifter_runtime::symbol::Symbol::Epsilon
            },
            S::Value { pattern_value, .. } => {
                let pvalue = PatternExpressionAdaptor::new(&self.language, pattern_value);
                quote! {
                    fugue_lifter_runtime::symbol::Symbol::Value {
                        pattern_value: #pvalue,
                    }
                }
            }
            S::ValueMap {
                pattern_value,
                value_table,
                table_is_filled,
                ..
            } => {
                let pvalue = PatternExpressionAdaptor::new(&self.language, pattern_value);

                if *table_is_filled {
                    let values = value_table.iter().copied();

                    quote! {
                        fugue_lifter_runtime::symbol::Symbol::ValueMapFilled {
                            pattern_value: #pvalue,
                            value_table: &[#(#values),*],
                        }
                    }
                } else {
                    let bad_indices = value_table.iter().enumerate().filter_map(|(i, v)| {
                        if *v == 0xbadbeef {
                            Some(i)
                        } else {
                            None
                        }
                    });
                    let limit = value_table.len();

                    self.build_filter(pattern_value, bad_indices, limit)
                        .to_tokens(tokens);

                    let values = value_table.iter().copied().map(|v| {
                        if v == 0xbadbeef {
                            None
                        } else {
                            Some(v)
                        }
                    });

                    quote! {
                        fugue_lifter_runtime::symbol::Symbol::ValueMap {
                            pattern_value: #pvalue,
                            value_table: &[#(#values),*],
                        }
                    }
                }
            }
            S::Name {
                pattern_value,
                name_table,
                table_is_filled,
                ..
            } => {
                if !*table_is_filled {
                    let bad_indices = name_table.iter().enumerate().filter_map(|(i, v)| {
                        if v == "\t" {
                            Some(i)
                        } else {
                            None
                        }
                    });
                    let limit = name_table.len();

                    self.build_filter(pattern_value, bad_indices, limit)
                        .to_tokens(tokens);
                }

                // NOTE: we could merge those cases that are behaviourally similar
                let pvalue = PatternExpressionAdaptor::new(&self.language, pattern_value);
                let symbols = name_table.iter().map(|v| {
                    if v == "\t" {
                        quote! { None }
                    } else {
                        quote! { Some(#v) }
                    }
                });

                quote! {
                    fugue_lifter_runtime::symbol::Symbol::Name {
                        pattern_value: #pvalue,
                        symbol_table: &[#(#symbols),*],
                    }
                }
            }
            S::Varnode {
                name,
                space,
                offset,
                size,
                ..
            } => {
                let name = name.as_str();
                let space = space.index() as u8;
                let offset = *offset;
                let size = *size as u16;

                quote! {
                    fugue_lifter_runtime::symbol::Symbol::Varnode {
                        name: #name,
                        space: #space,
                        offset: #offset,
                        size: #size,
                    }
                }
            }
            S::VarnodeList {
                pattern_value,
                varnode_table,
                table_is_filled,
                ..
            } => {
                let pvalue = PatternExpressionAdaptor::new(&self.language, pattern_value);

                if *table_is_filled {
                    let values = varnode_table
                        .iter()
                        .copied()
                        .map(|id| self.identifier_for(id.expect("table is filled") as usize));

                    let symbols = varnode_table.iter().copied().map(|id| {
                        let name = self
                            .language
                            .symbol_table()
                            .symbol(id.expect("table is filled") as usize)
                            .expect("valid symbol")
                            .name();
                        quote! { #name }
                    });

                    quote! {
                        fugue_lifter_runtime::symbol::Symbol::VarnodeListFilled {
                            pattern_value: #pvalue,
                            varnode_table: &[#(& #values),*],
                            symbol_table: &[#(#symbols),*],
                        }
                    }
                } else {
                    let bad_indices = varnode_table.iter().enumerate().filter_map(|(i, v)| {
                        if v.is_none() {
                            Some(i)
                        } else {
                            None
                        }
                    });
                    let limit = varnode_table.len();

                    self.build_filter(pattern_value, bad_indices, limit)
                        .to_tokens(tokens);

                    let values = varnode_table.iter().copied().map(|id| {
                        if let Some(id) = id {
                            let ident = self.identifier_for(id as usize);
                            quote! { Some(& #ident) }
                        } else {
                            quote! { None }
                        }
                    });

                    let symbols = varnode_table.iter().copied().map(|id| {
                        let Some(id) = id else {
                            return quote! { None };
                        };

                        let name = self
                            .language
                            .symbol_table()
                            .symbol(id)
                            .expect("valid symbol")
                            .name();

                        quote! { Some(#name) }
                    });

                    quote! {
                        fugue_lifter_runtime::symbol::Symbol::VarnodeList {
                            pattern_value: #pvalue,
                            varnode_table: &[#(#values),*],
                            symbol_table: &[#(#symbols),*],
                        }
                    }
                }
            }
            S::Operand { handle_index, .. } => {
                let handle_index = *handle_index;
                quote! {
                    fugue_lifter_runtime::symbol::Symbol::Operand {
                        handle_index: #handle_index,
                    }
                }
            }
            S::Start { .. } => {
                let space = self.language.spaces().default_space_ref();
                let id = space.index() as u8;
                let size = space.address_size() as u16;

                quote! {
                    fugue_lifter_runtime::symbol::Symbol::Start {
                        space: #id,
                        size: #size,
                    }
                }
            }
            S::End { .. } => {
                let space = self.language.spaces().default_space_ref();
                let id = space.index() as u8;
                let size = space.address_size() as u16;

                quote! {
                    fugue_lifter_runtime::symbol::Symbol::End {
                        space: #id,
                        size: #size,
                    }
                }
            }
            S::Next2 { .. } => {
                let space = self.language.spaces().default_space_ref();
                let id = space.index() as u8;
                let size = space.address_size() as u16;

                quote! {
                    fugue_lifter_runtime::symbol::Symbol::Next2 {
                        space: #id,
                        size: #size,
                    }
                }
            }
            _ => {
                return;
            }
        };

        let ident = self.identifier();
        let declaration = quote! {
            pub(crate) const #ident: fugue_lifter_runtime::symbol::Symbol = #value;
        };

        declaration.to_tokens(tokens)
    }
}
