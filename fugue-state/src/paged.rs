use std::mem::take;
use std::ops::Range;
use std::sync::Arc;

use fugue_ir::{Address, AddressSpace};
use iset::IntervalMap;
use thiserror::Error;
use ustr::Ustr;

use crate::chunked::{self, ChunkState};
use crate::flat::{self, FlatState};
use crate::traits::{State, StateOps, StateValue};

#[derive(Debug, Error)]
pub enum Error {
    #[error("unmapped virtual address at {address}")]
    UnmappedAddress { address: Address, size: usize },
    #[error("overlapped access from {address} byte access at {size}")]
    OverlappedAccess { address: Address, size: usize },
    #[error("overlapped mapping of {size} bytes from {address}")]
    OverlappedMapping { address: Address, size: usize },
    #[error(transparent)]
    Backing(flat::Error),
    #[error(transparent)]
    Chunked(chunked::Error),
}

impl Error {
    pub fn access(&self) -> (Address, usize) {
        match self {
            Self::UnmappedAddress { address, size }
            | Self::OverlappedAccess { address, size }
            | Self::OverlappedMapping { address, size } => (*address, *size),
            Self::Backing(
                flat::Error::OOBRead { address, size } | flat::Error::OOBWrite { address, size },
            ) => (*address, *size),
            Self::Backing(flat::Error::AccessViolation { address, size, .. }) => {
                (address.into(), *size)
            }
            Self::Chunked(
                chunked::Error::Backing(
                    flat::Error::OOBRead { address, size }
                    | flat::Error::OOBWrite { address, size },
                )
                | chunked::Error::AccessUnmanaged { address, size }
                | chunked::Error::HeapOverflow { address, size },
            ) => (*address, *size),
            Self::Chunked(chunked::Error::Backing(flat::Error::AccessViolation {
                address,
                size,
                ..
            })) => (address.into(), *size),
            _ => panic!("error is not an access violation"),
        }
    }

    fn backing(base: Address, e: flat::Error) -> Self {
        Self::Backing(match e {
            flat::Error::OOBRead { address, size } => flat::Error::OOBRead {
                address: address + base,
                size,
            },
            flat::Error::OOBWrite { address, size } => flat::Error::OOBWrite {
                address: address + base,
                size,
            },
            flat::Error::AccessViolation {
                address,
                access,
                size,
            } => flat::Error::AccessViolation {
                address: address + base,
                access,
                size,
            },
        })
    }
}

#[derive(Debug, Clone)]
pub enum Segment<T: StateValue> {
    Static { name: Ustr, offset: usize },
    Mapping { name: Ustr, backing: ChunkState<T> },
    StaticMapping { name: Ustr, backing: FlatState<T> },
}

