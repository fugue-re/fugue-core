use crate::address::AddressValue;
use crate::disassembly::Error;
use crate::disassembly::IRBuilderArena;
use crate::disassembly::context::ContextDatabase;
use crate::disassembly::pattern::PatternExpression;
use crate::disassembly::symbol::{Constructor, FixedHandle, Symbol, SymbolTable};
use crate::space_manager::SpaceManager;

use std::cell::RefCell;
use std::fmt;
use std::mem::size_of;

use bumpalo::collections::Vec as BVec;
use bumpalo::vec as bvec;

use unsafe_unwrap::UnsafeUnwrap;

pub struct InstructionFormatter<'b, 'c, 'z> {
    walker: RefCell<ParserWalker<'b, 'c, 'z>>,
    symbols: &'b SymbolTable,
    ctor: &'b Constructor,
}

pub struct MnemonicFormatter<'a, 'b, 'c, 'z> {
    inner: &'a InstructionFormatter<'b, 'c, 'z>,
}

pub struct OperandFormatter<'a, 'b, 'c, 'z> {
    inner: &'a InstructionFormatter<'b, 'c, 'z>,
}

impl<'b, 'c, 'z> InstructionFormatter<'b, 'c, 'z> {
    pub fn new(walker: ParserWalker<'b, 'c, 'z>, symbols: &'b SymbolTable, ctor: &'b Constructor) -> Self {
        Self {
            walker: RefCell::new(walker),
            symbols,
            ctor,
        }
    }

    pub fn mnemonic<'a>(&'a self) -> MnemonicFormatter<'a, 'b, 'c, 'z> {
        MnemonicFormatter {
            inner: self,
        }
    }

    pub fn operands<'a>(&'a self) -> OperandFormatter<'a, 'b, 'c, 'z> {
        OperandFormatter {
            inner: self,
        }
    }
}

impl<'b, 'c, 'z> fmt::Display for InstructionFormatter<'b, 'c, 'z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.ctor.format_mnemonic(f, &mut self.walker.borrow_mut(), self.symbols)?;
        write!(f, " ")?;
        self.ctor.format_body(f, &mut self.walker.borrow_mut(), self.symbols)?;
        Ok(())
    }
}

impl<'a, 'b, 'c, 'z> fmt::Display for MnemonicFormatter<'a, 'b, 'c, 'z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.inner.ctor.format_mnemonic(f, &mut self.inner.walker.borrow_mut(), self.inner.symbols)?;
        Ok(())
    }
}

impl<'a, 'b, 'c, 'z> fmt::Display for OperandFormatter<'a, 'b, 'c, 'z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.inner.ctor.format_body(f, &mut self.inner.walker.borrow_mut(), self.inner.symbols)?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct ConstructState<'b> {
    parent: Option<usize>,
    constructor: Option<&'b Constructor>,
    handle: Option<FixedHandle<'b>>,
    resolve: [Option<usize>; 64],
    length: usize,
    offset: usize,
}

impl<'b> ConstructState<'b> {
    pub fn set_parent(&mut self, parent: usize) {
        self.parent = Some(parent);
    }
}

impl<'b> Default for ConstructState<'b> {
    fn default() -> Self {
        Self {
            parent: None,
            constructor: None,
            handle: None,
            resolve: [None; 64],
            length: 0,
            offset: 0,
        }
    }
}

#[derive(Clone)]
pub struct ContextSet<'b> {
    triple: &'b Symbol,
    number: usize,
    mask: u32,
    value: u32,
    point: usize,
    flow: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ParserState {
    Uninitialised,
    Disassembly,
    PCode,
}

#[derive(Clone)]
pub struct ParserContext<'b, 'z> {
    parse_state: ParserState,
    context: BVec<'z, u32>,
    context_commit: BVec<'z, ContextSet<'b>>,

    backing: [u8; 16],

    address: AddressValue,
    next_address: Option<AddressValue>,

