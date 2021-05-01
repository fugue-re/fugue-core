use crate::Translator;
use crate::address::Address;
use crate::context::ContextDatabase;
use crate::pattern::PatternExpression;
use crate::error::disassembly as di;
use crate::space_manager::SpaceManager;
use crate::subtable::Constructor;
use crate::symbol_table::{FixedHandle, Symbol, SymbolTable};

use std::cell::RefCell;
use std::fmt;
use std::mem::size_of;
use snafu::OptionExt;

pub mod error;
pub use error::Error;

pub struct InstructionFormatter<'a, 'b> {
    walker: RefCell<ParserWalker<'a, 'b>>,
    symbols: &'a SymbolTable<'a>,
    ctor: &'a Constructor,
}

pub struct MnemonicFormatter<'a, 'b, 'c> {
    inner: &'c InstructionFormatter<'a, 'b>,
}

pub struct OperandFormatter<'a, 'b, 'c> {
    inner: &'c InstructionFormatter<'a, 'b>,
}

impl<'a, 'b> InstructionFormatter<'a, 'b> {
    pub fn new(walker: ParserWalker<'a, 'b>, symbols: &'a SymbolTable, ctor: &'a Constructor) -> Self {
        Self {
            walker: RefCell::new(walker),
            symbols,
            ctor,
        }
    }

    pub fn mnemonic<'c>(&'c self) -> MnemonicFormatter<'a, 'b, 'c> {
        MnemonicFormatter {
            inner: self,
        }
    }

    pub fn operands<'c>(&'c self) -> OperandFormatter<'a, 'b, 'c> {
        OperandFormatter {
            inner: self,
        }
    }
}

impl<'a, 'b> fmt::Display for InstructionFormatter<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.ctor.format_mnemonic(f, &mut self.walker.borrow_mut(), self.symbols)?;
        write!(f, " ")?;
        self.ctor.format_body(f, &mut self.walker.borrow_mut(), self.symbols)?;
        Ok(())
    }
}

impl<'a, 'b, 'c> fmt::Display for MnemonicFormatter<'a, 'b, 'c> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.inner.ctor.format_mnemonic(f, &mut self.inner.walker.borrow_mut(), self.inner.symbols)?;
        Ok(())
    }
}

impl<'a, 'b, 'c> fmt::Display for OperandFormatter<'a, 'b, 'c> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.inner.ctor.format_body(f, &mut self.inner.walker.borrow_mut(), self.inner.symbols)?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct ConstructState<'a> {
    parent: Option<usize>,
    constructor: Option<&'a Constructor>,
    handle: Option<FixedHandle<'a>>,
    resolve: [Option<usize>; 25],
    length: usize,
    offset: usize,
}

impl<'a> ConstructState<'a> {
    pub fn set_parent(&mut self, parent: usize) {
        self.parent = Some(parent);
    }
}

impl<'a> Default for ConstructState<'a> {
    fn default() -> Self {
        Self {
            parent: None,
            constructor: None,
            handle: None,
            resolve: [None; 25],
            length: 0,
            offset: 0,
        }
    }
}

#[derive(Clone)]
pub struct ContextSet<'a> {
    triple: &'a Symbol<'a>,
    point: usize,
    number: usize,
    mask: u32,
    value: u32,
    flow: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ParserState {
    Uninitialised,
    Disassembly,
    PCode,
}

#[derive(Clone)]
pub struct ParserContext<'a> {
    parse_state: ParserState,
    context: Vec<u32>,
    context_commit: Vec<ContextSet<'a>>,

    backing: [u8; 16],

    address: Address<'a>,
    next_address: Option<Address<'a>>,

    delay_slot: usize,

    alloc: usize,
    state: Vec<ConstructState<'a>>,
}

