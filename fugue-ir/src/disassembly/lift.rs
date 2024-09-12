use std::fmt;
use std::mem::swap;
use std::ops::Deref;
use std::sync::Arc;

use ahash::AHashMap as Map;
use stack_map::StackMap;
use ustr::Ustr;

use crate::address::AddressValue;
use crate::bits;
use crate::disassembly::construct::{ConstructTpl, OpTpl, VarnodeTpl};
use crate::disassembly::symbol::{Constructor, SymbolTable};
use crate::disassembly::Opcode;
use crate::disassembly::VarnodeData;
use crate::disassembly::{Error, ParserContext, ParserWalker};
use crate::float_format::FloatFormat;
use crate::space::AddressSpace;
use crate::space_manager::SpaceManager;
use crate::translator::{Translator, MAX_DELAY_SLOTS};

pub use bumpalo::collections::String as ArenaString;
pub use bumpalo::collections::Vec as ArenaVec;
pub use bumpalo::format as arena_format;
pub use bumpalo::vec as arena_vec;
pub use bumpalo::Bump as Arena;

pub type FloatFormats = Map<usize, Arc<FloatFormat>>;
pub type UserOpStr = Ustr;

const INVALID_LABEL: u64 = 0xdeaded;

#[derive(Debug)]
pub struct PCodeRaw<'z> {
    pub address: AddressValue,
    pub operations: ArenaVec<'z, PCodeData<'z>>,
    pub delay_slots: u8,
    pub length: u8,
}

pub struct PCodeRawFormatter<'a, 'b, 'z> {
    pcode: &'b PCodeRaw<'z>,
    translator: &'a Translator,
}

impl<'a, 'b, 'z> PCodeRawFormatter<'a, 'b, 'z> {
    fn new(pcode: &'b PCodeRaw<'z>, translator: &'a Translator) -> Self {
        Self { pcode, translator }
    }
}

impl<'a, 'b, 'z> fmt::Display for PCodeRawFormatter<'a, 'b, 'z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let len = self.pcode.operations.len();
        if len > 0 {
            for (i, op) in self.pcode.operations.iter().enumerate() {
                write!(
                    f,
                    "{}.{:02}: {}{}",
                    self.pcode.address,
                    i,
                    op.display(self.translator),
                    if i == len - 1 { "" } else { "\n" }
                )?;
            }
            Ok(())
        } else {
            write!(f, "{}.00: Nop", self.pcode.address)
        }
    }
}

impl<'z> PCodeRaw<'z> {
    pub fn display<'a, 'b>(&'b self, translator: &'a Translator) -> PCodeRawFormatter<'a, 'b, 'z> {
        PCodeRawFormatter::new(self, translator)
    }

    pub(crate) fn nop_in(arena: &'z Arena, address: AddressValue, length: usize) -> Self {
        Self {
            address,
            operations: ArenaVec::new_in(arena),
            delay_slots: 0,
            length: length as u8,
        }
    }

    pub fn nop(arena: &'z IRBuilderArena, address: AddressValue, length: usize) -> Self {
        Self::nop_in(arena.inner(), address, length)
    }

    pub fn address(&self) -> AddressValue {
        self.address.clone()
    }

    pub fn operations(&self) -> &[PCodeData<'z>] {
        self.operations.as_ref()
    }

    pub fn delay_slots(&self) -> usize {
        self.delay_slots as usize
    }

    pub fn length(&self) -> usize {
        self.length as usize
    }
}

#[derive(Debug, Copy, Clone)]
pub struct RelativeRecord {
    instruction: usize,
    index: usize,
}

impl RelativeRecord {
    pub fn new(instruction: usize, index: usize) -> Self {
        Self { instruction, index }
    }
}