    delay_slot: usize,

    alloc: usize,
    state: BVec<'z, ConstructState<'b>>,
}

impl<'b, 'z> ParserContext<'b, 'z> {
    pub fn empty(arena: &'z IRBuilderArena, space_manager: &'b SpaceManager) -> Self {
        Self {
            parse_state: ParserState::Uninitialised,
            context: BVec::with_capacity_in(2, arena.inner()),
            context_commit: BVec::with_capacity_in(2, arena.inner()),
            backing: [0; 16],
            address: AddressValue::new(space_manager.default_space(), 0),
            next_address: None,
            delay_slot: 0,
            alloc: 1,
            state: bvec![in arena.inner(); ConstructState::default(); 75],
        }
    }

    pub fn new(arena: &'z IRBuilderArena, context_db: &ContextDatabase, address: AddressValue, buffer: &[u8]) -> Self {
        let mut backing = [0u8; 16];
        let length = buffer.len().min(backing.len());

        backing[..length].copy_from_slice(&buffer[..length]);

        let context = context_db.get_context(&address);

        Self {
            parse_state: ParserState::Uninitialised,
            context: BVec::from_iter_in(context.iter().map(|v| *v), arena.inner()),
            backing,
            context_commit: BVec::with_capacity_in(2, arena.inner()),
            address,
            next_address: None,
            delay_slot: 0,
            alloc: 1,
            state: bvec![in arena.inner(); ConstructState::default(); 75], // state * param
        }
    }

    pub fn reinitialise(&mut self, _arena: &'z IRBuilderArena, context_db: &ContextDatabase, address: AddressValue, buffer: &[u8]) {
        let mut backing = [0u8; 16];
        let length = buffer.len().min(backing.len());

        backing[..length].copy_from_slice(&buffer[..length]);

        self.parse_state = ParserState::Uninitialised;
        self.backing = backing;
        self.context.clear();
        self.context.extend_from_slice(&context_db.get_context(&address));
        self.context_commit.clear();
        self.address = address;
        self.next_address = None;
        self.delay_slot = 0;
        self.alloc = 1;

        //self.state.truncate(1);
        self.state[0] = Default::default();
    }

    pub fn allocate_operand(&mut self, parent: Option<usize>) -> usize {
        let id = self.alloc;

        if self.state.len() <= id {
            self.state.resize((1 + self.state.len()) * 2, Default::default());
        }

        let op = unsafe { self.state.get_unchecked_mut(id) };

        op.parent = parent;
        op.constructor = None;
        op.handle = None;
        *unsafe { op.resolve.get_unchecked_mut(0) } = None;
        op.offset = 0;
        op.length = 0;

        self.alloc += 1;

        id
    }

    pub(crate) fn set_constructor(&mut self, point: usize, constructor: &'b Constructor) {
        unsafe { self.state.get_unchecked_mut(point) }.constructor = Some(constructor);
    }

    pub(crate) fn set_offset(&mut self, point: usize, offset: usize) {
        unsafe { self.state.get_unchecked_mut(point) }.offset = offset;
    }

