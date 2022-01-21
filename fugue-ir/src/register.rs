use std::sync::Arc;
use intervals::Interval;
use intervals::collections::DisjointIntervalTree as IntervalTree;
use fxhash::FxHashMap as Map;
use unsafe_unwrap::UnsafeUnwrap;

use crate::space::AddressSpace;

#[derive(Debug, Clone)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct RegisterNames {
    exact: Map<(u64, usize), Arc<str>>,
    reversed: Map<Arc<str>, (u64, usize)>,
    overlaps: IntervalTree<u64, Arc<str>>,
    space: Arc<AddressSpace>,
}

impl RegisterNames {
    pub fn new(space: Arc<AddressSpace>) -> Self {
        Self {
            exact: Map::default(),
            reversed: Map::default(),
            overlaps: IntervalTree::new(),
            space,
        }
    }

    pub fn insert(&mut self, offset: u64, size: usize, name: Arc<str>) {
        self.exact.insert((offset, size), name.clone());
        self.reversed.insert(name.clone(), (offset, size));
        self.overlaps.insert(offset..=(offset + size as u64 - 1), name);
    }

    pub fn get(&self, offset: u64, size: usize) -> Option<&Arc<str>> {
        if let Some(exact) = self.exact.get(&(offset, size)) {
            return Some(exact)
        }

        let range = Interval::from(offset..=(offset + size as u64 - 1));
        self.overlaps.find_all(&range)
            .into_iter()
            .find_map(|v| if v.interval().start() <= range.start() && v.interval().end() >= range.end() {
                Some(v.value())
            } else {
                None
            })
    }

    pub fn unchecked_get(&self, offset: u64, size: usize) -> &Arc<str> {
        unsafe { self.get(offset, size).unsafe_unwrap() }
    }

    pub fn get_by_name<N>(&self, name: N) -> Option<(&Arc<str>, u64, usize)>
    where N: AsRef<str> {
        self.reversed.get_key_value(name.as_ref()).map(|(k, vv)| (k, vv.0, vv.1))
    }

    pub fn register_space(&self) -> &Arc<AddressSpace> {
        &self.space
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item=(&(u64, usize), &Arc<str>)> {
        self.exact.iter()
    }
}