impl<'a> ParserContext<'a> {
    pub fn new<'b>(translator: &'b Translator, address: Address<'a>, buffer: &'b [u8]) -> Self {
        let mut backing = [0u8; 16];
        let length = buffer.len().min(backing.len());
        &mut backing[..length].copy_from_slice(&buffer[..length]);

        let context = translator.context().get_context(&address);

        Self {
            parse_state: ParserState::Uninitialised,
            context: context.clone(),
            backing,
            context_commit: Vec::new(),
            address,
            next_address: None,
            delay_slot: 0,
            alloc: 1,
            state: vec![Default::default(); 75], // state * param
        }
    }

    pub fn allocate_operand(&mut self, parent: Option<usize>) -> usize {
        let id = self.alloc;
        let op = &mut self.state[id];

        op.parent = parent;
        op.constructor = None;

        self.alloc += 1;

        id
    }

    pub fn set_constructor(&mut self, point: usize, constructor: &'a Constructor) {
        self.state[point].constructor = Some(constructor);
    }

    pub fn set_offset(&mut self, point: usize, offset: usize) {
        self.state[point].offset = offset;
    }

    pub fn point(&self, point: usize) -> &ConstructState {
        &self.state[point]
    }

    pub fn point_mut(&mut self, point: usize) -> &mut ConstructState<'a> {
        &mut self.state[point]
    }

    pub fn set_handle(&mut self, point: usize, handle: FixedHandle<'a>) {
        self.state[point].handle = Some(handle);
    }

    pub fn handle(&self, point: usize) -> Option<&FixedHandle> {
        self.state[point].handle.as_ref()
    }

    pub fn handle_mut(&mut self, point: usize) -> Option<&mut FixedHandle<'a>> {
        self.state[point].handle.as_mut()
    }

    pub fn base_state(&self) -> &ConstructState {
        &self.state[0]
    }

    pub fn base_state_mut(&mut self) -> &mut ConstructState<'a> {
        &mut self.state[0]
    }

    pub fn instruction_bytes(&self, start: usize, size: usize, offset: usize) -> u32 {
        let offset = offset + start;

        assert!(offset < self.backing.len());
        assert!(offset + size <= self.backing.len());

        let buf = &self.backing[offset..];

        let mut result = 0u32;

        for i in 0..size {
            result = result.checked_shl(8).unwrap_or(0);
            result |= buf[i] as u32;
        }

        result
    }

    pub fn instruction_bits(&self, start: usize, size: usize, offset: usize) -> u32 {
        let offset = offset + (start / 8);
        let start = start % 8;
        let bytes_size = (start + size - 1)/8 + 1;

        assert!(offset < self.backing.len());
        assert!(offset + bytes_size <= self.backing.len());

        let buf = &self.backing[offset..];
        let mut result = 0u32;

        for i in 0..bytes_size {
            result = result.checked_shl(8).unwrap_or(0);
            result |= buf[i] as u32;
        }

        result = result.checked_shl(8 * (size_of::<u32>() - bytes_size) as u32 + start as u32).unwrap_or(0);
        result = result.checked_shr(8 * size_of::<u32>() as u32 - size as u32).unwrap_or(0);

        result
    }

    pub fn context_bytes(&self, start: usize, size: usize) -> u32 {
        let start_off = start / size_of::<u32>();
        let bytes_off = start % size_of::<u32>();

        let unused = size_of::<u32>() - size;
        let mut result = self.context[start_off];

        result = result.checked_shl(bytes_off as u32 * 8).unwrap_or(0);
        result = result.checked_shr(unused as u32 * 8).unwrap_or(0);

        let remaining = (bytes_off + size).checked_sub(size_of::<u32>());

        if remaining.is_some() && remaining.unwrap() > 0 && start_off + 1 < self.context.len() {
            let mut nresult = self.context[start_off + 1];
            let unused = size_of::<u32>() - remaining.unwrap();
            nresult = nresult.checked_shr(unused as u32 * 8).unwrap_or(0);
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

    pub fn add_commit(&mut self, point: usize, symbol: &'a Symbol, num: usize, mask: u32, flow: bool) {
        let set = ContextSet {
            triple: symbol,
            point,
            number: num,
            mask,
            value: self.context[num] & mask,
            flow,
        };
        self.context_commit.push(set);
    }

    pub fn apply_commits<'b>(&'b mut self, db: &'b mut ContextDatabase, manager: &'a SpaceManager, symbols: &'a SymbolTable) -> Result<(), di::Error> {
        if self.context_commit.is_empty() {
            return Ok(())
        }

        let commits = self.context_commit.clone();
        let mut nwalker = ParserWalker::<'a, 'b>::new(self);

        for commit in commits {
            let symbol = commit.triple;
            let mut address = if let Symbol::Operand { handle_index, .. } = symbol {
                let handle = nwalker.handle(*handle_index)?
                    .with_context(|| di::InvalidHandle)?;
                Address::new(handle.space, handle.offset_offset)
            } else {
                let handle = symbol.fixed_handle(&mut nwalker, manager, symbols)?;
                Address::new(handle.space, handle.offset_offset)
            };

            if address.is_constant() {
                let noffset = address.offset() * address.space().word_size() as u64;
                address = Address::new(address.space(), noffset);
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

    pub fn constructor(&self, point: usize) -> Option<&'a Constructor> {
        self.state[point].constructor
    }

    pub fn set_next_address(&mut self, address: Address<'a>) {
        self.next_address = Some(address);
    }

    pub fn set_state(&mut self, state: ParserState) {
        self.parse_state = state;
    }
}

pub struct ParserWalker<'a, 'b> {
    ctx: &'b mut ParserContext<'a>,

    point: Option<usize>,
    depth: isize,
    breadcrumb: [usize; 32],
}

impl<'a, 'b> ParserWalker<'a, 'b> {
    pub fn new(ctx: &'b mut ParserContext<'a>) -> Self {
        Self {
            ctx,
            point: Some(0),
            depth: 0,
            breadcrumb: [0; 32],
        }
    }

    pub fn context_mut(&'b mut self) -> &'b mut ParserContext<'a> {
        &mut self.ctx
    }

    pub fn base_state(&mut self) {
        self.point = Some(0);
        self.depth = 0;
        self.breadcrumb[0] = 0;
    }

    pub fn is_state(&self) -> bool {
        self.point.is_some()
    }

    pub fn address(&self) -> Address {
        self.ctx.address.clone()
    }

    pub fn next_address(&self) -> Option<Address> {
        self.ctx.next_address.clone()
    }

    pub fn length(&self) -> usize {
        self.ctx.point(0).length
    }

    pub fn set_parent_handle(&mut self, handle: FixedHandle<'a>) -> Result<(), di::Error> {
        if let Some(index) = self.point {
            self.ctx.set_handle(index, handle);
            Ok(())
        } else {
            di::InconsistentState.fail()
        }
    }

    pub fn parent_handle_mut(&mut self) -> Result<Option<&mut FixedHandle<'a>>, di::Error> {
        if let Some(index) = self.point {
            Ok(self.ctx.handle_mut(index))
        } else {
            di::InconsistentState.fail()
        }
    }

    pub fn handle(&self, index: usize) -> Result<Option<FixedHandle>, di::Error> {
        let ph = self.point()
            .with_context(|| di::InconsistentState)?
            .resolve[index]
            .with_context(|| di::InconsistentState)?;
        Ok(self.ctx.handle(ph).map(|v| v.clone()))
    }

    pub fn set_next_address(&mut self, address: Address<'a>) {
        self.ctx.set_next_address(address);
    }

    pub fn set_state(&mut self, state: ParserState) {
        self.ctx.set_state(state);
    }

    pub fn set_current_length(&mut self, length: usize) -> Result<(), di::Error> {
        if let Some(index) = self.point {
            self.ctx.point_mut(index).length = length;
            Ok(())
        } else {
            di::InconsistentState.fail()
        }
    }

    pub fn set_delay_slot(&mut self, delay: usize) {
        self.ctx.delay_slot = delay;
    }

    pub fn delay_slot(&self) -> usize {
        self.ctx.delay_slot
    }

    pub fn calculate_length(&mut self, length: usize, nops: usize) -> Result<(), di::Error> {
        if let Some(index) = self.point {
            let poff = self.ctx.point(index).offset;
            let length = length + poff;
            let length = (0..nops).try_fold(length, |length, id| {
                let subpt = self.ctx.point(
                    self.ctx.point(index).resolve[id]
                        .with_context(|| di::InconsistentState)?
                );
                let sub_length = subpt.length + subpt.offset;
                Ok(length.max(sub_length))
            })?;
            self.ctx.point_mut(index).length = length - poff;
        }
        Ok(())
    }

    pub fn operand(&self) -> usize {
        self.breadcrumb[self.depth as usize]
    }

    pub fn allocate_operand(&mut self, id: usize) -> Result<(), di::Error> {
        let op = self.ctx.allocate_operand(self.point);

        self.ctx.point_mut(self.point.with_context(|| di::InconsistentState)?)
            .resolve[id] = Some(op);

        self.breadcrumb[self.depth as usize] += 1;
        self.depth += 1;

        self.point = Some(op);
        self.breadcrumb[self.depth as usize] = 0;

        Ok(())
    }

    pub fn push_operand(&mut self, id: usize) -> Result<(), di::Error> {
        self.breadcrumb[self.depth as usize] = id + 1;
        self.depth += 1;
        self.point = self.ctx.point(self.point.with_context(|| di::InconsistentState)?).resolve[id];
        self.breadcrumb[self.depth as usize] = 0;
        Ok(())
    }

    pub fn pop_operand(&mut self) -> Result<(), di::Error> {
        self.point = self.ctx.point(self.point.with_context(|| di::InconsistentState)?).parent;
        self.depth -= 1;
        Ok(())
    }

    pub fn offset(&self, offset: Option<usize>) -> Result<usize, di::Error> {
        Ok(match offset {
            None => self.point().with_context(|| di::InconsistentState)?.offset,
            Some(index) => {
                let op_index = self.point().with_context(|| di::InconsistentState)?
                    .resolve[index]
                    .with_context(|| di::InconsistentState)?;
                let op = self.ctx.point(op_index);
                op.offset + op.length
            },
        })
    }

    pub fn resolve_with(&mut self, pat: &PatternExpression, ctor: &'a Constructor, index: usize, symbols: &'a SymbolTable) -> Result<i64, di::Error> {
        let mut cur_depth = self.depth;
        let mut point = self.ctx.point(self.point.with_context(|| di::InconsistentState)?);

        while point.constructor.map(|ct| ct != ctor).unwrap_or(false) {
            if cur_depth == 0 {
                let mut nwalker = ParserWalker::new(self.ctx);
                let mut state = ConstructState::default();

                state.constructor = Some(ctor);
                nwalker.point = Some(nwalker.ctx.state.len());
                nwalker.ctx.state.push(state);

                let value = pat.value(&mut nwalker, symbols)?;

                nwalker.ctx.state.pop(); // remove temp. state
                return Ok(value)
            }
            cur_depth -= 1;
            point = self.ctx.point(point.parent.with_context(|| di::InconsistentState)?);
        }

        let sym = symbols.symbol(ctor.operand(index)).with_context(|| di::InvalidSymbol)?;
        let offset = if sym.offset_base()?.is_none() { // relative
            point.offset + sym.relative_offset()?
        } else {
            self.ctx.point(point.resolve[index].with_context(|| di::InconsistentState)?).offset
        };

        let mut state = ConstructState::default();
        state.offset = offset;
        state.constructor = Some(ctor);
        state.length = point.length;

        let mut nwalker = ParserWalker::new(self.ctx);

        nwalker.point = Some(nwalker.ctx.state.len());
        nwalker.ctx.state.push(state);

        let value = pat.value(&mut nwalker, symbols)?;

        nwalker.ctx.state.pop(); // remove temp. state
        Ok(value)
    }

    pub fn add_commit(&mut self, symbol: &'a Symbol, num: usize, mask: u32, flow: bool) -> Result<(), di::Error> {
        Ok(self.ctx.add_commit(self.point.with_context(|| di::InconsistentState)?, symbol, num, mask, flow))
    }

    pub fn apply_commits<'c>(&'c mut self, db: &'c mut ContextDatabase, manager: &'a SpaceManager, symbols: &'a SymbolTable) -> Result<(), di::Error> {
        self.ctx.apply_commits(db, manager, symbols)
    }

    pub fn set_context_word(&mut self, num: usize, value: u32, mask: u32) {
        self.ctx.set_context_word(num, value, mask)
    }

    pub fn set_constructor(&mut self, constructor: &'a Constructor) -> Result<(), di::Error> {
        Ok(self.ctx.set_constructor(self.point.with_context(|| di::InconsistentState)?, constructor))
    }

    pub fn constructor(&self) -> Result<Option<&'a Constructor>, di::Error> {
        if self.point.is_none() {
            Ok(None)
        } else {
            Ok(self.ctx.constructor(self.point.with_context(|| di::InconsistentState)?))
        }
    }

    pub fn point(&self) -> Option<&ConstructState> {
        self.point.map(|index| self.ctx.point(index))
    }

    pub fn set_offset(&mut self, offset: usize) -> Result<(), di::Error> {
        self.ctx.set_offset(self.point.with_context(|| di::InconsistentState)?, offset);
        Ok(())
    }

    pub fn context_bytes(&self, offset: usize, size: usize) -> u32 {
        self.ctx.context_bytes(offset, size)
    }

    pub fn context_bits(&self, offset: usize, size: usize) -> u32 {
        self.ctx.context_bits(offset, size)
    }

    pub fn instruction_bytes(&self, offset: usize, size: usize) -> Result<u32, di::Error> {
        let point = self.ctx.point(self.point.with_context(|| di::InconsistentState)?);
        Ok(self.ctx.instruction_bytes(offset, size, point.offset))
    }

    pub fn instruction_bits(&self, offset: usize, size: usize) -> Result<u32, di::Error> {
        let point = self.ctx.point(self.point.with_context(|| di::InconsistentState)?);
        Ok(self.ctx.instruction_bits(offset, size, point.offset))
    }
}
