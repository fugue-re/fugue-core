use std::mem::size_of;

use fugue_sleigh_language::construct::{ConstructTpl, HandleTpl};
use fugue_sleigh_language::pattern::PatternExpression;
use fugue_sleigh_language::symbol::sub_table::{
    Context, ContextPattern, DecisionPair, DisjointPattern, InstructionPattern,
};
use fugue_sleigh_language::symbol::{Constructor, DecisionNode, Symbol};
use fugue_sleigh_language::Language;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens, TokenStreamExt};
use syn::Ident;

use crate::types::context::ContextAdaptor;
use crate::types::pattern::PatternExpressionAdaptor;
use crate::types::symbol::SymbolAdaptor;
use crate::types::template::TplAdaptor;
use crate::LifterGeneratorError;

pub struct LifterGenerator<'a> {
    context_variables: Vec<(&'a str, usize, usize)>,
    symbols: Vec<TokenStream>,
    language: &'a Language,
}

impl<'a> LifterGenerator<'a> {
    pub fn new(language: &'a Language) -> Result<Self, LifterGeneratorError> {
        let mut slf = Self {
            context_variables: Vec::new(),
            symbols: Vec::new(),
            language,
        };

        slf.build()?;

        Ok(slf)
    }

    pub fn build(&mut self) -> Result<(), LifterGeneratorError> {
        let symtab = self.language.symbol_table();

        for symbol in symtab.symbols().iter() {
            if let Symbol::Subtable {
                id,
                scope,
                constructors,
                decision_tree,
                ..
            } = symbol
            {
                self.symbols.push(self.generate_subtable(
                    *id,
                    *scope,
                    constructors,
                    decision_tree,
                )?);
            } else {
                self.symbols
                    .push(SymbolAdaptor::new(&self.language, symbol).to_token_stream());
            }
        }

        for &sym_id in symtab.global_scope().unwrap().iter() {
            let Symbol::Context {
                ref name,
                ref pattern_value,
                ..
            } = symtab.symbol(sym_id).unwrap()
            else {
                continue;
            };

            if let PatternExpression::ContextField {
                bit_start, bit_end, ..
            } = pattern_value
            {
                self.context_variables.push((name, *bit_start, *bit_end));
            }
        }

        Ok(())
    }

