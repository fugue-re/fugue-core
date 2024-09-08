use std::mem::size_of;

use fugue_ir::disassembly::construct::ConstTpl;
use fugue_ir::disassembly::construct::HandleKind;
use fugue_ir::disassembly::construct::HandleTpl;
use fugue_ir::disassembly::symbol::sub_table::Context;
use fugue_ir::disassembly::symbol::sub_table::{
    ContextPattern, DecisionPair, DisjointPattern, InstructionPattern,
};
use fugue_ir::disassembly::symbol::{Constructor, DecisionNode, Symbol};
use fugue_ir::disassembly::PatternExpression;
use fugue_ir::Translator;

// use itertools::Itertools as _;
use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote, ToTokens, TokenStreamExt};
use syn::Ident;

use crate::LifterGeneratorError;

pub struct LifterGenerator<'a> {
    symbols: Vec<TokenStream>,
    lifter: TokenStream,
    translator: &'a Translator,
}

impl<'a> LifterGenerator<'a> {
    pub fn new(translator: &'a Translator) -> Result<Self, LifterGeneratorError> {
        let mut slf = Self {
            symbols: Vec::new(),
            lifter: Default::default(),
            translator,
        };

        slf.build_symbols()?;
        slf.build_lifter()?;

        Ok(slf)
    }

    pub fn build_lifter(&mut self) -> Result<(), LifterGeneratorError> {
        self.lifter = quote! {
            pub struct Lifter;

            impl Lifter {
                pub fn lift(addr: u64, bytes: &[u8], context: &mut ::fugue_ir::ContextDatabase) {
                    todo!()
                }
            }
        };

        Ok(())
    }

    pub fn build_symbols(&mut self) -> Result<(), LifterGeneratorError> {
        let symtab = self.translator.symbol_table();
        // let mut variants = Vec::new();

        for symbol in symtab.symbols().iter() {
            match symbol {
                /*
                Symbol::Operand {
                    id, scope, name, handle_index, offset, base, min_length, subsym_id, is_code, local_expr, def_expr
                } => {
                   self.symbols.push(self.generate_operand(id, scope, handle_index, offset, base, min_length, subsym_id, is_code, local_expr, def_expr));
                }
                */
                Symbol::Subtable {
                    id,
                    scope,
                    name,
                    constructors,
                    decision_tree,
                } => {
                    self.symbols.push(self.generate_subtable(
                        *id,
                        *scope,
                        name,
                        constructors,
                        decision_tree,
                        // &mut variants,
                    )?);
                }
                _ => (),
            }
        }

        // self.ctor_names = variants;

        Ok(())
    }