    pub(crate) fn point(&self, point: usize) -> &ConstructState<'b> {
        unsafe { self.state.get_unchecked(point) }
    }

    pub(crate) fn point_mut(&mut self, point: usize) -> &mut ConstructState<'b> {
        unsafe { self.state.get_unchecked_mut(point) }
    }

    pub(crate) fn set_handle(&mut self, point: usize, handle: FixedHandle<'b>) {
        unsafe { self.state.get_unchecked_mut(point) }.handle = Some(handle);
    }

    pub fn handle(&self, point: usize) -> Option<&FixedHandle<'b>> {
        self.state[point].handle.as_ref()
    }

    pub fn unchecked_handle(&self, point: usize) -> &FixedHandle<'b> {
        unsafe {
            if let Some(ref handle) = self.state.get_unchecked(point).handle {
                handle
            } else {
                unreachable!()
            }
        }
    }

    pub fn handle_mut(&mut self, point: usize) -> Option<&mut FixedHandle<'b>> {
        self.state[point].handle.as_mut()
    }

    pub fn base_state(&self) -> &ConstructState {
        &self.state[0]
    }

    pub fn base_state_mut(&mut self) -> &mut ConstructState<'b> {
        &mut self.state[0]
    }

    pub fn instruction_bytes(&self, start: usize, size: usize, offset: usize) -> Result<u32, Error> {
        let offset = offset + start;

        if offset >= self.backing.len() {
            return Err(Error::InstructionResolution)
        }

        //debug_assert!(offset < self.backing.len());
        //debug_assert!(offset + size <= self.backing.len());

        let size = (self.backing.len() - offset).min(size);
        let buf = &self.backing[offset..];

        let mut result = 0u32;

        for b in buf.iter().take(size) {
            result = result << 8;
            result |= *b as u32;
        }

        Ok(result)
    }

    pub fn instruction_bits(&self, start: usize, size: usize, offset: usize) -> Result<u32, Error> {
        let offset = offset + (start / 8);
        let start = start % 8;
        let bytes_size = (start + size - 1)/8 + 1;

        //debug_assert!(offset < self.backing.len());
        //debug_assert!(offset + bytes_size <= self.backing.len());

        if offset >= self.backing.len() {
            return Err(Error::InstructionResolution)
        }

        let bytes_size = (self.backing.len() - offset).min(bytes_size);

        let buf = &self.backing[offset..];
        let mut result = 0u32;

        for b in buf.iter().take(bytes_size) {
            result = result << 8;
            result |= *b as u32;
        }

        result = result.checked_shl(8 * (size_of::<u32>() - bytes_size) as u32 + start as u32).unwrap_or(0);
        result = result.checked_shr(8 * size_of::<u32>() as u32 - size as u32).unwrap_or(0);

        Ok(result)
    }

    pub fn context_bytes(&self, start: usize, size: usize) -> u32 {
        let start_off = start / size_of::<u32>();
        let bytes_off = start % size_of::<u32>();

        let unused = size_of::<u32>() - size;
        let mut result = self.context[start_off];

        result = result << (bytes_off as u32 * 8); //.checked_shl(bytes_off as u32 * 8).unwrap_or(0);
        result = result >> (unused as u32 * 8); //.checked_shr(unused as u32 * 8).unwrap_or(0);

        let remaining = (bytes_off + size).checked_sub(size_of::<u32>());

        if remaining.is_some() && remaining.unwrap() > 0 && start_off + 1 < self.context.len() {
            let mut nresult = self.context[start_off + 1];
            let unused = size_of::<u32>() - remaining.unwrap();
            nresult = nresult >> (unused as u32 * 8); //.checked_shr(unused as u32 * 8).unwrap_or(0);
            result |= nresult;
        }

        result
    }

    pub fn context_bits(&self, start: usize, size: usize) -> u32 {
        let start_off = start / (8 * size_of::<u32>());
        let bits_off = start % (8 * size_of::<u32>());

        let unused = 8 * size_of::<u32>() - size;
        let mut result = self.context[start_off];

        result = result.checked_shl(bits_off as u32).unwrap_or(0);
        result = result.checked_shr(unused as u32).unwrap_or(0);

        let remaining = (bits_off + size).checked_sub(8 * size_of::<u32>());

        if remaining.is_some() && remaining.unwrap() > 0 && start_off + 1 < self.context.len() {
            let mut nresult = self.context[start_off + 1];
            let unused = 8 * size_of::<u32>() - remaining.unwrap();
            nresult = nresult.checked_shr(unused as u32).unwrap_or(0);
            result |= nresult;
        }

        result
    }

    pub fn set_context_word(&mut self, num: usize, value: u32, mask: u32) {
        self.context[num] = (self.context[num] & !mask) | (mask & value);
    }

    pub fn add_commit(&mut self, symbol: &'b Symbol, num: usize, mask: u32, point: usize, flow: bool) {
        let set = ContextSet {
            triple: symbol,
            number: num,
            mask,
            value: self.context[num] & mask,
            point,
            flow,
        };
        self.context_commit.push(set);
    }

    pub fn apply_commits<'a, 'c>(&'c mut self, db: &mut ContextDatabase, manager: &'b SpaceManager, symbols: &'b SymbolTable) -> Result<(), Error> {
        if self.context_commit.is_empty() {
            return Ok(())
        }

        let commits = self.context_commit.clone();
        let mut nwalker = ParserWalker::<'b, 'c, 'z>::new(self);

        for commit in commits {
            let symbol = commit.triple;
            let mut address = if let Symbol::Operand { handle_index, .. } = symbol {
                let handle = nwalker.unchecked_handle_ref_via(commit.point, *handle_index); //?
                    //.ok_or_else(|| Error::InvalidHandle)?;
                AddressValue::new(handle.space, handle.offset_offset)
            } else {
                let handle = symbol.fixed_handle(&mut nwalker, manager, symbols)?;
                AddressValue::new(handle.space, handle.offset_offset)
            };

            if address.is_constant() {
                let space = manager.unchecked_space_by_id(address.space());
                let noffset = address.offset() * space.word_size() as u64;
                address = AddressValue::new(space, noffset);
            }

            if commit.flow {
                db.set_context_change_point(address, commit.number, commit.mask, commit.value);
            } else {
                let naddress = address.clone() + 1usize;
                if naddress.offset() < address.offset() {
                    db.set_context_change_point(address, commit.number, commit.mask, commit.value);
                } else {
                    db.set_context_region(address, Some(naddress), commit.number, commit.mask, commit.value);
                }
            }
        }

        Ok(())
    }

    pub fn constructor(&self, point: usize) -> Option<&'b Constructor> {
        self.state[point].constructor
    }

    pub fn unchecked_constructor(&self, point: usize) -> &'b Constructor {
        unsafe { self.state.get_unchecked(point).constructor.unsafe_unwrap() }
    }

    pub fn set_next_address(&mut self, address: AddressValue) {
        self.next_address = Some(address);
    }

    pub fn set_state(&mut self, state: ParserState) {
        self.parse_state = state;
    }
}

