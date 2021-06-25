use std::borrow::Borrow;
use std::collections::BTreeMap as Map;
use std::mem;
use std::ops::{Deref, DerefMut};

use itertools::Itertools;

use crate::address::Address;
use crate::disassembly::partmap::{BoundKind, PartMap};
use crate::disassembly::VarnodeData;

#[derive(Debug, Clone)]
pub struct ContextBitRange {
    word: usize,
    start_bit: usize,
    end_bit: usize,
    shift: u32,
    mask: u32,
}

impl ContextBitRange {
    pub fn new(start_bit: usize, end_bit: usize) -> Self {
        let bits = 8 * mem::size_of::<u32>();
        let word = start_bit / bits;
        let start_bit = start_bit - word * bits;
        let end_bit = end_bit - word * bits;
        let shift = (bits - end_bit - 1) as u32;
        let mask = (!0u32).checked_shr(start_bit as u32 + shift).unwrap_or(0);

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

    fn get(&self, values: &[u32]) -> u32 {
        values[self.word()].checked_shr(self.shift()).unwrap_or(0) & self.mask()
    }

    fn set(&self, values: &mut [u32], value: u32) {
        let mut nvalue = values[self.word()];
        nvalue &= !(self.mask().checked_shl(self.shift()).unwrap_or(0));
        nvalue |= (value & self.mask()).checked_shl(self.shift()).unwrap_or(0);
        values[self.word()] = nvalue;
    }
}

#[derive(Debug, Clone)]
pub struct TrackedContext {
    location: VarnodeData,
    value: u32,
}

#[derive(Debug, Clone)]
pub struct TrackedSet(Vec<TrackedContext>);

impl Default for TrackedSet {
    fn default() -> Self {
        Self(Vec::new())
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
struct FreeArray {
    values: Vec<u32>,
    masks: Vec<u32>,
}

impl FreeArray {
    pub fn reset(&mut self, size: usize) {
        self.values.resize_with(size, Default::default);
        self.masks.resize_with(size, Default::default);
    }
}

impl Default for FreeArray {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            masks: Vec::new(),
        }
    }
}

/*
pub enum CachedDatabaseContext {
    Plain(Box<ContextDatabase>),
    Cached {
        lower: u64,
        upper: u64,
        space: Arc<AddressSpace>,
        inner: OwningRef<Box<ContextDatabase>, Vec<u64>>,
    },
}

impl CachedDatabaseContext {
    pub fn has_cached(&self) -> bool {
        match self {
            Self::Plain(..) => true,
            _ => false,
        }
    }
}

pub struct ContextCache {
    database: CachedDatabaseContext,
    allow_set: bool,
}

impl CachedDatabaseContext {
    pub fn get_cached(&mut self, address: &Address) -> Vec<u64> {
        let mut ret = Vec::new();
        replace_with_or_abort(self, |db| match db {
            CachedDatabaseContext::Cached {
                ref space,
                upper,
                lower,
                ref inner,
            } if address.space() == space.as_ref()
                && lower <= address.offset()
                && upper >= address.offset() =>
            {
                ret.extend_from_slice(&*inner);
                db
            },
            CachedDatabaseContext::Cached { inner, .. } => {
                let mut lower = 0;
                let mut upper = 0;
                let db = inner.into_owner();
                let inner = OwningRef::new(db).map(|db| {
                    let (v, lb, ub) = db.get_context_bounds(address);
                    lower = lb;
                    upper = ub;
                    v
                });

                ret.extend_from_slice(&*inner);

                CachedDatabaseContext::Cached {
                    lower,
                    upper,
                    space: address.space_cloned(),
                    inner,
                }
            }
            CachedDatabaseContext::Plain(db) => {
                let mut lower = 0;
                let mut upper = 0;
                let inner = OwningRef::new(db).map(|db| {
                    let (v, lb, ub) = db.get_context_bounds(address);
                    lower = lb;
                    upper = ub;
                    v
                });

                ret.extend_from_slice(&*inner);

                CachedDatabaseContext::Cached {
                    lower,
                    upper,
                    space: address.space_cloned(),
                    inner,
                }
            }
        });
        ret
    }

    /*
    pub fn set_cached(&mut self, address: Address, ret: &mut Vec<u64>) {
        replace_with_or_abort(self, |db| match db {
            CachedDatabaseContext::Cached {
                ref space,
                upper,
                lower,
                ref inner,
            } if address.space() == space.as_ref()
                && lower <= address.offset()
                && upper >= address.offset() =>
            {
                ret.extend_from_slice(&*inner);
                db
            }
            CachedDatabaseContext::Cached { inner, .. } => {
                let mut lower = 0;
                let mut upper = 0;
                let db = inner.into_owner();
                let ndb = OwningRef::new(db).map(|db| {
                    let (v, lb, ub) = db.get_context_bounds(address.clone());
                    lower = lb;
                    upper = ub;
                    v
                });
                ret.extend_from_slice(&*ndb);

                CachedDatabaseContext::Cached {
                    lower,
                    upper,
                    space: address.space_cloned(),
                    inner: ndb,
                }
            }
            CachedDatabaseContext::Plain(db) => {
                let mut lower = 0;
                let mut upper = 0;
                let ndb = OwningRef::new(db).map(|db| {
                    let (v, lb, ub) = db.get_context_bounds(address.clone());
                    lower = lb;
                    upper = ub;
                    v
                });
                ret.extend_from_slice(&*ndb);

                CachedDatabaseContext::Cached {
                    lower,
                    upper,
                    space: address.space_cloned(),
                    inner: ndb,
                }
            }
        })
    }
    */
}