#[derive(Debug)]
pub struct PCodeData<'z> {
    pub opcode: Opcode,
    //pub inputs_length: u8,
    //pub inputs: [VarnodeData; 3],
    //pub inputs_spill: ArenaVec<'z, VarnodeData>,
    pub output: Option<VarnodeData>,
    //pub input0: VarnodeData, // default
    //pub input1: VarnodeData,
    //pub extra_inputs: ArenaVec<'z, VarnodeData>,
    pub inputs: ArenaVec<'z, VarnodeData>,
}

pub struct PCodeDataFormatter<'a, 'b, 'z> {
    pcode: &'b PCodeData<'z>,
    translator: &'a Translator,
}

impl<'a, 'b, 'z> PCodeDataFormatter<'a, 'b, 'z> {
    fn new(pcode: &'b PCodeData<'z>, translator: &'a Translator) -> Self {
        Self { pcode, translator }
    }
}

impl<'a, 'b, 'z> fmt::Display for PCodeDataFormatter<'a, 'b, 'z> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?}(", self.pcode.opcode)?;
        if let Some(ref output) = self.pcode.output {
            write!(f, "out={}", output.display(self.translator))?;
        }
        if self.pcode.inputs.len() /* self.pcode.inputs_length */ > 0 {
            write!(
                f,
                "{}in=[",
                if self.pcode.output.is_some() {
                    ", "
                } else {
                    ""
                }
            )?;
            for (i, input) in self.pcode.inputs.iter().enumerate() {
                write!(
                    f,
                    "{}{}",
                    if i == 0 { "" } else { ", " },
                    input.display(self.translator)
                )?;
            }
            write!(f, "]")?;
        }
        write!(f, ")")?;
        Ok(())
    }
}

impl<'z> PCodeData<'z> {
    pub fn display<'a, 'b>(&'b self, translator: &'a Translator) -> PCodeDataFormatter<'a, 'b, 'z> {
        PCodeDataFormatter::new(self, translator)
    }

    fn new_in(arena: &'z Arena, opcode: Opcode, inputs_length: usize) -> Self {
        Self {
            opcode,
            //inputs_length: inputs_length as u8,
            //inputs: Default::default(),
            //inputs_spill: if inputs_length > 3 {
            //   arena_vec![in arena; VarnodeData::default(); inputs_length - 3]
            //} else {
            // inputs: arena_vec![in arena; VarnodeData::default(); inputs_length],
            //   ArenaVec::new_in(arena)
            //},
            inputs: ArenaVec::with_capacity_in(inputs_length, arena),
            output: None,
        }
    }

    /*
    pub fn input(&self, index: usize) -> Option<&VarnodeData> {
        if self.inputs_length as usize > index {
            if index < 3 {
                self.inputs.get(index)
            } else {
                self.inputs_spill.get(index - 3)
            }
        } else {
            None
        }
    }

    pub fn input_mut(&mut self, index: usize) -> Option<&mut VarnodeData> {
        if self.inputs_length as usize > index {
            if index < 3 {
                self.inputs.get_mut(index)
            } else {
                self.inputs_spill.get_mut(index - 3)
            }
        } else {
            None
        }
    }

    pub unsafe fn input_unchecked(&self, index: usize) -> &VarnodeData {
        if index < 3 {
            self.inputs.get_unchecked(index)
        } else {
            self.inputs_spill.get_unchecked(index - 3)
        }
    }

    pub unsafe fn input_unchecked_mut(&mut self, index: usize) -> &mut VarnodeData {
        if index < 3 {
            self.inputs.get_unchecked_mut(index)
        } else {
            self.inputs_spill.get_unchecked_mut(index - 3)
        }
    }

    pub fn inputs<'a>(&'a self) -> VarnodeIter<'a, 'z> {
        VarnodeIter { pos: 0, pcode: self }
    }
    */
}