pub struct ParserWalker<'b, 'c, 'z> {
    ctx: &'c mut ParserContext<'b, 'z>,

    point: Option<usize>,
    depth: isize,
    breadcrumb: [usize; 32],
}

impl<'b, 'c, 'z> ParserWalker<'b, 'c, 'z> {
    pub fn new(ctx: &'c mut ParserContext<'b, 'z>) -> Self {
        Self {
            ctx,
            point: Some(0),
            depth: 0,
            breadcrumb: [0; 32],
        }
    }

    pub fn context_mut(&mut self) -> &mut ParserContext<'b, 'z> {
        self.ctx
    }

    pub fn base_state(&mut self) {
        self.point = Some(0);
        self.depth = 0;
        self.breadcrumb[0] = 0;
    }

    pub fn is_state(&self) -> bool {
        self.point.is_some()
    }

    pub fn address(&self) -> AddressValue {
        self.ctx.address.clone()
    }

    pub fn unchecked_next_address(&self) -> &AddressValue {
        if let Some(ref address) = self.ctx.next_address {
            address
        } else {
            unreachable!()
        }
    }

    pub fn next_address(&self) -> Option<AddressValue> {
        self.ctx.next_address.clone()
    }

    pub fn length(&self) -> usize {
        self.ctx.point(0).length
    }

    pub fn set_parent_handle(&mut self, handle: FixedHandle<'b>) {
        self.ctx.set_handle(unsafe { self.point.unsafe_unwrap() }, handle);
    }