    fn generate_constructor_operand_resolvers(&self, ctor: &Constructor) -> Vec<TokenStream> {
        let mut operands = Vec::new();

        for oid in 0..ctor.operand_count() {
            let index = ctor.operand(oid);
            let operand = self.language.symbol_table().symbol(index).unwrap();

            let offset_base = operand
                .offset_base()
                .map(|v| quote! { Some(#v) })
                .unwrap_or(quote! { None });
            let offset_rela = operand.relative_offset();
            let minimum_length = if let Symbol::Operand { min_length, .. } = operand {
                min_length
            } else {
                unreachable!()
            };

            let (resolver, handle_resolver) = if let Some(tsym) =
                operand.defining_symbol(self.language.symbol_table())
            {
                match tsym {
                    Symbol::Subtable { id, scope, .. } => {
                        let stname = format_ident!("SubTable{id}In{scope}");
                        let resolver = quote! { fugue_lifter_runtime::OperandResolver::Constructor(<#stname>::resolve) };
                        let handle_resolver =
                            quote! { fugue_lifter_runtime::OperandHandleResolver::None };

                        (resolver, handle_resolver)
                    }
                    Symbol::ValueMap {
                        id,
                        table_is_filled,
                        ..
                    } => {
                        let resolver = if *table_is_filled {
                            quote! { fugue_lifter_runtime::OperandResolver::None }
                        } else {
                            let ident = format_ident!("__SYM{id}_FILTER");
                            quote! { fugue_lifter_runtime::OperandResolver::Filter(&#ident) }
                        };

                        let ident = format_ident!("__SYM{id}");
                        let handle_resolver =
                            quote! { fugue_lifter_runtime::OperandHandleResolver::Symbol(&#ident) };

                        (resolver, handle_resolver)
                    }
                    Symbol::VarnodeList {
                        id,
                        table_is_filled,
                        ..
                    } => {
                        let resolver = if *table_is_filled {
                            quote! { fugue_lifter_runtime::OperandResolver::None }
                        } else {
                            let ident = format_ident!("__SYM{id}_FILTER");
                            quote! { fugue_lifter_runtime::OperandResolver::Filter(&#ident) }
                        };

                        let ident = format_ident!("__SYM{id}");
                        let handle_resolver =
                            quote! { fugue_lifter_runtime::OperandHandleResolver::Symbol(&#ident) };

                        (resolver, handle_resolver)
                    }
                    Symbol::Name {
                        id,
                        table_is_filled,
                        ..
                    } => {
                        let resolver = if *table_is_filled {
                            quote! { fugue_lifter_runtime::OperandResolver::None }
                        } else {
                            let ident = format_ident!("__SYM{id}_FILTER");
                            quote! { fugue_lifter_runtime::OperandResolver::Filter(&#ident) }
                        };

                        let ident = format_ident!("__SYM{id}");
                        let handle_resolver =
                            quote! { fugue_lifter_runtime::OperandHandleResolver::Symbol(&#ident) };

                        (resolver, handle_resolver)
                    }
                    symbol => {
                        let resolver = quote! { fugue_lifter_runtime::OperandResolver::None };

                        let id = symbol.id();
                        let ident = format_ident!("__SYM{id}");
                        let handle_resolver =
                            quote! { fugue_lifter_runtime::OperandHandleResolver::Symbol(&#ident) };

                        (resolver, handle_resolver)
                    }
                }
            } else {
                let resolver = quote! { fugue_lifter_runtime::OperandResolver::None };

                let pexp = operand.defining_expression().unwrap();
                let value = PatternExpressionAdaptor::new(&self.language, pexp);
                let handle_resolver =
                    quote! { fugue_lifter_runtime::OperandHandleResolver::Expression(#value) };

                (resolver, handle_resolver)
            };

            operands.push(quote! {
                fugue_lifter_runtime::Operand {
                    resolver: #resolver,
                    handle_resolver: #handle_resolver,
                    offset_base: #offset_base,
                    offset_rela: #offset_rela,
                    minimum_length: #minimum_length,
                }
            });
        }

        operands
    }

    fn generate_constructor_context_actions(
        &self,
        ctor: &Constructor,
    ) -> (Vec<TokenStream>, Vec<TokenStream>) {
        let mut pre_actions = Vec::new();
        let mut post_actions = Vec::new();

        for action in ctor.context().iter() {
            match action {
                Context::Operator { .. } => {
                    pre_actions.push(ContextAdaptor::new(&self.language, action).to_token_stream());
                }
                Context::Commit { .. } => {
                    post_actions
                        .push(ContextAdaptor::new(&self.language, action).to_token_stream());
                }
            }
        }

        (pre_actions, post_actions)
    }

    fn generate_handle_template(&self, tmpl: &HandleTpl) -> TokenStream {
        TplAdaptor::new(&self.language, tmpl).to_token_stream()
    }

    fn generate_constructor_template_resolvers(&self, ctor: &Constructor) -> TokenStream {
        if let Some(templ) = ctor.template().and_then(ConstructTpl::result) {
            let action = self.generate_handle_template(templ);
            quote! {
                Some(#action)
            }
        } else {
            quote! { None }
        }
    }

    fn generate_constructor_build_action(&self, tmpl: &ConstructTpl) -> TokenStream {
        TplAdaptor::new(&self.language, tmpl).to_token_stream()
    }

    fn generate_constructor_lifting_actions(&self, ctor: &Constructor) -> TokenStream {
        if let Some(tmpl) = ctor.template() {
            let template = self.generate_constructor_build_action(tmpl);
            quote! { Some(#template) }
        } else {
            quote! { None }
        }
    }

    fn generate_constructors<'b>(
        &'b self,
        id: usize,
        scope: usize,
        ctors: &'a [Constructor],
    ) -> impl Iterator<Item = TokenStream> + 'b {
        ctors.iter().enumerate().map(move |(cid, ctor)| {
            let ctor_vname = Self::ctor_vname(id, scope, cid);

            let (ctor_id1, ctor_id2) = ctor.id();
            let ctor_id = (ctor_id1 as u32 & 0xffff) << 16 | (ctor_id2 as u32 & 0xffff);

            let delay_slot_length = ctor.template().map(|tpl| tpl.delay_slot()).unwrap_or_default();
            let minimum_length = ctor.minimum_length();

            let pieces = ctor.print_pieces().iter().map(|piece| if piece.as_bytes()[0] == b'\n' {
                let index = (piece.as_bytes()[1] - b'A') as usize;
                quote! { fugue_lifter_runtime::constructor::PrintPiece::Operand(#index) }
            } else {
                quote! { fugue_lifter_runtime::constructor::PrintPiece::Token(#piece) }
            });

            let first_whitespace = ctor.first_whitespace()
                .map_or_else(|| quote! { None }, |index| quote! { Some(#index) });

            let flow_through_index = ctor.flow_through_index()
                .map_or_else(|| quote! { None }, |index| quote! { Some(#index) });

            let operands = self.generate_constructor_operand_resolvers(ctor);
            let (pre_actions, post_actions) = self.generate_constructor_context_actions(ctor);

            let template_result = self.generate_constructor_template_resolvers(ctor);
            let lifting_action = self.generate_constructor_lifting_actions(ctor);

            quote! {
                pub static #ctor_vname: fugue_lifter_runtime::Constructor = fugue_lifter_runtime::Constructor {
                    id: #ctor_id,
                    context_pre_actions: &[#(#pre_actions),*],
                    context_post_actions: &[#(#post_actions),*],
                    operands: &[#(#operands),*],
                    result: #template_result,
                    build_action: #lifting_action,
                    print_pieces: &[#(#pieces),*],
                    first_whitespace: #first_whitespace,
                    flow_through_index: #flow_through_index,
                    delay_slot_length: #delay_slot_length,
                    minimum_length: #minimum_length,
                };
            }
        })
    }

    pub(crate) fn ctor_vname(id: usize, scope: usize, cid: usize) -> Ident {
        format_ident!("__SYM{id}_IN{scope}_CTOR{cid}")
    }

    fn generate_dtree_pmatch_ctxt(&self, cpat: &ContextPattern) -> TokenStream {
        let pat = cpat.mask_value();

        if pat.always_true() {
            return quote! { true };
        }

        if pat.always_false() {
            return quote! { false };
        }

        let parts = pat
            .masks()
            .iter()
            .zip(pat.values().iter())
            .enumerate()
            .map(|(i, (m, v))| {
                let size = size_of::<u32>();
                let offset = pat.offset() + i * size;

                quote! {
                    (input.inputs.input.context_bytes(#offset, #size) & #m == #v)
                }
            });

        quote! {
            (true #( && #parts )*)
        }
    }

    fn generate_dtree_pmatch_insn(&self, ipat: &InstructionPattern) -> TokenStream {
        let pat = ipat.mask_value();

        if pat.always_true() {
            return quote! { true };
        }

        if pat.always_false() {
            return quote! { false };
        }

        let parts = pat
            .masks()
            .iter()
            .zip(pat.values().iter())
            .enumerate()
            .map(|(i, (m, v))| {
                let size = size_of::<u32>();
                let offset = pat.offset() + i * size;

                quote! {
                    (input.inputs.input.instruction_bytes(#offset, #size)? & #m == #v)
                }
            });

        quote! {
            (true #( && #parts )*)
        }
    }

    fn generate_dtree_pmatch(&self, id: usize, scope: usize, pat: &DecisionPair) -> TokenStream {
        match pat.pattern() {
            DisjointPattern::Instruction(ipat) => {
                let cid = pat.id();
                let ctor = Self::ctor_vname(id, scope, cid);
                let cond = self.generate_dtree_pmatch_insn(ipat);

                quote! {
                    if #cond {
                        return Some(& #ctor);
                    }
                }
            }
            DisjointPattern::Context(cpat) => {
                let cid = pat.id();
                let ctor = Self::ctor_vname(id, scope, cid);
                let cond = self.generate_dtree_pmatch_ctxt(cpat);

                quote! {
                    if #cond {
                        return Some(& #ctor);
                    }
                }
            }
            DisjointPattern::Combine {
                context: cpat,
                instruction: ipat,
            } => {
                let cid = pat.id();
                let ctor = Self::ctor_vname(id, scope, cid);

                let ccond = self.generate_dtree_pmatch_ctxt(cpat);
                let icond = self.generate_dtree_pmatch_insn(ipat);

                quote! {
                    if #icond && #ccond {
                        return Some(& #ctor);
                    }
                }
            }
        }
    }

    fn generate_dtree_aux(
        &self,
        id: usize,
        scope: usize,
        dtree: &DecisionNode,
        tree_fn_prefix: &Ident,
        trees: &mut Vec<TokenStream>,
    ) -> TokenStream {
        if dtree.size() == 0 {
            // This is a leaf
            let parts = dtree
                .patterns()
                .iter()
                .map(|pat| self.generate_dtree_pmatch(id, scope, pat));

            quote! {
                #(#parts)*
                return None;
            }
        } else {
            // This is a node--generate a function call for each body
            let parts = dtree
                .children()
                .iter()
                .enumerate()
                .map(|(i, node)| {
                    let bitn = i as u32;
                    let tree_fn = format_ident!("{tree_fn_prefix}_{bitn}");
                    let body = self.generate_dtree_aux(id, scope, node, &tree_fn, trees);

                    trees.push(quote! {
                        #[inline]
                        fn #tree_fn(input: &mut fugue_lifter_runtime::LiftingContextState) -> Option<&'static fugue_lifter_runtime::Constructor> {
                            unsafe {
                                #body
                            }
                        }
                    });

                    quote! {
                        (#tree_fn as fn(&mut fugue_lifter_runtime::LiftingContextState) -> Option<&'static fugue_lifter_runtime::Constructor>)
                    }
                })
                .collect::<Vec<_>>();

            let start_bit = dtree.start_bit();
            let size = dtree.size();

            let check = if dtree.context_decision() {
                quote! { input.inputs.input.context_bits(#start_bit, #size) }
            } else {
                quote! { input.inputs.input.instruction_bits(#start_bit, #size)? }
            };

            let nodes = dtree.children().len();

            let table = Ident::new(
                &format!("{tree_fn_prefix}_LOOKUP").to_uppercase(),
                proc_macro2::Span::call_site(),
            );

            trees.push(quote! {
                const #table: [fn(&mut fugue_lifter_runtime::LiftingContextState) -> Option<&'static fugue_lifter_runtime::Constructor>; #nodes] = [
                    #(#parts),*
                ];
            });

            quote! {
                (#table.get(#check as usize)?)(input)
            }
        }
    }

    fn generate_dtree(
        &self,
        id: usize,
        scope: usize,
        dtree: &DecisionNode,
        trees: &mut Vec<TokenStream>,
    ) -> TokenStream {
        // This will give us the body for a resolver; we should also allow to process sub-ctors
        let tree_fn = format_ident!("resolve_{id}_in_{scope}");
        let body = self.generate_dtree_aux(id, scope, dtree, &tree_fn, trees);
        quote! {
            #[inline]
            pub fn resolve(input: &mut fugue_lifter_runtime::LiftingContextState) -> Option<&'static fugue_lifter_runtime::Constructor> {
                unsafe {
                    #body
                }
            }
        }
    }

    fn generate_subtable(
        &self,
        id: usize,
        scope: usize,
        ctors: &[Constructor],
        dtree: &DecisionNode,
    ) -> Result<TokenStream, LifterGeneratorError> {
        let tname = format_ident!("SubTable{id}In{scope}");
        let mut trees = Vec::new();

        let ctor_tokens = self.generate_constructors(id, scope, ctors);
        let dtree_tokens = self.generate_dtree(id, scope, dtree, &mut trees);

        let tokens = quote! {
            #(#ctor_tokens)*

            #(#trees)*

            #[derive(Debug, Clone, Copy)]
            struct #tname;

            impl #tname {
                #[allow(unused_parens)]
                #dtree_tokens
            }
        };

        Ok(tokens)
    }
}

impl<'a> ToTokens for LifterGenerator<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let alignment = self.language.alignment();
        let unique_mask = self.language.unique_mask();

        let default_space = self.language.spaces().default_space_ref();

        let constant_space_id = self.language.spaces().constant_space_id().index() as u8;
        let default_space_id = default_space.index() as u8;
        let register_space_id = self.language.spaces().register_space_id().index() as u8;
        let unique_space_id = self.language.spaces().unique_space_id().index() as u8;

        let address_size = default_space.address_size();
        let address_bits = address_size as u32 * 8;
        let max_address = default_space.highest_offset();

        let register_space_size = self.language.register_space_size();
        let unique_space_size = self.language.unique_space_size();

        let mut userops = Vec::new();
        let mut userop_to_ids = Vec::new();
        let mut userop_to_names = Vec::new();

        for (i, op) in self.language.user_ops().iter().enumerate() {
            let id = i as u16;
            let name = op.as_str();

            let upper_snake_name = Ident::new(
                &heck::AsShoutySnakeCase(name).to_string(),
                Span::call_site(),
            );

            userops.push(quote! {
                pub const #upper_snake_name: u16 = #id;
            });
            userop_to_names.push(name);
            userop_to_ids.push(quote! { #name => #id });
        }

        let n_userops = self.language.user_ops().len();

        let space_word_sizes = self.language.spaces().iter().map(|spc| spc.word_size());

        let space_upper_bounds = self
            .language
            .spaces()
            .iter()
            .map(|spc| spc.highest_offset());

        let n_spaces = self.language.spaces().len();

        let space_cases = self.language.spaces().iter().enumerate().map(|(i, spc)| {
            let i = i as u8;
            if i == 0 {
                // constant
                quote! { #i => offset & fugue_lifter_runtime::calculate_mask(size as usize) }
            } else if spc.id().is_unique() {
                quote! { #i => offset | unique_offset }
            } else {
                let highest = spc.highest_offset();
                quote! { #i => fugue_lifter_runtime::wrap_offset(#highest, offset) }
            }
        });

        let space_match = quote! {
            match space {
                #(#space_cases,)*
                _ => unreachable!("invalid space"),
            }
        };

        let space_names = self.language.spaces().iter().map(|spc| {
            let name = spc.name();
            quote! { #name }
        });

        let space_ids = self.language.spaces().iter().enumerate().map(|(i, spc)| {
            let name = spc.name();
            let i = i as u8;
            quote! { #name => #i }
        });

        let context_variable_consts = self.context_variables.iter().map(|(name, start, end)| {
            let upper_snake_name = Ident::new(
                &heck::AsShoutySnakeCase(*name).to_string(),
                Span::call_site(),
            );
            quote! {
                pub const #upper_snake_name: fugue_lifter_runtime::context::ContextBitRange =
                    fugue_lifter_runtime::context::ContextBitRange::new(#start, #end);
            }
        });

        let context_variable_names = self.context_variables.iter().map(|(name, _, _)| {
            let upper_snake_name = Ident::new(
                &heck::AsShoutySnakeCase(*name).to_string(),
                Span::call_site(),
            );
            quote! {
                #name => #upper_snake_name
            }
        });

        let context_variable_registrations =
            self.context_variables.iter().map(|(name, start, end)| {
                quote! {
                    context.register_variable(#name, #start, #end);
                }
            });

        let n_registers = self.language.registers().name_mapping().len();

        let mut registers = Vec::with_capacity(n_registers);
        let mut register_names = Vec::with_capacity(n_registers);
        let mut register_ranges = Vec::with_capacity(n_registers);

        for ((off, sz), nm) in self.language.registers().iter() {
            let off = *off;
            let sz = *sz as u16;

            let upper_snake_name = Ident::new(
                &heck::AsShoutySnakeCase(nm.as_str()).to_string(),
                Span::call_site(),
            );

            // for register by known symbol/const
            let var = quote! {
                pub const #upper_snake_name: fugue_lifter_runtime::pcode::Varnode =
                    fugue_lifter_runtime::pcode::Varnode::new(#register_space_id, #off, #sz);
            };

            let nm = nm.as_str();

            // for name to varnode mapping
            let name_to_varnode = quote! {
                #nm => #upper_snake_name
            };

            // for range (off, sz) to name mapping
            let range_to_name = quote! {
                (#off, #sz, #nm)
            };

            registers.push(var);
            register_names.push(name_to_varnode);
            register_ranges.push(range_to_name);
        }

        let language_id = self.language.architecture().to_string();
        let little_endian = self.language.architecture().endian().is_little();
        let variant = self.language.architecture().variant();

        tokens.append_all(quote! {
            pub const LANGUAGE_ID: &'static str = #language_id;

            pub const ADDRESS_ALIGNMENT: usize = #alignment;
            pub const ADDRESS_BITS: u32 = #address_bits;
            pub const ADDRESS_SIZE: usize = #address_size;

            pub const ADDRESS_UPPER_BOUND: u64 = #max_address;

            pub const CONSTANT_SPACE: u8 = #constant_space_id;
            pub const DEFAULT_SPACE: u8 = #default_space_id;

            pub const REGISTER_SPACE: u8 = #register_space_id;
            pub const REGISTER_SPACE_SIZE: usize = #register_space_size;

            pub const UNIQUE_MASK: u64 = #unique_mask;
            pub const UNIQUE_SPACE: u8 = #unique_space_id;
            pub const UNIQUE_SPACE_SIZE: usize = #unique_space_size;

            pub const SPACE_WORD_SIZE: [usize; #n_spaces] = [
                #(#space_word_sizes),*
            ];
            pub const SPACE_UPPER_BOUND: [u64; #n_spaces] = [
                #(#space_upper_bounds),*
            ];

            pub mod context {
                use fugue_lifter_runtime::phf;

                #(#context_variable_consts)*

                static CONTEXT_VARIABLES_TO_BITS: phf::Map<&'static str, fugue_lifter_runtime::context::ContextBitRange> = phf::phf_map! {
                    #(#context_variable_names,)*
                };

                #[inline(always)]
                pub fn context_variable_by_name(
                    name: &str,
                ) -> Option<fugue_lifter_runtime::context::ContextBitRange> {
                    CONTEXT_VARIABLES_TO_BITS.get(name).copied()
                }
            }

            pub mod space {
                use fugue_lifter_runtime::phf;

                pub const SPACES: [&'static str; #n_spaces] = [
                    #(#space_names,)*
                ];

                static SPACES_TO_ID: phf::Map<&'static str, u8> = phf::phf_map! {
                    #(#space_ids,)*
                };

                #[inline(always)]
                pub fn space_by_name(
                    name: &str,
                ) -> Option<u8> {
                    SPACES_TO_ID.get(name).copied()
                }

                #[inline]
                pub fn space_name(
                    space: u8,
                ) -> Option<&'static str> {
                    SPACES.get(space as usize).copied()
                }
            }

            pub mod register {
                use fugue_lifter_runtime::phf;

                #(#registers)*

                pub const REGISTERS: [(u64, u16, &'static str); #n_registers] = [
                    #(#register_ranges),*
                ];

                static REGISTERS_TO_VARNODE: phf::Map<&'static str, fugue_lifter_runtime::pcode::Varnode> = phf::phf_map! {
                    #(#register_names,)*
                };

                #[inline(always)]
                pub fn register_by_name(
                    name: &str,
                ) -> Option<fugue_lifter_runtime::pcode::Varnode> {
                    REGISTERS_TO_VARNODE.get(name).copied()
                }

                #[inline]
                pub fn register_name(
                    varnode: &fugue_lifter_runtime::pcode::Varnode,
                ) -> Option<&'static str> {
                    if varnode.space != #register_space_id {
                        return None;
                    }

                    let key = (varnode.offset, varnode.size);

                    REGISTERS.binary_search_by(|&(off, sz, _)| (off, sz).cmp(&key))
                        .ok()
                        .map(|pos| REGISTERS[pos].2)
                }
            }

            pub mod user_op {
                use fugue_lifter_runtime::phf;

                #(#userops)*

                pub const USER_OPS: [&'static str; #n_userops] = [
                    #(#userop_to_names),*
                ];

                static USER_OPS_TO_IDS: phf::Map<&'static str, u16> = phf::phf_map! {
                    #(#userop_to_ids,)*
                };

                #[inline(always)]
                pub fn user_op_by_name(name: &str) -> Option<u16> {
                    USER_OPS_TO_IDS.get(name).copied()
                }

                #[inline(always)]
                pub fn user_op_by_id(id: u16) -> Option<&'static str> {
                    USER_OPS.get(id as usize).copied()
                }
            }

            struct Instruction;

            impl fugue_lifter_runtime::ConstructorResolver for Instruction {
                const ADDRESS_SIZE: usize = ADDRESS_SIZE;
                const DEFAULT_SPACE: u8 = DEFAULT_SPACE;
                const UNIQUE_SPACE: u8 = UNIQUE_SPACE;

                #[inline(always)]
                fn resolve(
                    state: &mut fugue_lifter_runtime::LiftingContextState,
                ) -> Option<&'static fugue_lifter_runtime::Constructor> {
                    resolve_constructor(state)
                }

                #[inline(always)]
                fn resolve_upper_bound(space: u8) -> u64 {
                    SPACE_UPPER_BOUND[space as usize]
                }

                #[inline(always)]
                fn resolve_word_size(space: u8) -> usize {
                    SPACE_WORD_SIZE[space as usize]
                }

                #[inline(always)]
                fn resolve_location_offset(
                    unique_offset: u64,
                    space: u8,
                    offset: u64,
                    size: u16,
                ) -> u64 {
                    #space_match
                }
            }

            #[inline(always)]
            pub fn resolve_constructor(
                state: &mut fugue_lifter_runtime::LiftingContextState,
            ) -> Option<&'static fugue_lifter_runtime::Constructor> {
                unsafe {
                    let ctor = SubTable0In0::resolve(state)?;
                    ctor.resolve_operands::<Instruction>(state)?;
                    Some(ctor)
                }
            }

            #[inline(always)]
            pub fn resolve_state(
                state: &mut fugue_lifter_runtime::LiftingContextState,
            ) -> Option<&'static fugue_lifter_runtime::Constructor> {
                unsafe {
                    let ctor = resolve_constructor(state)?;
                    ctor.resolve_handles::<Instruction>(state)?;
                    state.inputs.input.base_state();
                    state.apply_commits::<Instruction>();
                    Some(ctor)
                }
            }

            #[inline]
            pub fn resolve(
                address: u64,
                bytes: &[u8],
                context: &mut fugue_lifter_runtime::LiftingContext,
                apply_commits: bool,
            ) -> Option<usize> {
                unsafe {
                    let mut nop_issued = Vec::with_capacity(0);
                    let mut state = context.state_for(address, bytes, &mut nop_issued)?;

                    let ctor = resolve_constructor(&mut state)?;

                    let buffer_limit = bytes.len();
                    let length = state.len();

                    if length == 0 || length > buffer_limit {
                        return None;
                    }

                    if apply_commits {
                        ctor.resolve_handles::<Instruction>(&mut state)?;
                    }

                    state.inputs.input.base_state();

                    if apply_commits {
                        state.apply_commits::<Instruction>();
                    }

                    Some(length)
                }
            }

            #[inline]
            pub(crate) fn disassemble_to_string(
                address: u64,
                bytes: &[u8],
                context: &mut fugue_lifter_runtime::LiftingContext,
                output: &mut String,
            ) -> Option<usize> {
                disassemble(address, bytes, context, output).unwrap()
            }

            #[inline]
            pub fn disassemble<W: std::fmt::Write>(
                address: u64,
                bytes: &[u8],
                context: &mut fugue_lifter_runtime::LiftingContext,
                writer: &mut W,
            ) -> Result<Option<usize>, std::fmt::Error> {
                unsafe {
                    let mut nop_issued = Vec::with_capacity(0);

                    let Some(mut state) = context.state_for(address, bytes, &mut nop_issued) else {
                        return Ok(None);
                    };

                    let Some(ctor) = resolve_constructor(&mut state) else {
                        return Ok(None);
                    };

                    let buffer_limit = bytes.len();
                    let length = state.len();

                    if length == 0 || length > buffer_limit {
                        return Ok(None);
                    }

                    if ctor.resolve_handles::<Instruction>(&mut state).is_none() {
                        return Ok(None);
                    }

                    state.inputs.input.base_state();

                    state.apply_commits::<Instruction>();

                    state.format::<Instruction, _>(writer)?;

                    Ok(Some(length))
                }
            }

            #[inline]
            pub fn lift(
                address: u64,
                bytes: &[u8],
                context: &mut fugue_lifter_runtime::LiftingContext,
                issued: &mut Vec<fugue_lifter_runtime::pcode::PCodeOp>,
            ) -> Option<usize> {
                unsafe {
                    let mut state = context.state_for(address, bytes, issued)?;
                    let buffer_limit = bytes.len();

                    resolve_state(&mut state)?;

                    let length = state.len();

                    if length == 0 || length > buffer_limit {
                        return None;
                    }

                    let delay_slot_bytes = state.delay_slot_length();

                    if delay_slot_bytes == 0 {
                        state.emit::<Instruction>()?;
                        return Some(state.len());
                    }

                    let mut fall_offset = state.len();
                    let mut delay_count = 0usize;
                    let mut index = 0usize;

                    loop {
                        let address = address + fall_offset as u64;
                        let bytes = bytes.get(fall_offset..)?;

                        // NOTE: this does not ensure we have the context configured for lifting;
                        // we therefore need to directly initialise the first input.
                        let mut dstate = state.nth_delay_slot(index)?;

                        dstate.inputs.initialise(address, bytes);

                        resolve_state(&mut dstate)?;

                        let length = dstate.len();

                        if length == 0 || length > (buffer_limit - fall_offset) {
                            return None;
                        }

                        fall_offset += length;
                        delay_count += length;

                        if delay_count >= delay_slot_bytes {
                            break;
                        }

                        index += 1;
                    }

                    state.emit::<Instruction>()?;

                    Some(fall_offset)
                }
            }

            #[inline]
            pub fn lifter(ninputs: usize) -> fugue_lifter_runtime::LiftingContext {
                lifter_with(ninputs, default_context())
            }

            #[inline]
            pub fn lifter_with(
                ninputs: usize,
                context: fugue_lifter_runtime::ContextDatabase,
            ) -> fugue_lifter_runtime::LiftingContext {
                fugue_lifter_runtime::LiftingContext::new(ninputs, context, UNIQUE_MASK)
            }

            #[inline]
            #[allow(unused_mut)]
            pub fn default_context() -> fugue_lifter_runtime::ContextDatabase {
                let mut context =
                    fugue_lifter_runtime::ContextDatabase::new(ADDRESS_UPPER_BOUND);

                #(#context_variable_registrations)*

                context
            }

            struct L;
            impl fugue_lifter_runtime::lifter::LanguageImpl for L {
                const ID: &'static str = LANGUAGE_ID;

                const LITTLE_ENDIAN: bool = #little_endian;
                const VARIANT: &'static str = #variant;

                const ADDRESS_ALIGNMENT: usize = ADDRESS_ALIGNMENT;
                const ADDRESS_BITS: u32 = ADDRESS_BITS;
                const ADDRESS_SIZE: usize = ADDRESS_SIZE;
                const ADDRESS_UPPER_BOUND: u64 = ADDRESS_UPPER_BOUND;

                const CONSTANT_SPACE: u8 = CONSTANT_SPACE;
                const DEFAULT_SPACE: u8 = DEFAULT_SPACE;

                const REGISTER_SPACE: u8 = REGISTER_SPACE;
                const REGISTER_SPACE_SIZE: usize = REGISTER_SPACE_SIZE;

                const UNIQUE_MASK: u64 = UNIQUE_MASK;
                const UNIQUE_SPACE: u8 = UNIQUE_SPACE;
                const UNIQUE_SPACE_SIZE: usize = UNIQUE_SPACE_SIZE;

                const SPACE_WORD_SIZES: &'static [usize] = &SPACE_WORD_SIZE;
                const SPACE_UPPER_BOUNDS: &'static [u64] = &SPACE_UPPER_BOUND;
                const SPACE_BY_NAME: fn(&str) -> Option<u8> = space::space_by_name;
                const SPACE_NAME: fn(u8) -> Option<&'static str> = space::space_name;

                const CONTEXT_VARIABLE_BY_NAME: fn(&str) -> Option<fugue_lifter_runtime::context::ContextBitRange> = context::context_variable_by_name;

                const REGISTER_BY_NAME: fn(&str) -> Option<fugue_lifter_runtime::pcode::Varnode> = register::register_by_name;
                const REGISTER_NAME: fn(&fugue_lifter_runtime::pcode::Varnode) -> Option<&'static str> = register::register_name;

                const USER_OP_BY_NAME: fn(&str) -> Option<u16> = user_op::user_op_by_name;
                const USER_OP_BY_ID: fn(u16) -> Option<&'static str> = user_op::user_op_by_id;

                const RESOLVE: fn(u64, &[u8], &mut fugue_lifter_runtime::pcode::LiftingContext, bool) -> Option<usize> = resolve;
                const DISASSEMBLE: fn(u64, &[u8], &mut fugue_lifter_runtime::pcode::LiftingContext, &mut String) -> Option<usize> = disassemble_to_string;
                const LIFT: fn(u64, &[u8], &mut fugue_lifter_runtime::pcode::LiftingContext, &mut Vec<fugue_lifter_runtime::pcode::PCodeOp>) -> Option<usize> = lift;
            }
            pub static LANGUAGE: &'static fugue_lifter_runtime::lifter::Language = &fugue_lifter_runtime::lifter::Language::new::<L>();
        });

        tokens.append_all(&self.symbols);
    }
}