/*
pub struct VarnodeIter<'a, 'z> {
    pos: u8,
    pcode: &'a PCodeData<'z>,
}

impl<'a, 'z> Iterator for VarnodeIter<'a, 'z> {
    type Item = VarnodeData;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.pcode.inputs_length {
            let index = self.pos;
            self.pos += 1;
            Some(unsafe { *self.pcode.input_unchecked(index as usize) })
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        ((self.pcode.inputs_length - self.pos) as usize, Some((self.pcode.inputs_length - self.pos) as usize))
    }
}

impl<'a, 'z> ExactSizeIterator for VarnodeIter<'a, 'z> {
    fn len(&self) -> usize {
        self.pcode.inputs_length as usize
    }
}
*/

pub enum ArenaRef<'a, T: ?Sized + 'a> {
    Borrowed(&'a T),
    Owned(&'a mut T),
}

impl<'a, T> Deref for ArenaRef<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(v) => v,
            Self::Owned(v) => &*v,
        }
    }
}

impl<'a, T> ArenaRef<'a, T> {
    pub fn new_in(arena: &'a IRBuilderArena, v: T) -> Self {
        Self::Owned(arena.alloc(v))
    }

    pub fn borrowed(&self) -> ArenaRef<'_, T> {
        ArenaRef::Borrowed(self.deref())
    }
}

impl<'a, T> ArenaRef<'a, T>
where
    T: Clone,
{
    pub fn cloned<'b>(&self, arena: &'b IRBuilderArena) -> ArenaRef<'b, T> {
        ArenaRef::new_in(arena, self.deref().clone())
    }

    pub fn to_mut(&mut self, arena: &'a IRBuilderArena) -> &mut T {
        match self {
            Self::Borrowed(v) => {
                *self = Self::Owned(arena.alloc(v.clone()));
                if let Self::Owned(ref mut owned) = self {
                    owned
                } else {
                    unreachable!()
                }
            }
            Self::Owned(v) => v,
        }
    }
}

impl<'a, T> fmt::Debug for ArenaRef<'a, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<'a, T> fmt::Display for ArenaRef<'a, T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[repr(transparent)]
pub struct IRBuilderArena(Arena);

impl IRBuilderArena {
    pub fn with_capacity(size: usize) -> Self {
        Self(Arena::with_capacity(size))
    }

    pub fn inner<'z>(&'z self) -> &'z Arena {
        &self.0
    }

    pub fn boxed<'z, T>(&'z self, val: T) -> ArenaRef<'z, T> {
        ArenaRef::new_in(self, val)
    }

    pub fn alloc<'z, T>(&'z self, val: T) -> &'z mut T {
        self.0.alloc(val)
    }

    pub fn alloc_str<'z>(&'z self, val: &str) -> &'z str {
        self.0.alloc_str(val)
    }

    pub fn reset(&mut self) {
        self.0.reset();
    }
}

impl Deref for IRBuilderArena {
    type Target = Arena;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct IRBuilderBase<'b, 'cz> {
    const_space: &'b AddressSpace,
    unique_mask: u64,

    label_base: usize,
    label_count: usize,
    label_refs: ArenaVec<'cz, RelativeRecord>,
    labels: ArenaVec<'cz, u64>,

    manager: &'b SpaceManager,
}

impl<'b, 'cz> IRBuilderBase<'b, 'cz> {
    pub fn empty(
        alloc_inner: &'cz IRBuilderArena,
        manager: &'b SpaceManager,
        unique_mask: u64,
    ) -> Self {
        Self {
            const_space: manager.constant_space_ref(),
            unique_mask,
            label_base: 0,
            label_count: 0,
            labels: ArenaVec::with_capacity_in(16, alloc_inner.inner()),
            label_refs: ArenaVec::with_capacity_in(16, alloc_inner.inner()),
            manager,
        }
    }

    pub fn reinitialise(&mut self) {
        self.label_base = 0;
        self.label_count = 0;
        self.labels.clear();
        self.label_refs.clear();
    }