    pub fn parent_handle_mut(&mut self) -> Option<&mut FixedHandle<'b>> {
        self.ctx.handle_mut(unsafe { self.point.unsafe_unwrap() })
    }

    pub fn handle(&self, index: usize) -> Result<Option<FixedHandle<'b>>, Error> {
        let ph = self.point()
            .ok_or_else(|| Error::InconsistentState)?
            .resolve[index]
            .ok_or_else(|| Error::InconsistentState)?;
        Ok(self.ctx.handle(ph).map(|v| v.clone()))
    }

    pub fn handle_ref(&self, index: usize) -> Option<&FixedHandle<'b>> {
        self.point()
            .and_then(|ctor| ctor.resolve.get(index))
            .and_then(|v| *v)
            .and_then(|hidx| self.ctx.handle(hidx))
    }

    pub fn handle_ref_via(&self, point: usize, index: usize) -> Option<&FixedHandle<'b>> {
        self.ctx.point(point)
            .resolve.get(index)
            .and_then(|v| *v)
            .and_then(|hidx| self.ctx.handle(hidx))
    }

    pub fn unchecked_handle(&self, index: usize) -> FixedHandle<'b> {
        self.unchecked_handle_ref(index).clone()
    }

    pub fn unchecked_handle_ref(&self, index: usize) -> &FixedHandle<'b> {
        let ph = unsafe {
            self.unchecked_point()
                .resolve.get_unchecked(index)
                .unsafe_unwrap()
        };
        self.ctx.unchecked_handle(ph)
    }

    pub fn unchecked_handle_ref_via(&self, point: usize, index: usize) -> &FixedHandle<'b> {
        let ph = unsafe {
            self.ctx.point(point)
                .resolve.get_unchecked(index)
                .unsafe_unwrap()
        };
        self.ctx.unchecked_handle(ph)
    }

    pub fn set_next_address(&mut self, address: AddressValue) {
        self.ctx.set_next_address(address);
    }

    pub fn set_state(&mut self, state: ParserState) {
        self.ctx.set_state(state);
    }

    pub fn set_current_length(&mut self, length: usize) {
        self.ctx.point_mut(unsafe { self.point.unsafe_unwrap() }).length = length;
    }

    pub fn set_delay_slot(&mut self, delay: usize) {
        self.ctx.delay_slot = delay;
    }

    pub fn delay_slot(&self) -> usize {
        self.ctx.delay_slot
    }

    /*
    pub fn calculate_length(&mut self, length: usize, nops: usize) -> Result<(), Error> {
        if let Some(index) = self.point {
            let poff = self.ctx.point(index).offset;
            let length = length + poff;
            let length = (0..nops).try_fold(length, |length, id| {
                let subpt = self.ctx.point(
                    self.ctx.point(index).resolve[id]
                        .ok_or_else(|| Error::InconsistentState)?
                );
                let sub_length = subpt.length + subpt.offset;
                Ok(length.max(sub_length))
            })?;
            self.ctx.point_mut(index).length = length - poff;
        }
        Ok(())
    }
    */

    pub fn calculate_length(&mut self, length: usize, nops: usize) {
        let index = unsafe { self.point.unsafe_unwrap() };
        let poff = self.ctx.point(index).offset;

        let length = length + poff;
        let length = (0..nops).fold(length, |length, id| {
            let subpt = self.ctx.point(unsafe {
                self.ctx.state.get_unchecked(index).resolve.get_unchecked(id).unsafe_unwrap()
            });
            let sub_length = subpt.length + subpt.offset;
            length.max(sub_length)
        });

        self.ctx.point_mut(index).length = length - poff;
    }

    pub fn operand(&self) -> usize {
        *unsafe { self.breadcrumb.get_unchecked(self.depth as usize) }
    }

    pub fn allocate_operand(&mut self, id: usize) -> Result<(), Error> {
        let op = self.ctx.allocate_operand(self.point);

        self.ctx.point_mut(self.point.ok_or_else(|| Error::InconsistentState)?)
            .resolve[id] = Some(op);

        self.breadcrumb[self.depth as usize] += 1;
        self.depth += 1;

        self.point = Some(op);
        self.breadcrumb[self.depth as usize] = 0;

        Ok(())
    }

    pub(crate) fn unchecked_allocate_operand(&mut self, id: usize) {
        let op = self.ctx.allocate_operand(self.point);

        *unsafe {
            self.ctx.point_mut(self.point.unsafe_unwrap()).resolve.get_unchecked_mut(id)
        } = Some(op);

        *unsafe { self.breadcrumb.get_unchecked_mut(self.depth as usize) } += 1 ;
        self.depth += 1;

        self.point = Some(op);
        *unsafe { self.breadcrumb.get_unchecked_mut(self.depth as usize) } = 0;
    }

    pub fn push_operand(&mut self, id: usize) -> Result<(), Error> {
        self.breadcrumb[self.depth as usize] = id + 1;
        self.depth += 1;
        self.point = self.ctx.point(self.point.ok_or_else(|| Error::InconsistentState)?).resolve[id];
        self.breadcrumb[self.depth as usize] = 0;
        Ok(())
    }

    pub(crate) fn unchecked_push_operand(&mut self, id: usize) {
        *unsafe { self.breadcrumb.get_unchecked_mut(self.depth as usize) } = id + 1;
        self.depth += 1;
        self.point = unsafe { *self.ctx.point(self.point.unsafe_unwrap()).resolve.get_unchecked(id) };
        *unsafe { self.breadcrumb.get_unchecked_mut(self.depth as usize) } = 0;
    }

    pub fn pop_operand(&mut self) -> Result<(), Error> {
        self.point = self.ctx.point(self.point.ok_or_else(|| Error::InconsistentState)?).parent;
        self.depth -= 1;
        Ok(())
    }

    pub(crate) fn unchecked_pop_operand(&mut self) {
        self.point = unsafe { self.ctx.point(self.point.unsafe_unwrap()) }.parent;
        self.depth -= 1;
    }

    pub fn offset(&self, offset: Option<usize>) -> usize {
        match offset {
            None => self.unchecked_point().offset, //.ok_or_else(|| Error::InconsistentState)?.offset,
            Some(index) => {
                let op_index = unsafe { self.unchecked_point().resolve.get_unchecked(index).unsafe_unwrap() };
                    //.ok_or_else(|| Error::InconsistentState)?;
                let op = self.ctx.point(op_index);
                op.offset + op.length
            },
        }
    }

    pub fn resolve_with<'d>(&'d mut self, pat: &'b PatternExpression, ctor: &'b Constructor, index: usize, symbols: &'b SymbolTable) -> Result<i64, Error> {
        //resolve_with_aux(self, pat, ctor, index, symbols)
        let mut cur_depth = self.depth;
        let mut point = self.unchecked_point();

        while point.constructor.map(|ct| ct != ctor).unwrap_or(false) {
            if cur_depth == 0 {
                let mut nwalker = ParserWalker::<'b, 'd, 'z>::new(self.context_mut());
                let mut state = ConstructState::default();

                state.constructor = Some(ctor);
                nwalker.point = Some(nwalker.ctx.state.len());
                nwalker.ctx.state.push(state);

                let value = pat.value(&mut nwalker, symbols)?;

                nwalker.ctx.state.pop(); // remove temp. state

                return Ok(value)
            }
            cur_depth -= 1;
            point = self.ctx.point(unsafe { point.parent.unsafe_unwrap() });
        }

        let sym = symbols.unchecked_symbol(ctor.operand(index)); //.ok_or_else(|| Error::InvalidSymbol)?;
        let offset = if sym.offset_base().is_none() { // relative
            point.offset + sym.relative_offset()
        } else {
            self.ctx.point(unsafe { point.resolve.get_unchecked(index).unsafe_unwrap() }).offset //[index].ok_or_else(|| Error::InconsistentState)?).offset
        };

        let mut state = ConstructState::default();
        state.offset = offset;
        state.constructor = Some(ctor);
        state.length = point.length;

        let mut nwalker = ParserWalker::<'b, 'd, 'z>::new(self.context_mut());

        nwalker.point = Some(nwalker.ctx.state.len());
        nwalker.ctx.state.push(state);

        let value = pat.value(&mut nwalker, symbols)?;

        nwalker.ctx.state.pop(); // remove temp. state

        Ok(value)
    }

    pub fn add_commit(&mut self, symbol: &'b Symbol, num: usize, mask: u32, flow: bool) {
        let point = unsafe { self.point.unsafe_unwrap() };
        self.ctx.add_commit(symbol, num, mask, point, flow)
    }

    pub fn apply_commits(&mut self, db: &mut ContextDatabase, manager: &'b SpaceManager, symbols: &'b SymbolTable) -> Result<(), Error> {
        self.ctx.apply_commits(db, manager, symbols)
    }

    pub fn set_context_word(&mut self, num: usize, value: u32, mask: u32) {
        self.ctx.set_context_word(num, value, mask)
    }

    pub fn set_constructor(&mut self, constructor: &'b Constructor) {
        self.ctx.set_constructor(unsafe { self.point.unsafe_unwrap() }, constructor)
    }

    pub fn constructor(&self) -> Result<Option<&'b Constructor>, Error> {
        if self.point.is_none() {
            Ok(None)
        } else {
            Ok(self.ctx.constructor(self.point.ok_or_else(|| Error::InconsistentState)?))
        }
    }

    pub fn unchecked_constructor(&self) -> &'b Constructor {
        self.ctx.unchecked_constructor(unsafe { self.point.unsafe_unwrap() })
    }

    pub fn point(&self) -> Option<&ConstructState<'b>> {
        self.point.map(|index| self.ctx.point(index))
    }

    pub fn unchecked_point(&self) -> &ConstructState<'b> {
        unsafe { self.ctx.point(self.point.unsafe_unwrap()) }
    }

    pub fn set_offset(&mut self, offset: usize) -> Result<(), Error> {
        self.ctx.set_offset(self.point.ok_or_else(|| Error::InconsistentState)?, offset);
        Ok(())
    }

    pub fn context_bytes(&self, offset: usize, size: usize) -> u32 {
        self.ctx.context_bytes(offset, size)
    }

    pub fn context_bits(&self, offset: usize, size: usize) -> u32 {
        self.ctx.context_bits(offset, size)
    }

    pub fn instruction_bytes(&self, offset: usize, size: usize) -> Result<u32, Error> {
        let point = self.ctx.point(self.point.ok_or_else(|| Error::InconsistentState)?);
        Ok(self.ctx.instruction_bytes(offset, size, point.offset)?)
    }

    pub fn unchecked_instruction_bytes(&self, offset: usize, size: usize) -> u32 {
        let point = self.ctx.point(unsafe { self.point.unsafe_unwrap() });
        self.ctx.instruction_bytes(offset, size, point.offset).unwrap()
    }

    pub fn instruction_bits(&self, offset: usize, size: usize) -> Result<u32, Error> {
        let point = self.ctx.point(self.point.ok_or_else(|| Error::InconsistentState)?);
        Ok(self.ctx.instruction_bits(offset, size, point.offset)?)
    }

    pub fn unchecked_instruction_bits(&self, offset: usize, size: usize) -> u32 {
        let point = self.ctx.point(unsafe { self.point.unsafe_unwrap() });
        self.ctx.instruction_bits(offset, size, point.offset).unwrap()
    }
}
