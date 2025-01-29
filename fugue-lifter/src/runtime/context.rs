use std::collections::BTreeMap as Map;
use std::mem;
use std::ops::{Deref, DerefMut};

use itertools::Itertools;

use crate::runtime::constructor::ConstructorResolver;
use crate::runtime::input::{ContextCommit, FixedHandle};
use crate::runtime::partmap::{BoundKind, PartMap};
use crate::runtime::pattern::PatternExpression;
use crate::runtime::pcode::LiftingContextState;
use crate::runtime::symbol::Symbol;
use crate::runtime::varnode::VarnodeData;
use crate::runtime::wrap_offset;

#[derive(Clone)]
pub struct ContextPreAction {
    pub num: usize,
    pub shift: u32,
    pub mask: u32,
    pub value: PatternExpression,
}

impl ContextPreAction {
    #[inline]
    pub unsafe fn apply<R: ConstructorResolver>(
        &self,
        input: &mut LiftingContextState<'_>,
    ) -> Option<()> {
        let value = (self.value.resolve::<R>(input)? as u32) << self.shift;
        input.input().set_context_word(self.num, value, self.mask);
        Some(())
    }
}

#[derive(Clone)]
pub enum ContextPostActionHandle {
    Operand(usize),
    Symbol(&'static Symbol),
}

#[derive(Clone)]
pub struct ContextPostAction {
    pub handle: ContextPostActionHandle,
    pub num: usize,
    pub mask: u32,
    pub highest: u64,
    pub word_size: u64,
    pub flow: bool,
}

impl ContextPostAction {
    #[inline]
    pub fn extract(&self, input: &mut LiftingContextState<'_>) -> u32 {
        input.input().context.context[self.num] & self.mask
    }