#[derive(Debug, Clone)]
pub enum MappingRef<'a, T: StateValue> {
    Dynamic(&'a ChunkState<T>),
    Static(&'a FlatState<T>),
}

#[derive(Debug, Clone)]
pub enum MappingMut<'a, T: StateValue> {
    Dynamic(&'a ChunkState<T>),
    Static(&'a FlatState<T>),
}

impl<T: StateValue> Segment<T> {
    pub fn new<S: AsRef<str>>(name: S, offset: usize) -> Self {
        Self::Static {
            name: Ustr::from(name.as_ref()),
            offset,
        }
    }

    pub fn mapping<S: AsRef<str>>(name: S, mapping: ChunkState<T>) -> Self {
        Self::Mapping {
            name: Ustr::from(name.as_ref()),
            backing: mapping,
        }
    }

    pub fn static_mapping<S: AsRef<str>>(name: S, mapping: FlatState<T>) -> Self {
        Self::StaticMapping {
            name: Ustr::from(name.as_ref()),
            backing: mapping,
        }
    }

    pub fn is_static(&self) -> bool {
        matches!(self, Self::Static { .. })
    }

    pub fn is_mapping(&self) -> bool {
        matches!(self, Self::Mapping { .. })
    }

    pub fn as_mapping(&self) -> Option<MappingRef<T>> {
        match self {
            Self::Mapping { ref backing, .. } => Some(MappingRef::Dynamic(backing)),
            Self::StaticMapping { ref backing, .. } => Some(MappingRef::Static(backing)),
            _ => None,
        }
    }

    pub fn as_mapping_mut(&mut self) -> Option<MappingMut<T>> {
        match self {
            Self::Mapping {
                ref mut backing, ..
            } => Some(MappingMut::Dynamic(backing)),
            Self::StaticMapping {
                ref mut backing, ..
            } => Some(MappingMut::Static(backing)),
            _ => None,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Static { name, .. }
            | Self::Mapping { name, .. }
            | Self::StaticMapping { name, .. } => name,
        }
    }

    pub fn fork(&self) -> Self {
        match self {
            Self::Static { .. } => self.clone(),
            Self::Mapping { name, backing } => Self::Mapping {
                name: name.clone(),
                backing: backing.fork(),
            },
            Self::StaticMapping { name, backing } => Self::StaticMapping {
                name: name.clone(),
                backing: backing.fork(),
            },
        }
    }

    pub fn restore(&mut self, other: &Self) {
        match (self, other) {
            (
                Self::Static { name, offset },
                Self::Static {
                    name: rname,
                    offset: roffset,
                },
            ) => {
                if name != rname || offset != roffset {
                    panic!("attempting to restore segment `{}` at {} from incompatible segment `{}` at {}",
                           name,
                           offset,
                           rname,
                           roffset
                    );
                }
            }
            (
                Self::Mapping { name, backing },
                Self::Mapping {
                    name: rname,
                    backing: rbacking,
                },
            ) => {
                if name != rname
                    || backing.base_address() != rbacking.base_address()
                    || backing.len() != rbacking.len()
                {
                    panic!("attempting to restore segment `{}` at {} of size {} from incompatible segment `{}` at {} of size {}",
                           name,
                           backing.base_address(),
                           backing.len(),
                           rname,
                           rbacking.base_address(),
                           rbacking.len(),
                    );
                }

                backing.restore(rbacking);
            }
            (
                Self::StaticMapping { name, backing },
                Self::StaticMapping {
                    name: rname,
                    backing: rbacking,
                },
            ) => {
                if name != rname || backing.len() != rbacking.len() {
                    panic!("attempting to restore segment `{}` of size {} from incompatible segment `{}` of size {}",
                           name,
                           backing.len(),
                           rname,
                           rbacking.len(),
                    );
                }

                backing.restore(rbacking);
            }
            (slf, oth) => panic!(
                "attempting to restore segment `{}` from segment `{}` which have different kinds",
                slf.name(),
                oth.name()
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PagedState<T: StateValue> {
    segments: IntervalMap<Address, Segment<T>>,
    inner: FlatState<T>,
}

impl<T: StateValue> AsRef<Self> for PagedState<T> {
    #[inline(always)]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T: StateValue> AsMut<Self> for PagedState<T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T: StateValue> PagedState<T> {
    pub fn new(
        mapping: impl IntoIterator<Item = (Range<Address>, Segment<T>)>,
        space: Arc<AddressSpace>,
        size: usize,
    ) -> Self {
        Self::from_parts(mapping, FlatState::new(space, size))
    }

    pub fn from_parts(
        mapping: impl IntoIterator<Item = (Range<Address>, Segment<T>)>,
        backing: FlatState<T>,
    ) -> Self {
        Self {
            segments: IntervalMap::from_iter(mapping.into_iter().map(|(r, s)| (r.start..r.end, s))),
            inner: backing,
        }
    }

    pub fn static_mapping<S, A>(
        &mut self,
        name: S,
        base_address: A,
        size: usize,
    ) -> Result<(), Error>
    where
        S: AsRef<str>,
        A: Into<Address>,
    {
        let base_address = base_address.into();
        let range = base_address..base_address + size; // TODO: error for zero-size

        if self.segments.has_overlap(range.clone()) {
            return Err(Error::OverlappedMapping {
                address: base_address,
                size,
            });
        }

        self.segments.insert(
            range,
            Segment::static_mapping(name, FlatState::new(self.inner.address_space(), size)),
        );
        Ok(())
    }

    pub fn mapping<S, A>(&mut self, name: S, base_address: A, size: usize) -> Result<(), Error>
    where
        S: AsRef<str>,
        A: Into<Address>,
    {
        let base_address = base_address.into();
        let range = base_address..base_address + size; // TODO: error for zero-size

        if self.segments.has_overlap(range.clone()) {
            return Err(Error::OverlappedMapping {
                address: base_address,
                size,
            });
        }

        self.segments.insert(
            range,
            Segment::mapping(
                name,
                ChunkState::new(self.inner.address_space(), base_address, size),
            ),
        );
        Ok(())
    }

    pub fn segments(&self) -> &IntervalMap<Address, Segment<T>> {
        &self.segments
    }

    pub fn mappings(&self) -> impl Iterator<Item = &ChunkState<T>> {
        self.segments.values(..).filter_map(|v| {
            if let Segment::Mapping { backing, .. } = v {
                Some(backing)
            } else {
                None
            }
        })
    }

    pub fn mapping_for<A>(&self, address: A) -> Option<MappingRef<T>>
    where
        A: Into<Address>,
    {
        let address = address.into();
        if address + 1usize < address {
            return None;
        }
        self.segments
            .values_overlap(address)
            .next()
            .and_then(|e| e.as_mapping())
    }

    pub fn mapping_for_mut<A>(&mut self, address: A) -> Option<MappingMut<T>>
    where
        A: Into<Address>,
    {
        let address = address.into();
        if address + 1usize < address {
            return None;
        }
        self.segments
            .values_overlap_mut(address)
            .next()
            .and_then(|e| e.as_mapping_mut())
    }

    pub fn inner(&self) -> &FlatState<T> {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut FlatState<T> {
        &mut self.inner
    }

    pub fn address_space(&self) -> Arc<AddressSpace> {
        self.inner.address_space()
    }

    pub fn address_space_ref(&self) -> &AddressSpace {
        self.inner.address_space_ref()
    }
}

impl<T: StateValue> PagedState<T> {
    #[inline(always)]
    pub fn with_flat<'a, A, F, O: 'a>(
        &'a self,
        address: A,
        access_size: usize,
        f: F,
    ) -> Result<O, Error>
    where
        A: Into<Address>,
        F: FnOnce(&'a FlatState<T>, Address, usize) -> Result<O, Error>,
    {
        let address = address.into();
        if address + 1usize < address {
            return Err(Error::UnmappedAddress {
                address,
                size: access_size,
            });
        }
        if let Some((interval, value)) = self.segments.overlap(address).next() {
            if address + access_size > interval.end {
                // FIXME: checked arith.
                return Err(Error::OverlappedAccess {
                    address,
                    size: access_size,
                });
            }

            match value {
                Segment::Mapping { ref backing, .. } => {
                    let translated = backing
                        .translate_checked(address, access_size)
                        .map_err(Error::Chunked)?;
                    f(
                        backing.inner(),
                        Address::from(translated as u64),
                        access_size,
                    )
                }
                Segment::StaticMapping { ref backing, .. } => {
                    let translated = address - interval.start;
                    f(backing, translated, access_size)
                }
                Segment::Static { offset, .. } => {
                    let address = (address - interval.start) + *offset;
                    f(&self.inner, address, access_size)
                }
            }
        } else {
            Err(Error::UnmappedAddress {
                address,
                size: access_size,
            })
        }
    }

    #[inline(always)]
    pub fn with_flat_mut<'a, A, F, O: 'a>(
        &'a mut self,
        address: A,
        access_size: usize,
        f: F,
    ) -> Result<O, Error>
    where
        A: Into<Address>,
        F: FnOnce(&'a mut FlatState<T>, Address, usize) -> Result<O, Error>,
    {
        let address = address.into();
        if address + 1usize < address {
            return Err(Error::UnmappedAddress {
                address,
                size: access_size,
            });
        }
        if let Some((interval, value)) = self.segments.overlap_mut(address).next() {
            if address + access_size > interval.end {
                return Err(Error::OverlappedAccess {
                    address,
                    size: access_size,
                });
            }
            match value {
                Segment::Mapping {
                    ref mut backing, ..
                } => {
                    let translated = backing
                        .translate_checked(address, access_size)
                        .map_err(Error::Chunked)?;
                    f(
                        backing.inner_mut(),
                        Address::from(translated as u64),
                        access_size,
                    )
                }
                Segment::StaticMapping {
                    ref mut backing, ..
                } => {
                    let translated = address - interval.start;
                    f(backing, translated, access_size)
                }
                Segment::Static { offset, .. } => {
                    let address = (address - interval.start) + *offset;
                    f(&mut self.inner, address, access_size)
                }
            }
        } else {
            Err(Error::UnmappedAddress {
                address,
                size: access_size,
            })
        }
    }

    #[inline(always)]
    pub fn with_flat_from<'a, A, F, O: 'a>(&'a self, address: A, f: F) -> Result<O, Error>
    where
        A: Into<Address>,
        F: FnOnce(&'a FlatState<T>, Address, usize) -> Result<O, Error>,
    {
        let address = address.into();
        if address + 1usize < address {
            return Err(Error::UnmappedAddress { address, size: 1 });
        }
        if let Some((interval, value)) = self.segments.overlap(address).next() {
            match value {
                Segment::Mapping { ref backing, .. } => {
                    // TODO: Chunked::available_len (view whole allocation)
                    let access_size = backing.len(); // FIXME: should this be -1 due to the red-zone?
                    let translated = backing
                        .translate_checked(address, access_size)
                        .map_err(Error::Chunked)?;
                    f(
                        backing.inner(),
                        Address::from(translated as u64),
                        access_size,
                    )
                }
                Segment::StaticMapping { ref backing, .. } => {
                    let max_access_size = usize::from(interval.end - interval.start);
                    let address = address - interval.start;

                    let access_size = max_access_size - usize::from(address);

                    f(backing, address, access_size)
                }
                Segment::Static { offset, .. } => {
                    let max_access_size = usize::from(interval.end - interval.start);
                    let offset_in = address - interval.start;

                    let address = offset_in + *offset;
                    let access_size = max_access_size - usize::from(offset_in);

                    f(&self.inner, address, access_size)
                }
            }
        } else {
            Err(Error::UnmappedAddress { address, size: 1 })
        }
    }

    pub fn view_values_from<A>(&self, address: A) -> Result<&[T], Error>
    where
        A: Into<Address>,
    {
        self.with_flat_from(address, |inner, address, n| {
            inner
                .view_values(address, n)
                .map_err(|e| Error::backing(address, e))
        })
    }

    pub fn segment_bounds<A>(&self, address: A) -> Result<(Range<Address>, &Segment<T>), Error>
    where
        A: Into<Address>,
    {
        let address = address.into();
        if address + 1usize < address {
            return Err(Error::UnmappedAddress {
                address,
                size: 1usize,
            });
        }
        self.segments
            .overlap(address)
            .next()
            .ok_or_else(|| Error::UnmappedAddress {
                address,
                size: 1usize,
            })
    }
}

impl<V: StateValue> State for PagedState<V> {
    type Error = Error;

    fn fork(&self) -> Self {
        Self {
            segments: self.segments.iter(..).map(|(i, v)| (i, v.fork())).collect(),
            inner: self.inner.fork(),
        }
    }

    fn restore(&mut self, other: &Self) {
        self.inner.restore(&other.inner);

        self.segments = take(&mut self.segments)
            .unsorted_into_iter()
            .filter_map(|(i, mut v)| {
                if let Some(vo) = other.segments.get(i.clone()) {
                    v.restore(vo);
                    Some((i, v))
                } else {
                    None
                }
            })
            .collect();
    }
}

impl<V: StateValue> StateOps for PagedState<V> {
    type Value = V;

    fn copy_values<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Error>
    where
        F: Into<Address>,
        T: Into<Address>,
    {
        let from = from.into();
        let to = to.into();

        // TODO: can we avoid the intermediate allocation?

        let vals = self.view_values(from, size)?.to_vec();
        let view = self.view_values_mut(to, size)?;

        for (d, s) in view.iter_mut().zip(vals.into_iter()) {
            *d = s;
        }

        Ok(())
    }

    fn get_values<A>(&self, address: A, values: &mut [Self::Value]) -> Result<(), Error>
    where
        A: Into<Address>,
    {
        let address = address.into();
        let n = values.len();

        self.with_flat(address, n, |inner, address, _size| {
            inner
                .get_values(address, values)
                .map_err(|e| Error::backing(address, e))
        })
    }

    fn view_values<A>(&self, address: A, n: usize) -> Result<&[Self::Value], Error>
    where
        A: Into<Address>,
    {
        let address = address.into();
        self.with_flat(address, n, |inner, address, n| {
            inner
                .view_values(address, n)
                .map_err(|e| Error::backing(address, e))
        })
    }

    fn view_values_mut<A>(&mut self, address: A, n: usize) -> Result<&mut [Self::Value], Error>
    where
        A: Into<Address>,
    {
        let address = address.into();
        self.with_flat_mut(address, n, |inner, address, n| {
            inner
                .view_values_mut(address, n)
                .map_err(|e| Error::backing(address, e))
        })
    }

    fn set_values<A>(&mut self, address: A, values: &[Self::Value]) -> Result<(), Error>
    where
        A: Into<Address>,
    {
        let address = address.into();
        let n = values.len();
        self.with_flat_mut(address, n, |inner, address, _size| {
            inner
                .set_values(address, values)
                .map_err(|e| Error::backing(address, e))
        })
    }

    fn len(&self) -> usize {
        // what to do here? sum of all sizes?
        self.inner.len() + self.mappings().map(|m| m.len()).sum::<usize>()
    }
}