impl ContextCache {
    pub fn new(database: ContextDatabase) -> Self {
        Self {
            database: CachedDatabaseContext::Plain(Box::new(database)),
            allow_set: false,
        }
    }

    pub fn toggle_set(&mut self, allow: bool) {
        self.allow_set = allow;
    }

    pub fn context(&mut self, address: &Address) -> Vec<u64> {
        self.database.get_cached(address).to_vec()
    }

    /*
    pub fn set_context(&mut self, address: Address, num: usize, mask: u64, value: u64) {
        if !self.allow_set {
            return;
        }
        self.database.set_cached(&address, num, mask, value)
    }
    */
}
*/

#[derive(Debug, Clone)]
pub struct ContextDatabase {
    size: usize,
    variables: Map<String, ContextBitRange>,
    database: PartMap<Address, FreeArray>,
    trackbase: PartMap<Address, TrackedSet>,
}

impl ContextDatabase {
    pub fn new() -> Self {
        Self {
            size: 0,
            variables: Map::new(),
            database: PartMap::new(Default::default()),
            trackbase: PartMap::new(Default::default()),
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn new_tracked_set(&mut self, addr1: Address, addr2: Address) -> &mut TrackedSet {
        let range = self.trackbase.clear_range(&addr1, &addr2);
        range.clear();
        range
    }

    pub fn tracked_set(&self, address: Address) -> &TrackedSet {
        self.trackbase.get_or_default(&address)
    }

    pub fn tracked_default(&self) -> &TrackedSet {
        self.trackbase.default_value()
    }

    pub fn tracked_default_mut(&mut self) -> &mut TrackedSet {
        self.trackbase.default_value_mut()
    }

    pub fn variable<S: Borrow<str>>(&self, name: S) -> Option<&ContextBitRange> {
        self.variables.get(name.borrow())
    }

    pub fn variable_mut<S: Borrow<str>>(&mut self, name: S) -> Option<&mut ContextBitRange> {
        self.variables.get_mut(name.borrow())
    }

    pub fn get_variable<S: Borrow<str>>(&self, name: S, address: Address) -> Option<u32> {
        self.variable(name.borrow())
            .map(|context| context.get(&self.database.get_or_default(&address).values))
    }

    pub fn set_variable<S: Borrow<str>>(
        &mut self,
        name: S,
        address: Address,
        value: u32,
    ) -> Option<()> {
        let context = self.variables.get(name.borrow())?;
        let num = context.word();
        let mask = context.mask().checked_shl(context.shift()).unwrap_or(0);

        get_region_to_change_point(&mut self.database, address, num, mask, |change| {
            context.set(change, value)
        });

        Some(())
    }

    pub fn set_variable_default<S: Borrow<str>>(
        &mut self,
        name: S,
        value: u32,
    ) -> Option<()> {
        let context = self.variables.get(name.borrow())?;
        let default = self.database.default_value_mut();

        context.set(&mut default.values, value);

        Some(())
    }

    pub fn register_variable<S: Borrow<str>>(
        &mut self,
        name: S,
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

        self.variables.insert(name.borrow().to_owned(), bit_range);

        Some(())
    }

    pub fn get_context(&self, address: &Address) -> &Vec<u32> {
        &self.database.get_or_default(address).values
    }

    pub fn get_context_bounds(&self, address: &Address) -> (&Vec<u32>, u64, u64) {
        match self.database.bounds(address) {
            BoundKind::None(fa) => (&fa.values, 0, address.space().highest_offset()),
            BoundKind::Lower(l, fa) => {
                if l.space() == address.space() {
                    (&fa.values, l.offset(), address.space().highest_offset())
                } else {
                    (&fa.values, 0, address.space().highest_offset())
                }
            }
            BoundKind::Upper(u, fa) => {
                if u.space() == address.space() {
                    (&fa.values, 0, u.offset() - 1)
                } else {
                    (&fa.values, 0, address.space().highest_offset())
                }
            }
            BoundKind::Both(l, u, fa) => {
                let lb = if l.space() == address.space() {
                    l.offset()
                } else {
                    0
                };
                let ub = if u.space() == address.space() {
                    u.offset() - 1
                } else {
                    address.space().highest_offset()
                };
                (&fa.values, lb, ub)
            }
        }
    }

    pub fn set_context_change_point(
        &mut self,
        address: Address,
        num: usize,
        mask: u32,
        value: u32,
    ) where {
        get_region_to_change_point(&mut self.database, address, num, mask, |change| {
            let val = &mut change[num];
            *val &= !mask;
            *val |= value;
        })
    }

    pub fn set_context_region(
        &mut self,
        addr1: Address,
        addr2: Option<Address>,
        num: usize,
        mask: u32,
        value: u32,
    ) {
        get_region_for_set(&mut self.database, addr1, addr2, num, mask, |change| {
            change[num] = (change[num] & !mask) | value;
        })
    }

    pub fn set_variable_region<S: Borrow<str>>(
        &mut self,
        name: S,
        addr1: Address,
        addr2: Option<Address>,
        value: u32,
    ) -> Option<()> {
        let context = self.variables.get(name.borrow())?;
        get_region_for_set(
            &mut self.database,
            addr1,
            addr2,
            context.word(),
            context.mask(),
            |change| {
                context.set(change, value)
            }
        );
        Some(())
    }
}

fn get_region_to_change_point<'a, F>(
    db: &'a mut PartMap<Address, FreeArray>,
    addr: Address,
    num: usize,
    mask: u32,
    mut f: F
) where F: FnMut(&mut Vec<u32>) {
    use itertools::Position;

    db.split(&addr);

    for change in db.range_mut(addr..)
        .with_position()
        .take_while(move |pos| match pos {
            Position::First(_) | Position::Only(_) => true,
            Position::Middle((_, fa)) | Position::Last((_, fa)) => fa.masks[num] & mask == 0,
        })
        .map(move |pos| match pos {
            Position::First((_, fa)) | Position::Only((_, fa)) => {
                fa.masks[num] |= mask;
                &mut fa.values
            }
            Position::Middle((_, fa)) | Position::Last((_, fa)) => &mut fa.values,
        }) {
        f(change)
    }
}

fn get_region_for_set<'a, F>(
    db: &'a mut PartMap<Address, FreeArray>,
    addr1: Address,
    addr2: Option<Address>,
    num: usize,
    mask: u32,
    mut f: F
) where F: FnMut(&'a mut Vec<u32>) {
    db.split(&addr1);

    let ranges = if let Some(addr2) = addr2 {
        db.split(&addr2);
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
