use std::mem::size_of;

use fugue_ir::disassembly::symbol::sub_table::Context;
use fugue_ir::disassembly::symbol::sub_table::{
    ContextPattern, DecisionPair, DisjointPattern, InstructionPattern,
};
use fugue_ir::disassembly::symbol::{Constructor, DecisionNode, Symbol};
use fugue_ir::disassembly::PatternExpression;
use fugue_ir::Translator;

// use itertools::Itertools as _;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens, TokenStreamExt};
use syn::Ident;

use crate::LifterGeneratorError;

pub struct LifterGenerator<'a> {
    symbols: Vec<TokenStream>,
    ctor_names: Vec<Ident>,
    lifter: TokenStream,
    translator: &'a Translator,
}

impl<'a> LifterGenerator<'a> {
    pub fn new(translator: &'a Translator) -> Result<Self, LifterGeneratorError> {
        let mut slf = Self {
            symbols: Vec::new(),
            ctor_names: Vec::new(),
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
        let mut variants = Vec::new();

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
                        &mut variants,
                    )?);
                }
                _ => (),
            }
        }

        self.ctor_names = variants;

        Ok(())
    }

    pub fn generate_pattern_resolver(&self, pattern: &PatternExpression) -> TokenStream {
        match pattern {
            PatternExpression::Constant { value } => quote! { #value },
            PatternExpression::StartInstruction => quote! { (input.address() as i64) },
            PatternExpression::EndInstruction => {
                quote! { (input.next_address().unwrap_or(0) as i64) }
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
                        res = crate::utils::byte_swap(res, #size);
                    });
                }

                parts.push(quote! {
                    res = res.checked_shr(#shift).unwrap_or(if res < 0 { -1 } else { 0 });
                });

                let range = bit_end - bit_start;

                parts.push(if *sign_bit {
                    quote! { crate::utils::sign_extend(res, #range) }
                } else {
                    quote! { crate::utils::zero_extend(res, #range) }
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
                    quote! { crate::utils::sign_extend(res, #range) }
                } else {
                    quote! { crate::utils::zero_extend(res, #range) }
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

                // get current point + depth
                //
                // we traverse the tree upwards from the current point until
                // we find the constructor specified
                //
                // if we don't find it, then we compute the value
                // if we find that the current point's ctor is
                // until we hit the root of the tree.
                // 0
                let (ctor_id1, ctor_id2) = ctor.id();
                let ctor_id = (ctor_id1 as u32 & 0xffff) << 16 | (ctor_id2 as u32 & 0xffff);
                let ctor_vname = Self::ctor_vname(*table_id, *scope, *constructor_id);
                let pattern_value = self.generate_pattern_resolver(&pexpr);

                let rel_offset = operand.relative_offset() as u8;
                let offset = if operand.offset_base().is_none() {
                    quote! {
                        point.offset + #rel_offset;
                    }
                } else {
                    quote! {
                        input.context.constructors[point.operands as usize + #index].offset
                    }
                };

                quote! {
                    {
                        let operand_value = |input: &mut ParserInput| -> Option<i64> {
                            let mut cur_depth = input.depth;
                            let mut point = &input.context.constructors[input.point as usize];

                            while point.constructor.map(|ctor| ctor.id()) != Some(#ctor_id) {
                                if cur_depth <= 0 {
                                    // preserve old and init new state
                                    let old_point = input.point;
                                    let old_depth = std::mem::take(&mut input.depth);
                                    let old_breadcrumb = std::mem::replace(&mut input.breadcrumb, [0u8; BREADCRUMBS]);

                                    input.point = input.context.alloc;
                                    {
                                        let state = &mut input.context.constructors[input.point as usize];

                                        state.constructor = Some(#ctor_vname);
                                        state.parent = INVALID_HANDLE;
                                        state.operands = INVALID_HANDLE;
                                        state.offset = 0;
                                        state.length = 0;
                                    }

                                    // compute the value in the modified context
                                    let value = { #pattern_value };

                                    // restore old state
                                    {
                                        let state = &mut input.context.constructors[input.point as usize];

                                        state.constructor = None;
                                        state.parent = INVALID_HANDLE;
                                        state.operands = INVALID_HANDLE;
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
                            let old_breadcrumb = std::mem::replace(&mut input.breadcrumb, [0u8; BREADCRUMBS]);

                            input.point = input.context.alloc;
                            {
                                let state = &mut input.context.constructors[input.point as usize];

                                state.constructor = Some(#ctor_vname);
                                state.parent = INVALID_HANDLE;
                                state.operands = INVALID_HANDLE;
                                state.offset = offset;
                                state.length = length;
                            }

                            // compute the value in the modified context
                            let value = { #pattern_value };

                            // restore old state
                            {
                                let state = &mut input.context.constructors[input.point as usize];

                                state.constructor = None;
                                state.parent = INVALID_HANDLE;
                                state.operands = INVALID_HANDLE;
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

    /*
    pub fn generate_constructor_operand_resolver(
        &self,
        id: &Ident,
        ctor: &Constructor,
    ) -> TokenStream {
        // NOTE: INIT and FINI would be factored into functions on the
        // constructor, and we'd have a method `resolve_operand_chunk(N)`
        // that will resolve the operand chunk indicated by input.operand().
        //
        // This function may return an invalid handle (in the case of no
        // remaining operands), and in that case, we will call FINI.
        //
        // Thus, the function would look like:
        //
        // #id.begin_operand_resolution(input)?;
        //
        // while (input.is_state()) {
        //     let ctor = input.constructor();
        //     let opnd = input.operand();
        //
        //     if opnd == INVALID_HANDLE {
        //         ctor.end_operand_resolution(input)?;
        //         continue;
        //     }
        //
        //     ctor.resolve_operand_chunk(ctor)?;
        // }
        //
        // It could also be possible to handle invalid, i.e., "we are done",
        // sentinel values so we actually perform finalisation on the very
        // final chunk.
        //
        // Within resolve_operand_chunk(N), where we deal with a constructor:
        //
        // ...
        //
        // // Perform regular operand handling
        //
        // let offset = #offset_base + #offset_rela;
        // input.push_operand(#oid);
        // input.set_offset(offset);
        //
        // // For a regular operand we perform the following:
        // input.set_current_length(#min_length);
        // input.pop_operand();
        //
        // // For a constructor we begin resolution
        // let ctor = #stname.resolve(input)?;
        // ctor.begin_operand_resolution(input)?;
        //
        // // NOTE: this return corresponds to the inner break in the original
        // // implementation.
        // //
        // return Some(())
        //

        let count = ctor.operand_count();
        let minimum_length = ctor.minimum_length();
        let delay_slot = ctor
            .template()
            .and_then(|tpl| {
                let amount = tpl.delay_slot();
                if amount > 0 {
                    Some(quote! { input.set_delay_slot(#amount); })
                } else {
                    None
                }
            })
            .unwrap_or_default();

        if count == 0 {
            return quote! {
                #[inline]
                fn resolve_operands(&self, input: &mut ParserInput) -> Option<()> {
                    self.resolve_operand_constructor(input)?;
                    self.resolve_operand_chunk(input, 0)?;
                    Some(())
                }

                #[inline]
                fn resolve_operand_constructor(&self, input: &mut ParserInput) -> Option<()> {
                    // Perform the setup actions
                    input.set_constructor(#id);
                    self.apply_context(input)?;
                    Some(())
                }

                #[inline]
                fn resolve_operand_chunk(&self, input: &mut ParserInput, _operand: usize) -> Option<()> {
                    // Since we have no operands, we have no chunks, so we
                    // perform clean-up actions.

                    input.calculate_length(#minimum_length, #count);
                    input.pop_operand(); // the ctor

                    #delay_slot

                    Some(())
                }
            };
        }

        let mut operands = (0..count).into_iter().map(|oid| {
            let index = ctor.operand(oid);
            let operand = self.translator.symbol_table().unchecked_symbol(index);

            let offset_base = operand
                .offset_base()
                .map(|v| quote! { input.offset_for_operand(#v) })
                .unwrap_or(quote! { input.offset() });
            let offset_rela = operand.relative_offset();

            let clean_up = if let Symbol::Operand { min_length, .. } = operand {
                quote! {
                    input.set_current_length(#min_length);
                    input.pop_operand();
                }
            } else {
                TokenStream::new()
            };

            let part1 = quote! {
                let offset = #offset_base + #offset_rela;

                input.push_operand(#oid);
                input.set_offset(offset);
            };

            // NOTE: for constructors we will set this to true
            let mut stop_point = false;

            let part2 = if let Some(tsym) = operand.defining_symbol(self.translator.symbol_table())
            {
                // let symn = tsym.name();
                match tsym {
                    Symbol::Subtable { id, scope, .. } => {
                        // NOTE: This is a stop point (new constructor to explore)
                        stop_point = true;

                        // The subtable to perform resolution
                        let stname = format_ident!("SubTable{id}In{scope}");

                        quote! {
                            // NOTE:
                            //
                            // We track the operand bounds and "current" operand via breadcrumbs;
                            // this means that if we have an array of functions for handling each
                            // operand chunk, we could bound the stack growth of operand
                            // resolution. Consider the following:
                            //
                            // Each chunk will look like:
                            // - op
                            // - op
                            // - ctor (where ctor ends a chunk)
                            //
                            // So let's assume we have:
                            // - op
                            // - op
                            // - ctor
                            // - op
                            //
                            // We will have two chunks
                            //
                            // chunk 0:
                            // - op
                            // - op
                            // - ctor
                            //
                            // chunk 1:
                            // - op
                            //
                            // We will process things as before:
                            //
                            // let operand = input.operand(); // gives an index
                            //
                            // We could have "constructor" set as an array of functions:
                            //
                            //     &'static [fn(input: &mut ParserInput) -> Option<()>]
                            //
                            // Then we'd do this:
                            //
                            // let operand = input.operand();
                            // let operands = input.constructor();
                            //
                            // operands[operand](input)?;
                            //
                            // Alternatively, we could have N resolve operand functions that are
                            // dispatched via resolve_operands(n) which again resolves to a chunk.
                            //
                            // Why do this?
                            //
                            // This will eliminate stack growth during operand resolution, which
                            // will match the original Ghidra implementation; it should also
                            // reduce the size of functions.
                            //

                            // println!("operand {} for ({}, {}) / {}", #oid, #ctor_id1, #ctor_id2, #symn);

                            let op_ctor = #stname.resolve(input)?;

                            /*
                            for pp in op_ctor.print_pieces() {
                                print!("{pp}");
                            }
                            println!();
                            */

                            // This will put the constructor inside the parser context so we
                            // can resolve its operands (if they exist) on the next iteration.
                            op_ctor.resolve_operand_constructor(input)?;
                        }
                    }
                    Symbol::ValueMap {
                        table_is_filled,
                        pattern_value,
                        value_table,
                        ..
                    } => {
                        if !*table_is_filled {
                            let bad_indices = value_table
                                .iter()
                                .enumerate()
                                .filter_map(|(i, v)| if *v == 0xbadbeef { Some(i) } else { None });

                            let pattern_resolver = self.generate_pattern_resolver(pattern_value);
                            let limit = value_table.len();

                            quote! {
                                let index = #pattern_resolver as usize;
                                if index >= #limit || [#(#bad_indices),*].contains(&index) {
                                    return None;
                                }
                                #clean_up
                            }
                        } else {
                            clean_up
                        }
                    }
                    Symbol::VarnodeList {
                        table_is_filled,
                        pattern_value,
                        varnode_table,
                        ..
                    } => {
                        if !*table_is_filled {
                            let bad_indices = varnode_table
                                .iter()
                                .enumerate()
                                .filter_map(|(i, v)| if v.is_none() { Some(i) } else { None });

                            let pattern_resolver = self.generate_pattern_resolver(pattern_value);
                            let limit = varnode_table.len();

                            quote! {
                                let index = #pattern_resolver as usize;
                                if index >= #limit || [#(#bad_indices),*].contains(&index) {
                                    return None;
                                }
                                #clean_up
                            }
                        } else {
                            clean_up
                        }
                    }
                    Symbol::Name {
                        table_is_filled,
                        pattern_value,
                        name_table,
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

                            let pattern_resolver = self.generate_pattern_resolver(pattern_value);
                            let limit = name_table.len();

                            quote! {
                                let index = #pattern_resolver as usize;
                                if index >= #limit || [#(#bad_indices),*].contains(&index) {
                                    return None;
                                }
                                #clean_up
                            }
                        } else {
                            clean_up
                        }
                    }
                    _ => clean_up,
                }
            } else {
                clean_up
            };

            (
                oid,
                quote! {
                    #part1
                    #part2
                },
                stop_point,
            )
        });

        let mut chunks = Vec::new();

        loop {
            let mut chunk = (&mut operands).take_while_inclusive(|(_, _, stop)| !stop);

            let Some((oid, init, stop)) = chunk.next() else {
                break;
            };

            if stop {
                chunks.push(quote! {
                    #oid => {
                        #init
                        Some(())
                    }
                });
                continue;
            }

            let rest = chunk.map(|(_, tokens, _)| tokens);

            chunks.push(quote! {
                #oid => {
                    #init
                    #(#rest)*
                    Some(())
                }
            });
        }

        // #id.begin_operand_resolution(input)?;
        //
        // while (input.is_state()) {
        //     let ctor = input.constructor();
        //     let opnd = input.operand();
        //
        //     if opnd == INVALID_HANDLE {
        //         ctor.end_operand_resolution(input)?;
        //         continue;
        //     }
        //
        //     ctor.resolve_operand_chunk(ctor)?;
        // }

        quote! {
            #[inline]
            fn resolve_operands(&self, input: &mut ParserInput) -> Option<()> {
                self.resolve_operand_constructor(input)?;

                while !input.resolved() {
                    let ctor = input.constructor(); // I guess this should be unchecked.
                    let opnd = input.operand(); // This is the chunk number.

                    /*
                    println!("{ctor:?}");
                    for pp in ctor.print_pieces() {
                        print!("{pp}");
                    }
                    println!();
                    */

                    ctor.resolve_operand_chunk(input, opnd)?;
                }

                Some(())
            }

            #[inline]
            fn resolve_operand_constructor(&self, input: &mut ParserInput) -> Option<()> {
                input.set_constructor(#id);
                self.apply_context(input)?;
                input.allocate_operands(#count);
                Some(())
            }

            #[inline]
            fn resolve_operand_chunk(&self, input: &mut ParserInput, operand: usize) -> Option<()> {
                match operand {
                    #(#chunks),*
                    _ => {
                        // This is the default action when we have no more chunks to process
                        input.calculate_length(#minimum_length, #count);
                        input.pop_operand(); // the ctor

                        #delay_slot

                        Some(())
                    }
                }
            }
        }
    }
    */

    pub fn generate_constructor_operand_resolver(
        &self,
        id: &Ident,
        ctor: &Constructor,
    ) -> TokenStream {
        // let (ctor_id1, ctor_id2) = ctor.id();
        let count = ctor.operand_count();
        let minimum_length = ctor.minimum_length();
        let delay_slot = ctor
            .template()
            .and_then(|tpl| {
                let amount = tpl.delay_slot();
                if amount > 0 {
                    Some(quote! { input.set_delay_slot(#amount); })
                } else {
                    None
                }
            })
            .unwrap_or_default();

        if count == 0 {
            return quote! {
                // println!("constructor ({}, {}) (no args)", #ctor_id1, #ctor_id2);

                input.set_constructor(#id);
                #id.apply_context(input)?;

                input.calculate_length(#minimum_length, #count);
                input.pop_operand(); // the ctor

                #delay_slot

                return Some(());
            };
        }

        let operands = (0..count).into_iter().map(|oid| {
            let index = ctor.operand(oid);
            let operand = self.translator.symbol_table().unchecked_symbol(index);

            let offset_base = operand
                .offset_base()
                .map(|v| quote! { input.offset_for_operand(#v) })
                .unwrap_or(quote! { input.offset() });
            let offset_rela = operand.relative_offset();

            let clean_up = if let Symbol::Operand { min_length, .. } = operand {
                quote! {
                    input.set_current_length(#min_length);
                    input.pop_operand();
                }
            } else {
                TokenStream::new()
            };

            let part1 = quote! {
                let offset = #offset_base + #offset_rela;

                input.push_operand(#oid);
                input.set_offset(offset);
            };

            let part2 = if let Some(tsym) = operand.defining_symbol(self.translator.symbol_table())
            {
                // let symn = tsym.name();
                match tsym {
                    Symbol::Subtable { id, scope, .. } => {
                        let stname = format_ident!("SubTable{id}In{scope}");
                        quote! {
                            // NOTE:
                            //
                            // We track the operand bounds and "current" operand via breadcrumbs;
                            // this means that if we have an array of functions for handling each
                            // operand chunk, we could bound the stack growth of operand
                            // resolution. Consider the following:
                            //
                            // Each chunk will look like:
                            // - op
                            // - op
                            // - ctor (where ctor ends a chunk)
                            //
                            // So let's assume we have:
                            // - op
                            // - op
                            // - ctor
                            // - op
                            //
                            // We will have two chunks
                            //
                            // chunk 0:
                            // - op
                            // - op
                            // - ctor
                            //
                            // chunk 1:
                            // - op
                            //
                            // We will process things as before:
                            //
                            // let operand = input.operand(); // gives an index
                            //
                            // We could have "constructor" set as an array of functions:
                            //
                            //     &'static [fn(input: &mut ParserInput) -> Option<()>]
                            //
                            // Then we'd do this:
                            //
                            // let operand = input.operand();
                            // let operands = input.constructor();
                            //
                            // operands[operand](input)?;
                            //
                            // Alternatively, we could have N resolve operand functions that are
                            // dispatched via resolve_operands(n) which again resolves to a chunk.
                            //
                            // Why do this?
                            //
                            // This will eliminate stack growth during operand resolution, which
                            // will match the original Ghidra implementation; it should also
                            // reduce the size of functions.
                            //

                            // println!("operand {} for ({}, {}) / {}", #oid, #ctor_id1, #ctor_id2, #symn);

                            // NOTE: resolve will resolve the ctor, we don't need to splice
                            // clean_up as we do for all other cases.
                            {
                                // NOTE: we could avoid the dynamic dispatch here?
                                // #stname.resolve(input)?;

                                let op_ctor = #stname.resolve(input)?;

                                /*
                                for pp in op_ctor.print_pieces() {
                                    print!("{pp}");
                                }
                                println!();
                                */

                                op_ctor.resolve_operands(input)?;
                            }
                        }
                    }
                    Symbol::ValueMap {
                        table_is_filled,
                        pattern_value,
                        value_table,
                        ..
                    } => {
                        if !*table_is_filled {
                            let bad_indices = value_table
                                .iter()
                                .enumerate()
                                .filter_map(|(i, v)| if *v == 0xbadbeef { Some(i) } else { None });

                            let pattern_resolver = self.generate_pattern_resolver(pattern_value);
                            let limit = value_table.len();

                            quote! {
                                let index = #pattern_resolver as usize;
                                if index >= #limit || [#(#bad_indices),*].contains(&index) {
                                    return None;
                                }
                                #clean_up
                            }
                        } else {
                            clean_up
                        }
                    }
                    Symbol::VarnodeList {
                        table_is_filled,
                        pattern_value,
                        varnode_table,
                        ..
                    } => {
                        if !*table_is_filled {
                            let bad_indices = varnode_table
                                .iter()
                                .enumerate()
                                .filter_map(|(i, v)| if v.is_none() { Some(i) } else { None });

                            let pattern_resolver = self.generate_pattern_resolver(pattern_value);
                            let limit = varnode_table.len();

                            quote! {
                                let index = #pattern_resolver as usize;
                                if index >= #limit || [#(#bad_indices),*].contains(&index) {
                                    return None;
                                }
                                #clean_up
                            }
                        } else {
                            clean_up
                        }
                    }
                    Symbol::Name {
                        table_is_filled,
                        pattern_value,
                        name_table,
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

                            let pattern_resolver = self.generate_pattern_resolver(pattern_value);
                            let limit = name_table.len();

                            quote! {
                                let index = #pattern_resolver as usize;
                                if index >= #limit || [#(#bad_indices),*].contains(&index) {
                                    return None;
                                }
                                #clean_up
                            }
                        } else {
                            clean_up
                        }
                    }
                    _ => clean_up,
                }
            } else {
                clean_up
            };

            quote! {
                #part1
                #part2
            }
        });

        // TODO: pre/post-actions based on context updates...

        // NOTE: INIT and FINI would be factored into functions on the
        // constructor, and we'd have a method `resolve_operand_chunk(N)`
        // that will resolve the operand chunk indicated by input.operand().
        //
        // This function may return an invalid handle (in the case of no
        // remaining operands), and in that case, we will call FINI.
        //
        // Thus, the function would look like:
        //
        // #id.begin_operand_resolution(input)?;
        //
        // while (input.is_state()) {
        //     let ctor = input.constructor();
        //     let opnd = input.operand();
        //
        //     if opnd == INVALID_HANDLE {
        //         ctor.end_operand_resolution(input)?;
        //         continue;
        //     }
        //
        //     ctor.resolve_operand_chunk(ctor)?;
        // }
        //
        // It could also be possible to handle invalid, i.e., "we are done",
        // sentinel values so we actually perform finalisation on the very
        // final chunk.
        //
        // Within resolve_operand_chunk(N), where we deal with a constructor:
        //
        // ...
        //
        // // Perform regular operand handling
        //
        // let offset = #offset_base + #offset_rela;
        // input.push_operand(#oid);
        // input.set_offset(offset);
        //
        // // For a regular operand we perform the following:
        // input.set_current_length(#min_length);
        // input.pop_operand();
        //
        // // For a constructor we begin resolution
        // let ctor = #stname.resolve(input)?;
        // ctor.begin_operand_resolution(input)?;
        //
        // // NOTE: this return corresponds to the inner break in the original
        // // implementation.
        // //
        // return Some(())
        //

        quote! {
            // INIT: this part would correspond to ctor allocation
            input.set_constructor(#id);
            #id.apply_context(input)?;

            // we allocate all operands upfront, then push each one
            input.allocate_operands(#count);
            // INIT (end)

            #(#operands)*

            // FINI: this part would correspond to ctor finalisation
            input.calculate_length(#minimum_length, #count);
            input.pop_operand(); // the ctor

            #delay_slot
            // FINI (end)

            Some(())
        }
    }

    pub fn generate_constructor_context_actions(&self, ctor: &Constructor) -> TokenStream {
        let parts = ctor.context().iter().filter_map(|context| {
            let Context::Operator {
                num,
                shift,
                mask,
                pattern_value,
            } = context
            else {
                return None;
            };

            let num = *num;
            let shift = *shift;
            let mask = *mask;
            let value = self.generate_pattern_resolver(pattern_value);

            Some(quote! {
                let value = (#value as u32) << #shift;
                input.set_context_word(#num, value, #mask);
            })
        });

        quote! {
            #(#parts)*
        }
    }

    pub fn generate_constructors<'b>(
        &'b self,
        id: usize,
        scope: usize,
        ctors: &'a [Constructor],
        variants: &'b mut Vec<Ident>,
    ) -> impl Iterator<Item = TokenStream> + 'b {
        ctors.iter().enumerate().map(move |(cid, ctor)| {
            let ctor_tname = format_ident!("SubTable{id}In{scope}Constructor{cid}");
            let ctor_vname = Self::ctor_vname(id, scope, cid);
            let ctor_vname_id = format_ident!("{ctor_vname}_ID");
            let ctor_vname_impl = format_ident!("{ctor_vname}_IMPL");

            let (ctor_id1, ctor_id2) = ctor.id();
            let ctor_id = (ctor_id1 as u32 & 0xffff) << 16 | (ctor_id2 as u32 & 0xffff);

            let pieces = ctor.print_pieces();
            let resolver_body = self.generate_constructor_operand_resolver(&ctor_vname, ctor);
            let apply_context = self.generate_constructor_context_actions(ctor);

            variants.push(ctor_tname.clone());

            // procedure:
            //
            // 1. apply context for current constructor
            // 2. iterate over operands for current constructor;
            //  2.1. compute offset of operand (relative)
            //  2.2. if the operand has a defining symbol and it is a sub-table,
            //       then we set the current constructor (relative) and go to 1
            //  2.3. if there is no sub-table, the current (relative) length is
            //       updated based on the minimum length of the operand.
            //  2.4. we proceed as if we're at the state of the parent of the
            //       operand (set via the operand itself or relative to the ctor).
            //  2.5. if we have processed all operands (counted), we calculate the
            //       length of all operands (instruction length)
            //   2.5.1. if the current constructor has a template with delay, then
            //          set the current delay slot.
            //

            // operand will have:
            // - offset (calculated relative to current position or previous operand)
            // - (optional) parent
            // - ...

            // We could build the tree more easily?
            //
            // Unroll such that:
            // - process operand1
            // - process operand2
            //   - process operand2.1
            //   - process operand2.2
            // - process operand3

            quote! {
                #[derive(Debug, Clone, Copy)]
                pub struct #ctor_tname;

                pub const #ctor_vname_id: u32 = #ctor_id;
                pub const #ctor_vname_impl: #ctor_tname = #ctor_tname;

                pub const #ctor_vname: ConstructorT = ConstructorT::#ctor_tname(#ctor_vname_impl);

                impl #ctor_tname {
                    #[inline]
                    fn id(&self) -> u32 {
                        #ctor_id
                    }

                    #[inline]
                    fn apply_context(&self, input: &mut ParserInput) -> Option<()> {
                        #apply_context
                        Some(())
                    }

                    #[inline]
                    fn print_pieces(&self) -> &'static [&'static str] {
                        &[#(#pieces),*]
                    }

                    #[inline]
                    fn resolve_operands(&self, input: &mut ParserInput) -> Option<()> {
                        #resolver_body
                    }

                    // #resolver_body

                    #[inline]
                    fn resolve_operand_handles(&self, input: &mut ParserInput) -> Option<()> {
                        todo!()
                    }
                }
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
                // let body = self.generate_inner_dtree_aux(id, scope, cid);

                quote! {
                    if #cond {
                        // NOTE: calling this here might be more efficient
                        // #ctor.resolve_operands(input)?;
                        return Some(#ctor);
                    }
                }
            }
            DisjointPattern::Context(cpat) => {
                let cid = pat.id();
                let ctor = Self::ctor_vname(id, scope, cid);
                let cond = self.generate_dtree_pmatch_ctxt(cpat);
                // let body = self.generate_inner_dtree_aux(id, scope, cid);

                quote! {
                    if #cond {
                        // #ctor.resolve_operands(input)?;
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
                // let body = self.generate_inner_dtree_aux(id, scope, cid);

                let ccond = self.generate_dtree_pmatch_ctxt(cpat);
                let icond = self.generate_dtree_pmatch_insn(ipat);

                quote! {
                    if #icond && #ccond {
                        // #ctor.resolve_operands(input)?;
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
                        fn #tree_fn(input: &mut ParserInput) -> Option<ConstructorT> {
                            #body
                        }
                    });

                    quote! {
                        (#tree_fn as fn(&mut ParserInput) -> Option<ConstructorT>)
                    }
                    /*
                    quote! {
                        #bitn => { #tree_fn(input) }
                    }
                    */
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
                const #table: [fn(&mut ParserInput) -> Option<ConstructorT>; #nodes] = [
                    #(#parts),*
                ];
            });

            quote! {
                (#table.get(#check as usize)?)(input)
            }

            /*
            quote! {
                match #check {
                    #(#parts),*
                    _ => { return None },
                }
            }
            */
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
            pub fn resolve(&self, input: &mut ParserInput) -> Option<ConstructorT> {
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
        variants: &mut Vec<Ident>,
    ) -> Result<TokenStream, LifterGeneratorError> {
        let tname = format_ident!("SubTable{id}In{scope}");
        let mut trees = Vec::new();

        let ctor_tokens = self.generate_constructors(id, scope, ctors, variants);
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
        let cname = &self.ctor_names;

        // This is the main impl for all constructors
        let ctor = quote! {
            #![allow(unused_parens)]
            #![allow(unused_variables)]

            use fugue_lifter::utils::input::*;

            #[derive(Debug, Clone, Copy)]
            pub enum ConstructorT {
                #(#cname(#cname)),*
            }

            impl ConstructorT {
                #[inline]
                pub fn id(&self) -> u32 {
                    match self {
                        #(Self::#cname(ref v) => v.id()),*
                    }
                }

                #[inline]
                pub fn apply_context(&self, input: &mut ParserInput) -> Option<()> {
                    match self {
                        #(Self::#cname(ref v) => v.apply_context(input)),*
                    }
                }

                #[inline]
                pub fn print_pieces(&self) -> &'static [&'static str] {
                    match self {
                        #(Self::#cname(ref v) => v.print_pieces()),*
                    }
                }

                #[inline]
                pub fn resolve_operands(&self, input: &mut ParserInput) -> Option<()> {
                    match self {
                        #(Self::#cname(ref v) => v.resolve_operands(input)),*
                    }
                }

                /*
                #[inline]
                pub fn resolve_operand_constructor(&self, input: &mut ParserInput) -> Option<()> {
                    match self {
                        #(Self::#cname(ref v) => v.resolve_operand_constructor(input)),*
                    }
                }

                #[inline]
                pub fn resolve_operand_chunk(&self, input: &mut ParserInput, operand: usize) -> Option<()> {
                    match self {
                        #(Self::#cname(ref v) => v.resolve_operand_chunk(input, operand)),*
                    }
                }
                */

                #[inline]
                pub fn resolve_operand_handles(&self, input: &mut ParserInput) -> Option<()> {
                    match self {
                        #(Self::#cname(ref v) => v.resolve_operand_handles(input)),*
                    }
                }
            }
        };

        tokens.append_all(ctor);
        tokens.append_all(&self.symbols);
    }
}