    pub fn generate_handle_resolver(&self, symbol: &Symbol) -> TokenStream {
        match symbol {
            Symbol::Epsilon { .. } => {
                quote! {
                    fugue_lifter::utils::FixedHandle {
                        space: 0,
                        ..Default::default()
                    }
                }
            }
            Symbol::Name { pattern_value, .. } | Symbol::Value { pattern_value, .. } => {
                let expr = self.generate_pattern_resolver(pattern_value);
                quote! {
                    fugue_lifter::utils::FixedHandle {
                        space: 0,
                        offset_offset: #expr as u64,
                        ..Default::default()
                    }
                }
            }
            Symbol::Varnode {
                space,
                offset,
                size,
                ..
            } => {
                let space = space.index() as u8;
                let offset = *offset;
                let size = *size as u8;

                quote! {
                    fugue_lifter::utils::FixedHandle {
                        space: #space,
                        size: #size,
                        offset_offset: #offset,
                        ..Default::default()
                    }
                }
            }
            Symbol::Operand { handle_index, .. } => {
                let opid = *handle_index as u8;
                quote! {
                    {
                        let opid = input.context.constructors[point as usize].operands + #opid;
                        input.context.constructors[opid as usize]?
                    }
                }
            }
            Symbol::Start { .. } => {
                let space = self.translator.manager().default_space_ref();
                let space_id = space.id().index() as u8;
                let size = space.address_size() as u8;
                quote! {
                    fugue_lifter::utils::FixedHandle {
                        space: #space_id,
                        size: #size,
                        offset_offset: input.context.address,
                        ..Default::default()
                    }
                }
            }
            Symbol::End { .. } => {
                let space = self.translator.manager().default_space_ref();
                let space_id = space.id().index() as u8;
                let size = space.address_size() as u8;
                quote! {
                    fugue_lifter::utils::FixedHandle {
                        space: #space_id,
                        size: #size,
                        offset_offset: input.next_address(),
                        ..Default::default()
                    }
                }
            }
            Symbol::Next2 { .. } => {
                quote! { unimplemented!() }
            }
            Symbol::VarnodeList {
                pattern_value,
                varnode_table,
                ..
            } => {
                // NOTE: it's possible to sometimes compress such lists; for example,
                // we often see something like:
                //
                // match index {
                //     0usize => FixedHandle { ... },
                //     1usize => FixedHandle { ... },
                //     2usize => FixedHandle { ... },
                //     3usize => FixedHandle { ... },
                //     ...
                // }
                //
                // Where the FixedHandle differs in only a single field, and the fields
                // set are all constant and known at build time. We could compress these
                // cases, by matching for the field assignment, or, if possible, compute
                // the target value from the index.
                //

                let index = self.generate_pattern_resolver(pattern_value);
                let cases = varnode_table.iter().enumerate().map(|(i, symid)| {
                    if let Some(symid) = symid {
                        let sym = self.translator.symbol_table().unchecked_symbol(*symid);
                        let value = self.generate_handle_resolver(sym);
                        quote! { #i => { #value } }
                    } else {
                        quote! { #i => { return None; } }
                    }
                });

                quote! {
                    match #index as usize {
                        #(#cases,)*
                        _ => { return None },
                    }
                }
            }
            Symbol::ValueMap {
                pattern_value,
                value_table,
                ..
            } => {
                let index = self.generate_pattern_resolver(pattern_value);
                let cases = value_table.iter().enumerate().map(|(i, symid)| {
                    if *symid != 0xbadbeef {
                        let sym = self.translator.symbol_table().unchecked_symbol(*symid as _);
                        let value = self.generate_handle_resolver(sym);
                        quote! { #i => { #value as u64 } }
                    } else {
                        quote! { #i => { return None; } }
                    }
                });

                quote! {
                    fugue_lifter::utils::FixedHandle {
                        space: 0,
                        offset_offset: match #index as usize {
                            #(#cases,)*
                            _ => { return None },
                        },
                        ..Default::default()
                    }
                }
            }
            _ => TokenStream::new(),
        }
    }

    pub fn generate_pattern_resolver(&self, pattern: &PatternExpression) -> TokenStream {
        match pattern {
            PatternExpression::Constant { value } => quote! { #value },
            PatternExpression::StartInstruction => quote! { (input.address() as i64) },
            PatternExpression::EndInstruction => {
                quote! { (input.next_address() as i64) }
            }
            PatternExpression::Next2Instruction => {
                quote! { unsupported!("next2_inst is not supported") }
            }
            PatternExpression::TokenField {
                big_endian,
                sign_bit,
                bit_start,
                bit_end,
                byte_start,
                byte_end,
                shift,
            } => {
                let size = byte_end - byte_start + 1;
                let mut start = *byte_start as isize;
                let mut tsize = size as isize;

                let mut parts = Vec::new();
                let access_size = size_of::<u32>();
                let access_bits = 8 * size_of::<u32>() as u32;

                while tsize >= size_of::<u32>() as isize {
                    let start_val = start as usize;
                    parts.push(quote! {
                        res = (((res as u64) << #access_bits) | (input.instruction_bytes(#start_val, #access_size)? as u64)) as i64;
                    });
                    start += size_of::<u32>() as isize;
                    tsize = (*byte_end as isize) - start + 1;
                }

                if tsize > 0 {
                    let start_val = start as usize;
                    let tsize = tsize as usize;
                    let shift = 8 * tsize as u32;
                    parts.push(quote! {
                        res = (((res as u64) << #shift) | (input.instruction_bytes(#start_val, #tsize)? as u64)) as i64;
                    });
                }

                if !*big_endian {
                    parts.push(quote! {
                        res = fugue_lifter::utils::byte_swap(res, #size);
                    });
                }

                parts.push(quote! {
                    res = res.checked_shr(#shift).unwrap_or(if res < 0 { -1 } else { 0 });
                });

                let range = bit_end - bit_start;

                parts.push(if *sign_bit {
                    quote! { fugue_lifter::utils::sign_extend(res, #range) }
                } else {
                    quote! { fugue_lifter::utils::zero_extend(res, #range) }
                });

                quote! {
                    {
                        let mut res = 0i64;
                        #(#parts)*
                    }
                }
            }
            PatternExpression::ContextField {
                sign_bit,
                bit_start,
                bit_end,
                byte_start,
                byte_end,
                shift,
            } => {
                let mut size = (*byte_end as isize) - (*byte_start as isize) + 1;
                let mut start = *byte_start as isize;

                let mut parts = Vec::new();
                let access_size = size_of::<u32>();
                let access_bits = 8 * size_of::<u32>() as u32;

                while size >= size_of::<u32>() as isize {
                    let start_val = start as usize;
                    parts.push(quote! {
                        res = (((res as u64) << #access_bits) | (input.context_bytes(#start_val, #access_size) as u64)) as i64;
                    });
                    start += size_of::<u32>() as isize;
                    size = (*byte_end as isize) - start + 1;
                }

                if size > 0 {
                    let start_val = start as usize;
                    let size = size as usize;
                    let shift = 8 * size as u32;
                    parts.push(quote! {
                        res = (((res as u64) << #shift) | (input.context_bytes(#start_val, #size) as u64)) as i64;
                    });
                }

                parts.push(quote! {
                    res = res.checked_shr(#shift).unwrap_or(if res < 0 { -1 } else { 0 });
                });

                let range = bit_end - bit_start;

                parts.push(if *sign_bit {
                    quote! { fugue_lifter::utils::sign_extend(res, #range) }
                } else {
                    quote! { fugue_lifter::utils::zero_extend(res, #range) }
                });

                quote! {
                    {
                        let mut res = 0i64;
                        #(#parts)*
                    }
                }
            }
            PatternExpression::Operand {
                index,
                table_id,
                constructor_id,
            } => {
                let symbols = self.translator.symbol_table();
                let table = symbols.unchecked_symbol(*table_id);
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
                } = symbols.unchecked_symbol(ctor.operand(*index))
                else {
                    unreachable!("this state should not be reachable");
                };

                let pexpr = if let Some(def_expr) = def_expr.as_ref() {
                    def_expr
                } else if let Some(subsym_id) = subsym_id.as_ref() {
                    let sym = symbols.unchecked_symbol(*subsym_id);
                    sym.pattern_value()
                } else {
                    return quote! { 0i64 };
                };

                let index = *index;
                let symbol = ctor.operand(index);
                let operand = self.translator.symbol_table().unchecked_symbol(symbol);

                let (ctor_id1, ctor_id2) = ctor.id();
                let ctor_id = (ctor_id1 as u32 & 0xffff) << 16 | (ctor_id2 as u32 & 0xffff);
                let ctor_vname = Self::ctor_vname(*table_id, *scope, *constructor_id);
                let pattern_value = self.generate_pattern_resolver(&pexpr);

                let rel_offset = operand.relative_offset() as u8;
                let offset = if operand.offset_base().is_none() {
                    quote! {
                        point.offset + #rel_offset
                    }
                } else {
                    quote! {
                        input.context.constructors[point.operands as usize + #index].offset
                    }
                };

                quote! {
                    {
                        let operand_value = |input: &mut fugue_lifter::utils::ParserInput| -> Option<i64> {
                            let mut cur_depth = input.depth;
                            let mut point = &input.context.constructors[input.point as usize];

                            while point.constructor.map(|ctor| ctor.id) != Some(#ctor_id) {
                                if cur_depth <= 0 {
                                    // preserve old and init new state
                                    let old_point = input.point;
                                    let old_depth = std::mem::take(&mut input.depth);
                                    let old_breadcrumb = std::mem::replace(&mut input.breadcrumb, [0u8; fugue_lifter::utils::input::BREADCRUMBS]);

                                    input.point = input.context.alloc;
                                    {
                                        let state = &mut input.context.constructors[input.point as usize];

                                        state.constructor = Some(#ctor_vname);
                                        state.handle = None;
                                        state.parent = fugue_lifter::utils::input::INVALID_HANDLE;
                                        state.operands = fugue_lifter::utils::input::INVALID_HANDLE;
                                        state.offset = 0;
                                        state.length = 0;
                                    }

                                    // compute the value in the modified context
                                    let value = { #pattern_value };

                                    // restore old state
                                    {
                                        let state = &mut input.context.constructors[input.point as usize];

                                        state.constructor = None;
                                        state.handle = None;
                                        state.parent = fugue_lifter::utils::input::INVALID_HANDLE;
                                        state.operands = fugue_lifter::utils::input::INVALID_HANDLE;
                                        state.offset = 0;
                                        state.length = 0;
                                    }

                                    input.point = old_point;
                                    input.depth = old_depth;
                                    input.breadcrumb = old_breadcrumb;

                                    return Some(value);
                                }

                                cur_depth -= 1;
                                point = &input.context.constructors[point.parent as usize];
                            }

                            // if we reach here, we've resolved the ctor in the current tree
                            let offset = #offset;
                            let length = point.length;

                            // preserve old and init new state
                            let old_point = input.point;
                            let old_depth = std::mem::take(&mut input.depth);
                            let old_breadcrumb = std::mem::replace(&mut input.breadcrumb, [0u8; fugue_lifter::utils::input::BREADCRUMBS]);

                            input.point = input.context.alloc;
                            {
                                let state = &mut input.context.constructors[input.point as usize];

                                state.constructor = Some(#ctor_vname);
                                state.handle = None;
                                state.parent = fugue_lifter::utils::input::INVALID_HANDLE;
                                state.operands = fugue_lifter::utils::input::INVALID_HANDLE;
                                state.offset = offset;
                                state.length = length;
                            }

                            // compute the value in the modified context
                            let value = { #pattern_value };

                            // restore old state
                            {
                                let state = &mut input.context.constructors[input.point as usize];

                                state.constructor = None;
                                state.handle = None;
                                state.parent = fugue_lifter::utils::input::INVALID_HANDLE;
                                state.operands = fugue_lifter::utils::input::INVALID_HANDLE;
                                state.offset = 0;
                                state.length = 0;
                            }

                            input.point = old_point;
                            input.depth = old_depth;
                            input.breadcrumb = old_breadcrumb;

                            Some(value)
                        };
                        operand_value(input)?
                    }
                }
            }
            PatternExpression::And(lhs, rhs) => {
                let lhs = self.generate_pattern_resolver(lhs);
                let rhs = self.generate_pattern_resolver(rhs);
                quote! { (#lhs & #rhs) }
            }
            PatternExpression::Or(lhs, rhs) => {
                let lhs = self.generate_pattern_resolver(lhs);
                let rhs = self.generate_pattern_resolver(rhs);
                quote! { (#lhs | #rhs) }
            }
            PatternExpression::Xor(lhs, rhs) => {
                let lhs = self.generate_pattern_resolver(lhs);
                let rhs = self.generate_pattern_resolver(rhs);
                quote! { (#lhs ^ #rhs) }
            }
            PatternExpression::Plus(lhs, rhs) => {
                let lhs = self.generate_pattern_resolver(lhs);
                let rhs = self.generate_pattern_resolver(rhs);
                quote! { #lhs.wrapping_add(#rhs) }
            }
            PatternExpression::Sub(lhs, rhs) => {
                let lhs = self.generate_pattern_resolver(lhs);
                let rhs = self.generate_pattern_resolver(rhs);
                quote! { #lhs.wrapping_sub(#rhs) }
            }
            PatternExpression::Div(lhs, rhs) => {
                let lhs = self.generate_pattern_resolver(lhs);
                let rhs = self.generate_pattern_resolver(rhs);
                quote! { { let rhs = #rhs; if rhs == 0 { return None; } else { #lhs.wrapping_div(rhs) } } }
            }
            PatternExpression::Mult(lhs, rhs) => {
                let lhs = self.generate_pattern_resolver(lhs);
                let rhs = self.generate_pattern_resolver(rhs);
                quote! { #lhs.wrapping_mul(#rhs) }
            }
            PatternExpression::LeftShift(lhs, rhs) => {
                let lhs = self.generate_pattern_resolver(lhs);
                let rhs = self.generate_pattern_resolver(rhs);
                quote! { #lhs.checked_shl(#rhs as u8 as u32).unwrap_or(0) }
            }
            PatternExpression::RightShift(lhs, rhs) => {
                let lhs = self.generate_pattern_resolver(lhs);
                let rhs = self.generate_pattern_resolver(rhs);
                quote! { #lhs.checked_shr(#rhs as u8 as u32).unwrap_or(if #lhs < 0 { -1 } else { 0 }) }
            }
            PatternExpression::Not(val) => {
                let val = self.generate_pattern_resolver(val);
                quote! { -(#val) }
            }
            PatternExpression::Minus(val) => {
                let val = self.generate_pattern_resolver(val);
                quote! { !(#val) }
            }
        }
    }

    pub fn generate_constructor_operand_resolvers(
        &self,
        id: usize,
        scope: usize,
        cid: usize,
        ctor: &Constructor,
    ) -> (Vec<TokenStream>, Vec<TokenStream>) {
        let mut helpers = Vec::new();
        let mut operands = Vec::new();

        for oid in 0..ctor.operand_count() {
            let index = ctor.operand(oid);
            let operand = self.translator.symbol_table().unchecked_symbol(index);

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
                operand.defining_symbol(self.translator.symbol_table())
            {
                match tsym {
                    Symbol::Subtable { id, scope, .. } => {
                        // The subtable to perform resolution
                        let stname = format_ident!("SubTable{id}In{scope}");
                        let resolver = quote! { fugue_lifter::utils::OperandResolver::Constructor(<#stname>::resolve) };
                        let handle_resolver = quote! { None };

                        (resolver, handle_resolver)
                    }
                    Symbol::ValueMap {
                        table_is_filled,
                        pattern_value,
                        value_table,
                        ..
                    } => {
                        let resolver = if !*table_is_filled {
                            let bad_indices = value_table
                                .iter()
                                .enumerate()
                                .filter_map(|(i, v)| if *v == 0xbadbeef { Some(i) } else { None });

                            let pattern_resolver = self.generate_pattern_resolver(pattern_value);
                            let limit = value_table.len();

                            let ctor_opnd_resolver =
                                format_ident!("operand_resolver_{id}_{scope}_{cid}_{oid}");

                            helpers.push(quote! {
                                #[inline]
                                fn #ctor_opnd_resolver(input: &mut fugue_lifter::utils::ParserInput) -> Option<()> {
                                    let index = #pattern_resolver as usize;
                                    if index >= #limit || [#(#bad_indices),*].contains(&index) {
                                        None
                                    } else {
                                        Some(())
                                    }
                                }
                            });

                            quote! { fugue_lifter::utils::OperandResolver::Filter(#ctor_opnd_resolver) }
                        } else {
                            quote! { fugue_lifter::utils::OperandResolver::None }
                        };

                        let ctor_opnd_handle_resolver =
                            format_ident!("operand_handle_resolver_{id}_{scope}_{cid}_{oid}");

                        let handle_resolver = self.generate_handle_resolver(tsym);

                        helpers.push(quote! {
                            #[inline]
                            fn #ctor_opnd_handle_resolver(input: &mut fugue_lifter::utils::ParserInput) -> Option<()> {
                                let handle = #handle_resolver;
                                input.set_parent_handle(handle);
                                Some(())
                            }
                        });

                        let handle_resolver = quote! {
                            Some(#ctor_opnd_handle_resolver)
                        };

                        (resolver, handle_resolver)
                    }
                    Symbol::VarnodeList {
                        table_is_filled,
                        pattern_value,
                        varnode_table,
                        ..
                    } => {
                        let resolver = if !*table_is_filled {
                            let bad_indices = varnode_table
                                .iter()
                                .enumerate()
                                .filter_map(|(i, v)| if v.is_none() { Some(i) } else { None });

                            let pattern_resolver = self.generate_pattern_resolver(pattern_value);
                            let limit = varnode_table.len();

                            let ctor_opnd_resolver =
                                format_ident!("operand_resolver_{id}_{scope}_{cid}_{oid}");

                            helpers.push(quote! {
                                #[inline]
                                fn #ctor_opnd_resolver(input: &mut fugue_lifter::utils::ParserInput) -> Option<()> {
                                    let index = #pattern_resolver as usize;
                                    if index >= #limit || [#(#bad_indices),*].contains(&index) {
                                        None
                                    } else {
                                        Some(())
                                    }
                                }
                            });

                            quote! { fugue_lifter::utils::OperandResolver::Filter(#ctor_opnd_resolver) }
                        } else {
                            quote! { fugue_lifter::utils::OperandResolver::None }
                        };

                        let ctor_opnd_handle_resolver =
                            format_ident!("operand_handle_resolver_{id}_{scope}_{cid}_{oid}");

                        let handle_resolver = self.generate_handle_resolver(tsym);

                        helpers.push(quote! {
                            #[inline]
                            fn #ctor_opnd_handle_resolver(input: &mut fugue_lifter::utils::ParserInput) -> Option<()> {
                                let handle = #handle_resolver;
                                input.set_parent_handle(handle);
                                Some(())
                            }
                        });

                        let handle_resolver = quote! {
                            Some(#ctor_opnd_handle_resolver)
                        };

                        (resolver, handle_resolver)
                    }
                    Symbol::Name {
                        table_is_filled,
                        pattern_value,
                        name_table,
                        ..
                    } => {
                        let resolver = if !*table_is_filled {
                            let bad_indices = name_table.iter().enumerate().filter_map(|(i, v)| {
                                if v == "\t" {
                                    Some(i)
                                } else {
                                    None
                                }
                            });

                            let pattern_resolver = self.generate_pattern_resolver(pattern_value);
                            let limit = name_table.len();

                            let ctor_opnd_resolver =
                                format_ident!("operand_resolver_{id}_{scope}_{cid}_{oid}");

                            helpers.push(quote! {
                                #[inline]
                                fn #ctor_opnd_resolver(input: &mut fugue_lifter::utils::ParserInput) -> Option<()> {
                                    let index = #pattern_resolver as usize;
                                    if index >= #limit || [#(#bad_indices),*].contains(&index) {
                                        None
                                    } else {
                                        Some(())
                                    }
                                }
                            });

                            quote! { fugue_lifter::utils::OperandResolver::Filter(#ctor_opnd_resolver) }
                        } else {
                            quote! { fugue_lifter::utils::OperandResolver::None }
                        };

                        let ctor_opnd_handle_resolver =
                            format_ident!("operand_handle_resolver_{id}_{scope}_{cid}_{oid}");

                        let handle_resolver = self.generate_handle_resolver(tsym);

                        helpers.push(quote! {
                            #[inline]
                            fn #ctor_opnd_handle_resolver(input: &mut fugue_lifter::utils::ParserInput) -> Option<()> {
                                let handle = #handle_resolver;
                                input.set_parent_handle(handle);
                                Some(())
                            }
                        });

                        let handle_resolver = quote! {
                            Some(#ctor_opnd_handle_resolver)
                        };

                        (resolver, handle_resolver)
                    }
                    _ => {
                        let ctor_opnd_handle_resolver =
                            format_ident!("operand_handle_resolver_{id}_{scope}_{cid}_{oid}");

                        let handle_resolver = self.generate_handle_resolver(tsym);

                        helpers.push(quote! {
                            #[inline]
                            fn #ctor_opnd_handle_resolver(input: &mut fugue_lifter::utils::ParserInput) -> Option<()> {
                                let handle = #handle_resolver;
                                input.set_parent_handle(handle);
                                Some(())
                            }
                        });

                        let handle_resolver = quote! {
                            Some(#ctor_opnd_handle_resolver)
                        };

                        let resolver = quote! { fugue_lifter::utils::OperandResolver::None };

                        (resolver, handle_resolver)
                    }
                }
            } else {
                let pexp = operand.defining_expression().unwrap();
                let value = self.generate_pattern_resolver(pexp);

                let ctor_opnd_handle_resolver =
                    format_ident!("operand_handle_resolver_{id}_{scope}_{cid}_{oid}");

                helpers.push(quote! {
                    #[inline]
                    fn #ctor_opnd_handle_resolver(input: &mut fugue_lifter::utils::ParserInput) -> Option<()> {
                        let offset = #value as u64;
                        if let Some(handle) = input.parent_handle_mut() {
                            handle.space = 0;
                            handle.offset_space = fugue_lifter::utils::input::INVALID_HANDLE;
                            handle.offset_offset = offset;
                            handle.size = 0;
                        } else {
                            input.set_parent_handle(fugue_lifter::utils::FixedHandle {
                                space: 0,
                                offset_offset: offset,
                                ..Default::default()
                            });
                        }
                        Some(())
                    }
                });

                let handle_resolver = quote! {
                    Some(#ctor_opnd_handle_resolver)
                };

                let resolver = quote! { fugue_lifter::utils::OperandResolver::None };

                (resolver, handle_resolver)
            };

            operands.push(quote! {
                fugue_lifter::utils::Operand {
                    resolver: #resolver,
                    handle_resolver: #handle_resolver,
                    offset_base: #offset_base,
                    offset_rela: #offset_rela,
                    minimum_length: #minimum_length,
                }
            });
        }

        (helpers, operands)
    }

    pub fn generate_constructor_context_actions(
        &self,
        id: usize,
        scope: usize,
        cid: usize,
        ctor: &Constructor,
    ) -> (TokenStream, TokenStream) {
        let mut pre_actions = Vec::new();
        let mut post_actions = Vec::new();
        let mut post_context_extractors = Vec::new();

        for action in ctor.context().iter() {
            match action {
                Context::Operator {
                    num,
                    shift,
                    mask,
                    pattern_value,
                } => {
                    let num = *num;
                    let shift = *shift;
                    let mask = *mask;
                    let value = self.generate_pattern_resolver(pattern_value);

                    pre_actions.push(quote! {
                        let value = (#value as u32) << #shift;
                        input.set_context_word(#num, value, #mask);
                    });
                }
                Context::Commit {
                    symbol_id,
                    num,
                    mask,
                    flow,
                } => {
                    let index = post_actions.len();
                    let symbol = self.translator.symbol_table().unchecked_symbol(*symbol_id);
                    let handle = if let Symbol::Operand { handle_index, .. } = &symbol {
                        let opid = *handle_index as u8;
                        quote! {
                            let id = input.context.constructors[point as usize].operands + #opid;
                            input.context.constructors[id as usize]
                                .handle
                                .as_ref()
                                .map(|handle| (handle.offset_space, handle.offset_offset))
                                .unwrap_or_default()
                        }
                    } else {
                        let resolver = self.generate_handle_resolver(symbol);
                        quote! {
                            let handle = #resolver;
                            (handle.space, handle.offset_offset)
                        }
                    };

                    let space = self.translator.manager().default_space_ref();
                    let word_size = space.word_size() as u64;

                    let space_fix = quote! {
                        let (space, mut offset) = { #handle };
                        if space == 0 {
                            offset = offset * #word_size;
                        }
                    };

                    let number = *num;
                    let mask = *mask;

                    let flow = if *flow {
                        quote! {
                            context
                                .set_context_change_point(
                                    input.context.address,
                                    offset,
                                    #number,
                                    #mask,
                                    commit.values[#index],
                                );
                        }
                    } else {
                        // we wrap the address with respect to the space
                        let highest = space.highest_offset();
                        quote! {
                            let noffset = fugue_lifter::utils::wrap_offset(#highest, offset.wrapping_add(1u64));
                            if noffset < offset {
                                context
                                    .set_context_change_point(
                                        input.context.address,
                                        offset,
                                        #number,
                                        #mask,
                                        commit.values[#index],
                                    );
                            } else {
                                context
                                    .set_context_region(
                                        input.context.address,
                                        Some(noffset),
                                        #number,
                                        #mask,
                                        commit.values[#index],
                                    );
                            }
                        }
                    };

                    post_context_extractors.push(quote! { (input.context.context[#num] & #mask) });
                    post_actions.push(quote! {
                        {
                            #space_fix
                            #flow
                        }
                    });
                }
            }
        }

        if post_actions.is_empty() && pre_actions.is_empty() {
            return (TokenStream::new(), quote! { None });
        }

        let (post_field, post_fcn) = if !post_actions.is_empty() {
            let ctor_post_apply_context = format_ident!("apply_post_context_{id}_{scope}_{cid}");
            let post_fcn = quote! {
                #[inline]
                fn #ctor_post_apply_context(
                    input: &fugue_lifter::utils::ParserInput,
                    context: &mut fugue_lifter::utils::ContextDatabase,
                    commit: &fugue_lifter::utils::ContextCommit,
                ) -> Option<()> {
                    let point = commit.point;

                    #(#post_actions);*

                    Some(())
                }
            };
            (Some(ctor_post_apply_context), post_fcn)
        } else {
            (None, TokenStream::new())
        };

        let ctor_pre_apply_context = format_ident!("apply_pre_context_{id}_{scope}_{cid}");

        let post_register = post_field.as_ref().map(|fcn| {
            quote! {
                let commit = fugue_lifter::utils::ContextCommit {
                    applier: #fcn,
                    point: input.point,
                    values: [
                        #(#post_context_extractors),*
                    ].into_iter().collect(),
                };
                input.register_context_commit(commit);
            }
        });

        let pre_fcn = quote! {
            #[inline]
            fn #ctor_pre_apply_context(input: &mut fugue_lifter::utils::ParserInput) -> Option<()> {
                #(#pre_actions)*

                #post_register

                Some(())
            }
        };

        let fcns = quote! {
            #post_fcn
            #pre_fcn
        };

        (fcns, quote! { Some(#ctor_pre_apply_context) })
    }

    pub fn generate_const_template(&self, tmpl: &ConstTpl) -> TokenStream {
        match tmpl {
            ConstTpl::Start => quote! { input.context.address() },
            ConstTpl::Next => quote! { input.context.next_address() },
            ConstTpl::Next2 => quote! { unimplemented!("next2 not supported") },
            ConstTpl::CurrentSpaceSize => {
                let size = self.translator.manager().default_space_ref().address_size() as u64;
                quote! { #size }
            }
            ConstTpl::CurrentSpace => {
                let index = self.translator.manager().default_space_ref().index() as u64;
                quote! { #index }
            }
            ConstTpl::Relative(value) | ConstTpl::Real(value) => {
                let value = *value;
                quote! { #value }
            }
            ConstTpl::SpaceId(space) => {
                let index = space.index() as u64;
                quote! { #index }
            }
            ConstTpl::Handle(index, kind) => {
                let index = *index;
                let handle = quote! {
                    let handle = {
                        let opnds = input
                            .context
                            .constructors[input.point as usize].operands as usize;

                        input.context.constructors[opnds + #index].handle.as_ref().unwrap()
                    };
                };

                let action = match kind {
                    HandleKind::Space => quote! {
                        if handle.offset_space == fugue_lifter::utils::input::INVALID_HANDLE {
                            handle.space as u64
                        } else {
                            handle.temporary_space as u64
                        }
                    },
                    HandleKind::Offset => quote! {
                        if handle.offset_space == fugue_lifter::utils::input::INVALID_HANDLE {
                            handle.offset_offset
                        } else {
                            handle.temporary_offset
                        }
                    },
                    HandleKind::Size => quote! {
                        handle.size as u64
                    },
                    HandleKind::OffsetPlus(value) => {
                        let value = *value;
                        let value_short = value & 0xffff;
                        let value_shift = 8 * (value >> 16) as u32;

                        quote! {
                            if handle.space == 0 { // constant space
                                let val = if handle.offset_space == fugue_lifter::utils::input::INVALID_HANDLE {
                                    handle.offset_offset
                                } else {
                                    handle.temporary_offset
                                };
                                val.checked_shr(#value_shift).unwrap_or(0)
                            } else {
                                if handle.offset_space == fugue_lifter::utils::input::INVALID_HANDLE {
                                    handle.offset_offset + #value_short
                                } else {
                                    handle.temporary_offset + #value_short
                                }
                            }
                        }
                    }
                };

                quote! {
                    {
                        #handle
                        #action
                    }
                }
            }
            _ => unimplemented!("flow operations not supported"),
        }
    }

    pub fn generate_const_template_offset(&self, tmpl: &ConstTpl) -> TokenStream {
        match tmpl {
            ConstTpl::Handle(index, _) => {
                let index = *index;
                quote! {
                    {
                        let h = {
                            let opnds = input
                                .context
                                .constructors[input.point as usize].operands as usize;

                            input.context.constructors[opnds + #index].handle.as_ref().unwrap()
                        };

                        handle.offset_space = h.offset_space;
                        handle.offset_offset = h.offset_offset;
                        handle.offset_size = h.offset_size;
                        handle.temporary_space = h.temporary_space;
                        handle.temporary_offset = h.temporary_offset;
                    }
                }
            }
            _ => {
                let wrap_cases = self.translator.manager().spaces().iter().enumerate().map(|(i, spc)| {
                    let i = i as u8;
                    let highest = spc.highest_offset();
                    quote! { #i => #highest }
                });

                let wrap_match = quote! {
                    match handle.space {
                        #(#wrap_cases,)*
                        _ => unreachable!("invalid space"),
                    }
                };

                let value = self.generate_const_template(tmpl);

                quote! {
                    handle.offset_space = fugue_lifter::utils::input::INVALID_HANDLE;
                    handle.offset_offset = fugue_lifter::utils::wrap_offset(#wrap_match, #value);
                }
            }
        }
    }

    pub fn generate_const_template_space(&self, tmpl: &ConstTpl) -> TokenStream {
        match tmpl {
            ConstTpl::CurrentSpace => {
                let space = self.translator.manager().default_space_ref().index() as u8;
                quote! { #space }
            },
            ConstTpl::Handle(index, HandleKind::Space) => {
                let index = *index;
                quote! {
                    {
                        let h = {
                            let opnds = input
                                .context
                                .constructors[input.point as usize].operands as usize;

                            input.context.constructors[opnds + #index].handle.as_ref().unwrap()
                        };

                        if h.offset_space == fugue_lifter::utils::input::INVALID_HANDLE {
                            h.space
                        } else {
                            h.temporary_space
                        }
                    }
                }
            }
            ConstTpl::SpaceId(id) => {
                let space = self.translator.manager().space_by_id(*id).index() as u8;
                quote! { #space }
            }
            _ => unreachable!()
        }
    }

    pub fn generate_handle_template(&self, tmpl: &HandleTpl) -> TokenStream {
        if tmpl.ptr_space().is_real() {
            let space = self.generate_const_template_space(tmpl.space());
            let size = self.generate_const_template(tmpl.size());
            let offset_upd = self.generate_const_template_offset(tmpl.ptr_offset());

            return quote! {
                {
                    let mut handle = fugue_lifter::utils::FixedHandle {
                        space: #space,
                        size: #size as u8,
                        ..Default::default()
                    };

                    #offset_upd

                    handle
                }
            };
        }

        let space = self.generate_const_template_space(tmpl.space());
        let size = self.generate_const_template(tmpl.size());

        let offset_offset = self.generate_const_template(tmpl.ptr_offset());
        let offset_space = self.generate_const_template_space(tmpl.ptr_space());
        let offset_size = self.generate_const_template(tmpl.ptr_size());

        let temporary_offset = self.generate_const_template(tmpl.tmp_offset());
        let temporary_space = self.generate_const_template_space(tmpl.tmp_space());

        let space_cases = self.translator.manager().spaces().iter().enumerate().map(|(i, spc)| {
            let i = i as u8;
            let highest = spc.highest_offset();
            let word_size = spc.word_size() as u64;
            quote! { #i => (#highest, #word_size) }
        });

        let space_match = quote! {
            match handle.space {
                #(#space_cases,)*
                _ => unreachable!("invalid space"),
            }
        };

        quote! {
            {
                let mut handle = fugue_lifter::utils::FixedHandle {
                    space: #space,
                    size: #size as u8,
                    offset_offset: #offset_offset,
                    ..Default::default()
                };

                let offset_space = #offset_space;

                if offset_space == 0 { // constant
                    let (hoffset, word_size) = #space_match;

                    handle.offset_offset =
                        fugue_lifter::utils::wrap_offset(hoffset, handle.offset_offset * word_size);
                } else {
                    handle.offset_space = offset_space;
                    handle.offset_size = #offset_size as u8;

                    handle.temporary_offset = #temporary_offset;
                    handle.temporary_space = #temporary_space as u8;
                }

                handle
            }
        }
    }

    pub fn generate_constructor_template_resolvers(
        &self,
        id: usize,
        scope: usize,
        cid: usize,
        ctor: &Constructor,
    ) -> (TokenStream, TokenStream) {
        let mut helpers = TokenStream::new();

        let Some(templ) = ctor.template() else {
            return (helpers, quote! { None });
        };

        let result_resolver = if let Some(result) = templ.result() {
            let resolver = format_ident!("tmpl_result_resolver_{id}_{scope}_{cid}");
            let resolver_body = self.generate_handle_template(result);

            helpers.append_all(quote! {
                fn #resolver(input: &mut fugue_lifter::utils::ParserInput) -> fugue_lifter::utils::FixedHandle {
                    #resolver_body
                }
            });

            quote! { Some(#resolver) }
        } else {
            quote! { None }
        };

        if templ.operations().is_empty() {
            return (helpers, result_resolver);
        }

        (helpers, result_resolver)
    }

    pub fn generate_constructors<'b>(
        &'b self,
        id: usize,
        scope: usize,
        ctors: &'a [Constructor],
    ) -> impl Iterator<Item = TokenStream> + 'b {
        ctors.iter().enumerate().map(move |(cid, ctor)| {
            let ctor_vname = Self::ctor_vname(id, scope, cid);

            let (ctor_id1, ctor_id2) = ctor.id();
            let ctor_id = (ctor_id1 as u32 & 0xffff) << 16 | (ctor_id2 as u32 & 0xffff);

            let delay_slots = ctor.template().map(|tpl| tpl.delay_slot()).unwrap_or_default();
            let minimum_length = ctor.minimum_length();

            let pieces = ctor.print_pieces();

            let (operand_helpers, operands) = self.generate_constructor_operand_resolvers(id, scope, cid, ctor);
            let (context_helpers, apply_context) = self.generate_constructor_context_actions(id, scope, cid, ctor);

            let (template_helpers, template_result) = self.generate_constructor_template_resolvers(id, scope, cid, ctor);

            quote! {
                pub const #ctor_vname: &'static fugue_lifter::utils::Constructor = &fugue_lifter::utils::Constructor {
                    id: #ctor_id,
                    context_actions: #apply_context,
                    operands: &[#(#operands),*],
                    result: #template_result,
                    print_pieces: &[#(#pieces),*],
                    delay_slots: #delay_slots,
                    minimum_length: #minimum_length,
                };

                #context_helpers

                #(#operand_helpers)*

                #template_helpers
            }
        })
    }

    fn ctor_vname(id: usize, scope: usize, cid: usize) -> Ident {
        format_ident!("__SYM{id}_IN{scope}_CTOR{cid}")
    }

    pub fn generate_dtree_pmatch_ctxt(&self, cpat: &ContextPattern) -> TokenStream {
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
                    (input.context_bytes(#offset, #size) & #m == #v)
                }
            });

        quote! {
            (true #( && #parts )*)
        }
    }

    pub fn generate_dtree_pmatch_insn(&self, ipat: &InstructionPattern) -> TokenStream {
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
                    (input.instruction_bytes(#offset, #size)? & #m == #v)
                }
            });

        quote! {
            (true #( && #parts )*)
        }
    }

    pub fn generate_inner_dtree_aux(&self, id: usize, scope: usize, cid: usize) -> TokenStream {
        let ctor_name = Self::ctor_vname(id, scope, cid);
        quote! {
            #ctor_name.resolve_operands(input)?;
        }
    }

    pub fn generate_dtree_pmatch(
        &self,
        id: usize,
        scope: usize,
        pat: &DecisionPair,
    ) -> TokenStream {
        match pat.pattern() {
            DisjointPattern::Instruction(ipat) => {
                let cid = pat.id();
                let ctor = Self::ctor_vname(id, scope, cid);
                let cond = self.generate_dtree_pmatch_insn(ipat);

                quote! {
                    if #cond {
                        return Some(#ctor);
                    }
                }
            }
            DisjointPattern::Context(cpat) => {
                let cid = pat.id();
                let ctor = Self::ctor_vname(id, scope, cid);
                let cond = self.generate_dtree_pmatch_ctxt(cpat);

                quote! {
                    if #cond {
                        return Some(#ctor);
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
                        return Some(#ctor);
                    }
                }
            }
        }
    }

    pub fn generate_dtree_aux(
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
                        fn #tree_fn(input: &mut fugue_lifter::utils::ParserInput) -> Option<&'static fugue_lifter::utils::Constructor> {
                            #body
                        }
                    });

                    quote! {
                        (#tree_fn as fn(&mut fugue_lifter::utils::ParserInput) -> Option<&'static fugue_lifter::utils::Constructor>)
                    }
                })
                .collect::<Vec<_>>();

            let start_bit = dtree.start_bit();
            let size = dtree.size();

            let check = if dtree.context_decision() {
                quote! { input.context_bits(#start_bit, #size) }
            } else {
                quote! { input.instruction_bits(#start_bit, #size)? }
            };

            let nodes = dtree.children().len();

            let table = Ident::new(
                &format!("{tree_fn_prefix}_LOOKUP").to_uppercase(),
                proc_macro2::Span::call_site(),
            );

            trees.push(quote! {
                const #table: [fn(&mut fugue_lifter::utils::ParserInput) -> Option<&'static fugue_lifter::utils::Constructor>; #nodes] = [
                    #(#parts),*
                ];
            });

            quote! {
                (#table.get(#check as usize)?)(input)
            }
        }
    }

    pub fn generate_dtree(
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
            pub fn resolve(input: &mut fugue_lifter::utils::ParserInput) -> Option<&'static fugue_lifter::utils::Constructor> {
                #body
            }
        }
    }

    pub fn generate_subtable(
        &self,
        id: usize,
        scope: usize,
        name: &str,
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
            pub struct #tname;

            impl #tname {
                pub const ID: usize = #id;
                pub const SCOPE: usize = #scope;
                pub const NAME: &'static str = #name;

                #[allow(unused_parens)]
                #dtree_tokens
            }
        };

        Ok(tokens)
    }
}

impl<'a> ToTokens for LifterGenerator<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        // TODO: we should make a mapping for registers to compute
        // overlaps, etc.

        let userops = self.translator.user_ops().iter().map(|op| op.as_str());
        let userops_to_ids = self.translator.user_ops().iter().enumerate().map(|(i, op)| {
            let id = i as u16;
            let name = op.as_str();
            let name_bytes = Literal::byte_string(name.as_bytes());

            quote! { #name_bytes => #id }
        });
        let n_userops = self.translator.user_ops().len();


        // TODO: this should be a string-like type that can allow easier
        // comparisons.

        tokens.append_all(quote! {
            pub const USER_OPS: [&'static str; #n_userops] = [
                #(#userops),*
            ];

            #[inline]
            pub const fn user_op_by_name(name: &'static str) -> u16 {
                match name.as_bytes() {
                    #(#userops_to_ids,)*
                    _ => panic!("unknown user op"),
                }
            }

            #[inline]
            pub const fn user_op_by_id(id: u16) -> &'static str {
                USER_OPS[id as usize]
            }
        });

        tokens.append_all(&self.symbols);
    }
}