    /*
    pub(crate) fn arena(&self) -> &'z Arena {
        self.alloc
    }

    pub fn alloc<T>(&self, val: T) -> &'z mut T {
        self.alloc.alloc(val)
    }

    pub fn alloc_vec<T>(&self) -> ArenaVec<'z, T> {
        ArenaVec::new_in(self.alloc)
    }
    */
}

pub struct IRBuilder<'b, 'c, 'cz, 'z> {
    base: &'c mut IRBuilderBase<'b, 'cz>,
    arena: &'z IRBuilderArena,
    unique_offset: u64,
    issued: ArenaVec<'z, PCodeData<'z>>,
    delay_contexts: StackMap<AddressValue, &'c mut ParserContext<'b, 'cz>, MAX_DELAY_SLOTS>,
    walker: ParserWalker<'b, 'c, 'cz>,
}

/*
pub struct IRBuilder<'b, 'c> {
    const_space: &'b AddressSpace,
    unique_mask: u64,
    unique_offset: u64,

    issued: SmallVec<[PCodeData; 16]>,

    label_base: usize,
    label_count: usize,
    label_refs: SmallVec<[RelativeRecord; 16]>,
    labels: SmallVec<[u64; 16]>,

    manager: &'b SpaceManager,
    float_formats: &'b Map<usize, Arc<FloatFormat>>,
    registers: &'b IntervalTree<u64, Arc<str>>,
    user_ops: &'b [Arc<str>],

    delay_contexts: Map<AddressValue, &'c mut ParserContext<'b>>,
    walker: ParserWalker<'b, 'c>,
}
*/

impl<'b, 'c, 'cz, 'z> Deref for IRBuilder<'b, 'c, 'cz, 'z> {
    type Target = &'c mut IRBuilderBase<'b, 'cz>;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl<'b, 'c, 'cz, 'z> IRBuilder<'b, 'c, 'cz, 'z> {
    pub fn new(
        base: &'c mut IRBuilderBase<'b, 'cz>,
        arena: &'z IRBuilderArena,
        walker: ParserWalker<'b, 'c, 'cz>,
        delay_contexts: StackMap<AddressValue, &'c mut ParserContext<'b, 'cz>, MAX_DELAY_SLOTS>,
    ) -> Self {
        base.reinitialise();
        Self {
            unique_offset: (walker.address().offset() & base.unique_mask) << 4,
            issued: ArenaVec::with_capacity_in(16, arena),
            base,
            arena,
            walker,
            delay_contexts,
        }
    }

    #[inline]
    pub fn label_base(&self) -> usize {
        self.base.label_base
    }

    #[inline]
    pub fn walker(&self) -> &ParserWalker<'b, 'c, 'cz> {
        &self.walker
    }

