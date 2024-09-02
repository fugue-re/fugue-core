use std::borrow::Borrow;
use std::collections::btree_map::{Range, RangeMut};
use std::collections::BTreeMap as Map;
use std::ops::Bound::Excluded;
use std::ops::RangeBounds;

#[derive(Debug, Clone)]
pub enum BoundKind<'a, K, V> {
    None(&'a V),
    Lower(&'a K, &'a V),
    Upper(&'a K, &'a V),
    Both(&'a K, &'a K, &'a V),
}

impl<'a, K, V> BoundKind<'a, K, V> {
    pub fn lower(&self) -> Option<&'a K> {
        match self {
            Self::None(_) | Self::Upper(_, _) => None,
            Self::Lower(k, _) | Self::Both(k, _, _) => Some(k),
        }
    }

    pub fn upper(&self) -> Option<&'a K> {
        match self {
            Self::None(_) | Self::Lower(_, _) => None,
            Self::Upper(k, _) | Self::Both(_, k, _) => Some(k),
        }
    }

    pub fn value(&self) -> &'a V {
        match self {
            Self::None(v) | Self::Lower(_, v) | Self::Upper(_, v) | Self::Both(_, _, v) => v,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PartMap<K: Ord, V> {
    mapping: Map<K, V>,
    default: V,
}

impl<K, V> PartMap<K, V>
where
    K: Clone + Ord,
    V: Clone,
{
    pub fn new(default: V) -> Self {
        Self {
            mapping: Map::new(),
            default,
        }
    }

    pub fn default_value(&self) -> &V {
        &self.default
    }

    pub fn default_value_mut(&mut self) -> &mut V {
        &mut self.default
    }

    pub fn is_empty(&self) -> bool {
        self.mapping.is_empty()
    }

    pub fn bounds<'a>(&self, point: &'a K) -> BoundKind<K, V> {
        let lb = self.mapping.range(..=point).rev().next();
        let ub = self
            .mapping
            .range(point..)
            .find_map(|(k, _)| if k > point { Some(k) } else { None });

        match (lb, ub) {
            (None, None) => BoundKind::None(self.default_value()),
            (Some((l, v)), None) => BoundKind::Lower(l, v),
            (None, Some(u)) => BoundKind::Upper(u, self.default_value()),
            (Some((l, v)), Some(u)) => BoundKind::Both(l, u, v),
        }
    }

    pub fn clear(&mut self) {
        self.mapping = Map::new();
    }

    pub fn clear_range<'a>(&mut self, start: &'a K, end: &'a K) -> &mut V {
        self.split(start);
        self.split(end);

        let keys = self
            .mapping
            .range((Excluded(start), Excluded(end)))
            .map(|(k, _)| k.clone())
            .collect::<Vec<K>>();

        for key in keys {
            self.mapping.remove(&key);
        }

        self.mapping.get_mut(start).unwrap()
    }

    pub fn get<'a>(&self, point: &'a K) -> Option<&V> {
        self.mapping.range(..=point).rev().next().map(|(_, v)| v)
    }

    pub fn get_or_default<'a>(&self, point: &'a K) -> &V {
        self.get(point).unwrap_or_else(|| self.default_value())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.mapping.iter()
    }

    pub fn begin<'a>(&'a self, point: &'a K) -> Range<'a, K, V> {
        self.range(point..)
    }

    pub fn begin_mut<'a>(&'a mut self, point: &'a K) -> RangeMut<'a, K, V> {
        self.range_mut(point..)
    }

    pub fn range<'a, T, R>(&'a self, range: R) -> Range<'a, K, V>
    where
        K: Borrow<T> + 'a,
        R: RangeBounds<T> + 'a,
        T: Ord + ?Sized + 'a,
    {
        self.mapping.range(range)
    }

    pub fn range_mut<'a, T, R>(&'a mut self, range: R) -> RangeMut<'a, K, V>
    where
        K: Borrow<T> + 'a,
        R: RangeBounds<T> + 'a,
        T: Ord + ?Sized + 'a,
    {
        self.mapping.range_mut(range)
    }

    pub fn split<'a>(&'a mut self, at: &'a K) -> &'a V {
        self.split_mut(at)
    }

    pub fn split_mut<'a>(&'a mut self, at: &'a K) -> &'a mut V {
        let value = if let Some(point) = self.mapping.range(..=at).rev().next() {
            if point.0 == at {
                None
            } else {
                Some(point.1.clone())
            }
        } else {
            Some(self.default.clone())
        };

        if let Some(value) = value {
            self.mapping.entry(at.clone()).or_insert(value)
        } else {
            self.mapping.get_mut(&at).unwrap()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn simple_partmap() {
        let mut map = PartMap::<isize, usize>::new(0);

        *map.split_mut(&5) = 5;
        *map.split_mut(&2) = 2;
        *map.split_mut(&3) = 4;
        *map.split_mut(&3) = 3;

        assert_eq!(map.get(&6), Some(&5));
        assert_eq!(map.get(&8), Some(&5));
        assert_eq!(map.get(&4), Some(&3));
        assert_eq!(map.get(&1), None);
    }
}