    #[inline]
    pub unsafe fn apply<R: ConstructorResolver>(
        &self,
        input: &mut LiftingContextState<'_>,
        commit: &ContextCommit,
    ) -> Option<()> {
        let FixedHandle {
            space,
            offset_offset: mut offset,
            ..
        } = match self.handle {
            ContextPostActionHandle::Symbol(symbol) => symbol.resolve_handle::<R>(input)?,
            ContextPostActionHandle::Operand(index) => unsafe {
                input
                    .input()
                    .unchecked_operand_via(commit.point as usize, index)
                    .handle
                    .unwrap_or_default()
            },
        };

        if space == 0 {
            offset *= self.word_size
        }

        if self.flow {
            input.inputs.context.set_context_change_point(
                offset,
                self.num,
                self.mask,
                commit.value,
            );
        } else {
            let noffset = wrap_offset(self.highest, offset.wrapping_add(1u64));
            if noffset < offset {
                input.inputs.context.set_context_change_point(
                    offset,
                    self.num,
                    self.mask,
                    commit.value,
                );
            } else {
                input.inputs.context.set_context_region(
                    offset,
                    Some(noffset),
                    self.num,
                    self.mask,
                    commit.value,
                );
            }
        }
        Some(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContextBitRange {
    word: usize,
    start_bit: usize,
    end_bit: usize,
    shift: u32,
    mask: u32,
}

impl AsRef<ContextBitRange> for ContextBitRange {
    fn as_ref(&self) -> &ContextBitRange {
        self
    }
}

impl ContextBitRange {
    pub const fn new(start_bit: usize, end_bit: usize) -> Self {
        let bits = 8 * mem::size_of::<u32>();
        let word = start_bit / bits;
        let start_bit = start_bit - word * bits;
        let end_bit = end_bit - word * bits;
        let shift = (bits - end_bit - 1) as u32;
        let mask = (!0u32) >> (start_bit as u32 + shift);

        Self {
            word,
            start_bit,
            end_bit,
            shift,
            mask,
        }
    }

    pub fn start_bit(&self) -> usize {
        self.start_bit
    }

    pub fn end_bit(&self) -> usize {
        self.end_bit
    }

    pub fn word(&self) -> usize {
        self.word
    }

    pub fn shift(&self) -> u32 {
        self.shift
    }

    pub fn mask(&self) -> u32 {
        self.mask
    }

    #[inline]
    fn get(&self, values: &[u32]) -> u32 {
        values[self.word()].checked_shr(self.shift()).unwrap_or(0) & self.mask()
    }

    #[inline]
    fn set(&self, values: &mut [u32], value: u32) {
        let mut nvalue = values[self.word()];
        nvalue &= !(self.mask().checked_shl(self.shift()).unwrap_or(0));
        nvalue |= (value & self.mask()).checked_shl(self.shift()).unwrap_or(0);
        values[self.word()] = nvalue;
    }

    #[inline]
    fn set_full(&self, values: &mut [u32], masks: &mut [u32], value: u32) {
        let mut nvalue = values[self.word()];
        let nmask = self.mask().checked_shl(self.shift()).unwrap_or(0);

        nvalue &= !(self.mask().checked_shl(self.shift()).unwrap_or(0));
        nvalue |= (value & self.mask()).checked_shl(self.shift()).unwrap_or(0);

        values[self.word()] = nvalue;
        masks[self.word()] |= nmask;
    }
}

#[derive(Debug, Clone)]
pub struct TrackedContext {
    location: VarnodeData,
    value: u32,
}

impl TrackedContext {
    pub fn location(&self) -> &VarnodeData {
        &self.location
    }

    pub fn value(&self) -> u32 {
        self.value
    }
}

#[derive(Debug, Clone)]
pub struct TrackedSet(Vec<TrackedContext>);

impl Default for TrackedSet {
    fn default() -> Self {
        Self(Vec::with_capacity(2))
    }
}

impl Deref for TrackedSet {
    type Target = Vec<TrackedContext>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TrackedSet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Clone)]
pub struct FreeArray {
    values: Vec<u32>,
    masks: Vec<u32>,
}

impl FreeArray {
    pub fn reset(&mut self, size: usize) {
        self.values.resize_with(size, Default::default);
        self.masks.resize_with(size, Default::default);
    }
}

// TODO: merge the arrays?
impl Default for FreeArray {
    fn default() -> Self {
        Self {
            values: Vec::with_capacity(2),
            masks: Vec::with_capacity(2),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContextDatabase {
    size: usize,
    variables: Map<String, ContextBitRange>,
    database: PartMap<u64, FreeArray>,
    trackbase: PartMap<u64, TrackedSet>,
    address_limit: u64,
}

impl ContextDatabase {
    pub fn new(address_limit: u64) -> Self {
        Self {
            size: 0,
            variables: Map::new(),
            database: PartMap::new(Default::default()),
            trackbase: PartMap::new(Default::default()),
            address_limit,
        }
    }

    /*
    pub fn from_translator(translator: &Translator) -> Self {
        let limit = translator.manager().default_space_ref().highest_offset();
        let ctxt = translator.context_database();

        Self {
            size: ctxt.size(),
            address_limit: limit,
            variables: ctxt
                .variables()
                .map(|(k, v)| {
                    (
                        k.to_owned(),
                        ContextBitRange {
                            start_bit: v.start_bit(),
                            end_bit: v.end_bit(),
                            mask: v.mask(),
                            shift: v.shift(),
                            word: v.word(),
                        },
                    )
                })
                .collect(),
            database: PartMap::new({
                let array = ctxt.database().default_value();

                FreeArray {
                    masks: array.masks().to_vec(),
                    values: array.values().to_vec(),
                }
            }),
            trackbase: PartMap::new({
                let base = ctxt
                    .trackbase()
                    .default_value()
                    .iter()
                    .map(|ctxt| TrackedContext {
                        location: VarnodeData {
                            space: ctxt.location().space().index() as _,
                            offset: ctxt.location().offset(),
                            size: ctxt.location().size() as _,
                        },
                        value: ctxt.value(),
                    })
                    .collect();
                TrackedSet(base)
            }),
        }
    }
    */

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn new_tracked_set(&mut self, addr1: u64, addr2: u64) -> &mut TrackedSet {
        let range = self.trackbase.clear_range(addr1, addr2);
        range.clear();
        range
    }

    pub fn tracked_set(&self, address: u64) -> &TrackedSet {
        self.trackbase.get_or_default(address)
    }

    pub fn tracked_default(&self) -> &TrackedSet {
        self.trackbase.default_value()
    }

    pub fn tracked_default_mut(&mut self) -> &mut TrackedSet {
        self.trackbase.default_value_mut()
    }

    pub fn variable(&self, name: impl AsRef<str>) -> Option<&ContextBitRange> {
        self.variables.get(name.as_ref())
    }

    pub fn variable_mut(&mut self, name: impl AsRef<str>) -> Option<&mut ContextBitRange> {
        self.variables.get_mut(name.as_ref())
    }

    pub fn get_variable(&self, name: impl AsRef<str>, address: u64) -> Option<u32> {
        self.variable(name.as_ref())
            .map(|context| context.get(&self.database.get_or_default(address).values))
    }

    pub fn get_variable_by_bits(&self, bits: impl AsRef<ContextBitRange>, address: u64) -> u32 {
        bits.as_ref()
            .get(&self.database.get_or_default(address).values)
    }

    pub fn set_variable(&mut self, name: impl AsRef<str>, address: u64, value: u32) -> Option<()> {
        let context = self.variables.get(name.as_ref())?;
        let num = context.word();
        let mask = context.mask().checked_shl(context.shift()).unwrap_or(0);

        get_region_to_change_point(&mut self.database, address, num, mask, |change| {
            context.set(change, value)
        });

        Some(())
    }

    pub fn set_variable_by_bits(
        &mut self,
        bits: impl AsRef<ContextBitRange>,
        address: u64,
        value: u32,
    ) {
        let bits = bits.as_ref();
        let num = bits.word();
        let mask = bits.mask().checked_shl(bits.shift()).unwrap_or(0);

        get_region_to_change_point(&mut self.database, address, num, mask, |change| {
            bits.set(change, value)
        });
    }

    pub fn set_variable_default(&mut self, name: impl AsRef<str>, value: u32) -> Option<()> {
        let context = self.variables.get(name.as_ref())?;
        let default = self.database.default_value_mut();

        context.set_full(&mut default.values, &mut default.masks, value);

        Some(())
    }

    pub fn set_variable_default_by_bits(&mut self, bits: impl AsRef<ContextBitRange>, value: u32) {
        let default = self.database.default_value_mut();
        bits.as_ref()
            .set_full(&mut default.values, &mut default.masks, value);
    }

    pub fn register_variable(
        &mut self,
        name: impl Into<String>,
        start_bit: usize,
        end_bit: usize,
    ) -> Option<()> {
        if !self.database.is_empty() {
            return None;
        }

        let bit_range = ContextBitRange::new(start_bit, end_bit);
        let word_size = mem::size_of::<u32>();
        let size = start_bit / (8 * word_size) + 1;
        if end_bit / (8 * word_size) + 1 != size {
            return None;
        }

        if size > self.size {
            self.size = size;
            self.database.default_value_mut().reset(size);
        }

        self.variables.insert(name.into(), bit_range);

        Some(())
    }

    pub fn get_context(&self, address: u64) -> &[u32] {
        &self.database.get_or_default(address).values
    }

    pub fn get_context_bounds(&self, address: u64) -> (&[u32], u64, u64) {
        match self.database.bounds(address) {
            BoundKind::None(fa) => (&fa.values, 0, self.address_limit),
            BoundKind::Lower(l, fa) => (&fa.values, *l, self.address_limit),
            BoundKind::Upper(u, fa) => (&fa.values, 0, *u - 1),
            BoundKind::Both(l, u, fa) => {
                let lb = *l;
                let ub = *u - 1;
                (&fa.values, lb, ub)
            }
        }
    }

    pub fn set_context_change_point(
        &mut self,
        // current_address: u64,
        address: u64,
        num: usize,
        mask: u32,
        value: u32,
    ) {
        // self.database.split(current_address);

        get_region_to_change_point(&mut self.database, address, num, mask, |change| {
            let val = &mut change[num];
            *val &= !mask;
            *val |= value;
        })
    }

    pub fn set_context_region(
        &mut self,
        addr1: u64,
        addr2: Option<u64>,
        num: usize,
        mask: u32,
        value: u32,
    ) {
        get_region_for_set(&mut self.database, addr1, addr2, num, mask, |change| {
            change[num] = (change[num] & !mask) | value;
        })
    }

    pub fn set_variable_region(
        &mut self,
        name: impl AsRef<str>,
        addr1: u64,
        addr2: Option<u64>,
        value: u32,
    ) -> Option<()> {
        let context = self.variables.get(name.as_ref())?;
        get_region_for_set(
            &mut self.database,
            addr1,
            addr2,
            context.word(),
            context.mask(),
            |change| context.set(change, value),
        );
        Some(())
    }

    pub fn set_variable_region_by_bits(
        &mut self,
        bits: impl AsRef<ContextBitRange>,
        addr1: u64,
        addr2: Option<u64>,
        value: u32,
    ) {
        let bits = bits.as_ref();
        get_region_for_set(
            &mut self.database,
            addr1,
            addr2,
            bits.word(),
            bits.mask(),
            |change| bits.set(change, value),
        );
    }
}

#[inline(always)]
fn get_region_to_change_point<'a, F>(
    db: &'a mut PartMap<u64, FreeArray>,
    addr: u64,
    num: usize,
    mask: u32,
    mut f: F,
) where
    F: FnMut(&mut Vec<u32>),
{
    use itertools::Position;

    db.split(addr);

    for change in db
        .range_mut(addr..)
        .with_position()
        .take_while(move |pos| match pos {
            (Position::First | Position::Only, _) => true,
            (Position::Middle | Position::Last, (_, fa)) => fa.masks[num] & mask == 0,
        })
        .map(move |pos| match pos {
            (Position::First | Position::Only, (_, fa)) => {
                fa.masks[num] |= mask;
                &mut fa.values
            }
            (Position::Middle | Position::Last, (_, fa)) => &mut fa.values,
        })
    {
        f(change)
    }
}

#[inline(always)]
fn get_region_for_set<'a, F>(
    db: &'a mut PartMap<u64, FreeArray>,
    addr1: u64,
    addr2: Option<u64>,
    num: usize,
    mask: u32,
    mut f: F,
) where
    F: FnMut(&'a mut Vec<u32>),
{
    db.split(addr1);

    let ranges = if let Some(addr2) = addr2 {
        db.split(addr2);
        db.range_mut(addr1..addr2)
    } else {
        db.range_mut(addr1..)
    };

    for change in ranges.map(move |(_, fa)| {
        fa.masks[num] |= mask;
        &mut fa.values
    }) {
        f(change)
    }
}
