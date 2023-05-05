use std::sync::Arc;
use iset::IntervalMap;
use ahash::AHashMap as Map;
use unsafe_unwrap::UnsafeUnwrap;
use ustr::Ustr;

use crate::space::AddressSpace;

#[derive(Debug, Clone)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct RegisterNames {
    exact: Map<(u64, usize), Ustr>,
    reversed: Map<Ustr, (u64, usize)>,
    overlaps: IntervalMap<u64, Ustr>,
    space: Arc<AddressSpace>,
}

impl RegisterNames {
    pub fn new(space: Arc<AddressSpace>) -> Self {
        Self {
            exact: Map::default(),
            reversed: Map::default(),
            overlaps: IntervalMap::new(),
            space,
        }
    }

    pub fn insert(&mut self, offset: u64, size: usize, name: Ustr) {
        self.exact.insert((offset, size), name.clone());
        self.reversed.insert(name.clone(), (offset, size));
        self.overlaps.insert(offset..offset + size as u64, name);
    }

    pub fn get(&self, offset: u64, size: usize) -> Option<&Ustr> {
        if let Some(exact) = self.exact.get(&(offset, size)) {
            return Some(exact)
        }

        let range = offset..offset + size as u64;
        self.overlaps.iter(range.clone())
            .into_iter()
            .find_map(|(r, v)| if r.start <= range.start && r.end >= range.end {
                Some(v)
            } else {
                None
            })
    }

    pub fn unchecked_get(&self, offset: u64, size: usize) -> &Ustr {
        unsafe { self.get(offset, size).unsafe_unwrap() }
    }

    pub fn get_by_name<N>(&self, name: N) -> Option<(&Ustr, u64, usize)>
    where N: AsRef<str> {
        self.reversed.get_key_value(&name.as_ref().into()).map(|(k, vv)| (k, vv.0, vv.1))
    }

    pub fn register_space(&self) -> &Arc<AddressSpace> {
        &self.space
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item=(&(u64, usize), &Ustr)> {
        self.exact.iter()
    }
}
