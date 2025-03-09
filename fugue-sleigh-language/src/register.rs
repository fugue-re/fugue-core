use std::collections::BTreeMap as Map;
use std::sync::Arc;

use iset::IntervalMap;
use ustr::{Ustr, UstrMap};

use crate::spaces::AddressSpace;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RegisterNames {
    exact: Map<(u64, usize), Ustr>,
    reversed: UstrMap<(u64, usize)>,
    overlaps: IntervalMap<u64, Ustr>,
    space: Arc<AddressSpace>,
}

impl RegisterNames {
    pub fn new(space: Arc<AddressSpace>) -> Self {
        Self {
            exact: Map::default(),
            reversed: UstrMap::default(),
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
            return Some(exact);
        }

        let range = offset..offset + size as u64;
        self.overlaps
            .iter(range.clone())
            .into_iter()
            .find_map(|(r, v)| {
                if r.start <= range.start && r.end >= range.end {
                    Some(v)
                } else {
                    None
                }
            })
    }

    pub fn get_by_name<N>(&self, name: N) -> Option<(&Ustr, u64, usize)>
    where
        N: AsRef<str>,
    {
        self.reversed
            .get_key_value(&name.as_ref().into())
            .map(|(k, vv)| (k, vv.0, vv.1))
    }

    pub fn register_space(&self) -> &Arc<AddressSpace> {
        &self.space
    }

    pub fn range_mapping(&self) -> &Map<(u64, usize), Ustr> {
        &self.exact
    }

    pub fn name_mapping(&self) -> &UstrMap<(u64, usize)> {
        &self.reversed
    }

    // NOTE: sorted iteration order
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&(u64, usize), &Ustr)> {
        self.exact.iter()
    }
}