    #[inline]
    pub fn walker_mut(&mut self) -> &mut ParserWalker<'b, 'c, 'cz> {
        &mut self.walker
    }

    #[inline]
    pub fn set_unique_offset(&mut self, offset: u64) {
        self.unique_offset = (offset & self.unique_mask).checked_shl(4).unwrap_or(0);
    }

    pub fn build_empty(
        &mut self,
        ctor: &'b Constructor,
        section_num: Option<usize>,
        symbols: &'b SymbolTable,
    ) -> Result<(), Error> {
        let nops = ctor.operand_count();

        for i in 0..nops {
            let operand = symbols.unchecked_symbol(self.walker.unchecked_constructor().operand(i));
            let symbol = operand.defining_symbol(symbols);
            if symbol.is_none() || !symbol.as_ref().unwrap().is_subtable() {
                continue;
            }

            self.walker.unchecked_push_operand(i); //?;
            if let Some(ctpl) = self
                .walker
                .unchecked_constructor()
                .named_template(unsafe { section_num.unwrap_unchecked() })
            {
                self.build(ctpl, section_num, symbols)?;
            } else {
                self.build_empty(self.walker.unchecked_constructor(), section_num, symbols)?;
            }
            self.walker.unchecked_pop_operand(); //?;
        }
        Ok(())
    }

    #[inline]
    pub fn append_build(
        &mut self,
        op: &'b OpTpl,
        section_num: Option<usize>,
        symbols: &'b SymbolTable,
    ) -> Result<(), Error> {
        let index = op.input(0).offset().real() as usize;
        let operand = symbols.unchecked_symbol(self.walker.unchecked_constructor().operand(index));
        let symbol = operand.defining_symbol(symbols);
        if symbol.is_none() || !symbol.as_ref().unwrap().is_subtable() {
            return Ok(());
        }

        self.walker.unchecked_push_operand(index);
        let constructor = self.walker.unchecked_constructor();
        if let Some(section_num) = section_num {
            if let Some(ctpl) = constructor.named_template(section_num) {
                self.build(ctpl, Some(section_num), symbols)?;
            } else {
                self.build_empty(constructor, Some(section_num), symbols)?;
            }
        } else {
            if let Some(ctpl) = constructor.template() {
                self.build(ctpl, None, symbols)?;
            }
        }
        self.walker.unchecked_pop_operand();
        Ok(())
    }

    pub fn delay_slot(&mut self, symbols: &'b SymbolTable) -> Result<(), Error> {
        let old_unique_offset = self.unique_offset;
        let base_address = self.walker.address();
        let delay_count = self.walker.delay_slot();
        let mut fall_offset = self.walker.length();
        let mut byte_count = 0;

        loop {
            let address = base_address.clone() + fall_offset;
            self.set_unique_offset(address.offset());

            let context = unsafe { self.delay_contexts.remove(&address).unwrap_unchecked() };
            let mut nwalker = ParserWalker::new(context, self.walker.translator());
            let length = nwalker.length();

            // swap out
            swap(&mut self.walker, &mut nwalker);

            self.walker.base_state();

            if let Some(ctpl) = self.walker.unchecked_constructor().template() {
                self.build(ctpl, None, symbols)?;
            }

            fall_offset += length;
            byte_count += length;

            swap(&mut self.walker, &mut nwalker);

            if byte_count >= delay_count {
                break;
            }
        }

        self.unique_offset = old_unique_offset;

        Ok(())
    }

    pub fn generate_location(&mut self, varnode: &'b VarnodeTpl) -> Result<VarnodeData, Error> {
        let space = varnode
            .space()
            .fix_space(&mut self.walker, self.base.manager)
            .ok()
            .and_then(|v| v)
            .ok_or(Error::InvalidHandle)?;

        let size = varnode.size().fix(&mut self.walker, self.base.manager);

        let offset = if space.is_constant() {
            let offset = varnode.offset().fix(&mut self.walker, self.base.manager);
            offset & bits::calculate_mask(size as usize)
        } else if space.is_unique() {
            let offset = varnode.offset().fix(&mut self.walker, self.base.manager);
            offset | self.unique_offset
        } else {
            space.wrap_offset(varnode.offset().fix(&mut self.walker, self.base.manager))
        };

        Ok(VarnodeData::new(space, offset, size as usize))
    }

    pub fn generate_pointer(
        &mut self,
        varnode: &'b VarnodeTpl,
    ) -> Result<(&'b AddressSpace, VarnodeData), Error> {
        let handle_index = varnode
            .offset()
            .handle_index()
            .ok_or(Error::InvalidHandle)?;

        let handle = self
            .walker
            .handle_ref(handle_index)
            .ok_or(Error::InvalidHandle)?;

        let space = handle.offset_space.ok_or(Error::InvalidHandle)?;
        let size = handle.offset_size;

        let offset = if space.is_constant() {
            handle.offset_offset & bits::calculate_mask(size as _)
        } else if space.is_unique() {
            handle.offset_offset | self.unique_offset
        } else {
            space.wrap_offset(handle.offset_offset)
        };

        Ok((handle.space, VarnodeData::new(space, offset, size as _)))
    }

    #[inline]
    pub fn add_label_ref(&mut self, instruction: usize, input: usize) {
        self.base
            .label_refs
            .push(RelativeRecord::new(instruction, input))
    }

    #[inline]
    pub fn add_label(&mut self, id: usize) {
        if id >= self.base.labels.len() {
            self.base.labels.resize(id + 1, INVALID_LABEL);
        }
        self.base.labels[id] = self.issued.len() as u64;
    }

    #[inline]
    pub fn set_label(&mut self, op: &'b OpTpl) {
        // offset of label is not offset, it's label id
        self.add_label(op.input(0).offset().real() as usize + self.label_base());
    }

    pub fn dump(&mut self, op: &'b OpTpl) -> Result<(), Error> {
        let input_count = op.input_count();
        let mut pcode = PCodeData::new_in(self.arena, op.opcode(), input_count);

        for i in 0..input_count {
            let input = op.input(i);
            if input.is_dynamic(&mut self.walker)? {
                let varnode = self.generate_location(input)?;
                let (spc, ptr) = self.generate_pointer(input)?;
                let index = VarnodeData::new(self.const_space, spc.index() as u64, 0);
                self.issued.push(PCodeData {
                    opcode: Opcode::Load,
                    inputs: arena_vec![in self.arena; index, ptr],
                    output: Some(varnode.clone()),
                });
                pcode.inputs.push(varnode);
            } else {
                pcode.inputs.push(self.generate_location(input)?);
            }
        }

        if input_count > 0 && op.input(0).is_relative() {
            pcode.inputs[0].offset += self.label_base() as u64;
            self.add_label_ref(self.issued.len(), 0);
        }

        if let Some(output) = op.output() {
            let outp = self.generate_location(output)?;
            pcode.output = Some(outp);
            self.issued.push(pcode);

            if output.is_dynamic(&mut self.walker)? {
                let (spc, ptr) = self.generate_pointer(output)?;
                let index = VarnodeData::new(self.const_space, spc.index() as u64, 0);
                self.issued.push(PCodeData {
                    opcode: Opcode::Store,
                    inputs: arena_vec![in self.arena; index, ptr, outp],
                    output: None,
                })
            }
        } else {
            self.issued.push(pcode);
        }

        Ok(())
    }

    pub fn build(
        &mut self,
        constructor: &'b ConstructTpl,
        section_num: Option<usize>,
        symbols: &'b SymbolTable,
    ) -> Result<(), Error> {
        let old_base = self.label_base;
        self.base.label_base = self.label_count;
        self.base.label_count += constructor.labels();

        self.base.labels.resize(self.label_count, 0);

        for op in constructor.operations() {
            match op.opcode() {
                Opcode::Build => {
                    self.append_build(op, section_num, symbols)?;
                }
                Opcode::DelaySlot => {
                    self.delay_slot(symbols)?;
                }
                Opcode::Label => {
                    self.set_label(op);
                }
                Opcode::CrossBuild => {
                    return Err(Error::Invariant("unexpected cross-build directive".into()))
                }
                _ => {
                    self.dump(op)?;
                }
            }
        }

        self.base.label_base = old_base;

        Ok(())
    }

    pub fn resolve_relatives(&mut self) -> Result<(), Error> {
        for rel in &self.base.label_refs {
            let varnode = &mut self.issued[rel.instruction].inputs[rel.index];
            let id = varnode.offset();

            match self.base.labels.get(id as usize) {
                // >= self.base.labels.len() or INVALID_LABEL
                None | Some(0xdeaded) => {
                    return Err(Error::Invariant(format!(
                        "no known ways to set label {}",
                        id
                    )));
                }
                Some(label) => {
                    let res = label.wrapping_sub(rel.instruction as u64)
                        & bits::calculate_mask(varnode.size());
                    varnode.offset = res;
                }
            }
        }

        Ok(())
    }

    pub fn emit(self, length: usize) -> PCodeRaw<'z> {
        let mut slf = self;
        slf.walker.base_state();

        let mut operations = ArenaVec::new_in(slf.arena);
        swap(&mut slf.issued, &mut operations);

        PCodeRaw {
            address: slf.walker().address(),
            operations,
            delay_slots: slf.walker.delay_slot() as u8,
            length: length as u8,
        }
    }
}
