use std::cell::Cell;
use std::collections::BTreeMap;

use smallvec::SmallVec;
use ustr::{Ustr, UstrMap};

use crate::lifter::ContextUpdates;
use crate::types::Address;

#[derive(Debug, Clone)]
pub struct FunctionThunkTemplate {
    bytes: SmallVec<[u8; 16]>,
    context: ContextUpdates,
}

impl<T> From<T> for FunctionThunkTemplate
where
    T: AsRef<[u8]>,
{
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl FunctionThunkTemplate {
    pub fn new(bytes: impl AsRef<[u8]>) -> Self {
        Self::new_with(bytes, ContextUpdates::default())
    }

    pub fn new_with(bytes: impl AsRef<[u8]>, context: ContextUpdates) -> Self {
        Self {
            bytes: SmallVec::from_slice(bytes.as_ref()),
            context,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn context(&self) -> &ContextUpdates {
        &self.context
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolEntry {
    address: Address,
    symbol: Option<Ustr>,
    properties: SymbolProperties,
}

impl SymbolEntry {
    pub fn new(
        address: Address,
        symbol: impl Into<Option<Ustr>>,
        properties: SymbolProperties,
    ) -> Self {
        Self {
            address,
            symbol: symbol.into(),
            properties,
        }
    }

    pub fn address(&self) -> Address {
        self.address
    }

    pub fn symbol(&self) -> Option<&Ustr> {
        self.symbol.as_ref()
    }

    pub fn properties(&self) -> SymbolProperties {
        self.properties
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct SymbolProperties: u8 {
        const NONE     = 0b0000_0000;
        const EXTERN   = 0b0000_0001;
        const LOCAL    = 0b0000_0010;
        const FUNCTION = 0b0000_0100;
        const DATA     = 0b0000_1000;
    }
}

#[derive(Debug, Clone, Default)]
pub struct LocalSymbols {
    indices: BTreeMap<usize, Address>,
    sym_to_addr: UstrMap<Address>,
    addr_to_sym: BTreeMap<Address, (Option<Ustr>, Cell<SymbolProperties>)>,
}

impl LocalSymbols {
    pub fn new() -> Self {
        Self {
            indices: BTreeMap::new(),
            sym_to_addr: UstrMap::default(),
            addr_to_sym: BTreeMap::new(),
        }
    }

    pub fn add_symbol(
        &mut self,
        index: usize,
        addr: impl Into<Address>,
        symbol: impl Into<Option<Ustr>>,
    ) {
        Self::add_symbol_with(self, index, addr, symbol, SymbolProperties::LOCAL)
    }

    pub fn add_symbol_with(
        &mut self,
        index: usize,
        addr: impl Into<Address>,
        symbol: impl Into<Option<Ustr>>,
        props: SymbolProperties,
    ) {
        let addr = addr.into();
        let sym = symbol.into();

        let sym = sym.and_then(|sym| sym.is_empty().then(|| None).unwrap_or(Some(sym)));

        self.indices.insert(index, addr);

        self.addr_to_sym
            .insert(addr, (sym, Cell::new(props | SymbolProperties::LOCAL)));

        let Some(sym) = sym else {
            return;
        };

        self.sym_to_addr.insert(sym, addr);
    }

    pub fn symbol(&self, addr: impl Into<Address>) -> Option<(Option<Ustr>, SymbolProperties)> {
        self.addr_to_sym
            .get(&addr.into())
            .map(|(sym, props)| (*sym, props.get()))
    }

    pub fn symbol_properties(&self, addr: impl Into<Address>) -> Option<SymbolProperties> {
        self.addr_to_sym
            .get(&addr.into())
            .map(|(_, props)| props.get())
    }

    pub fn update_symbol_properties(
        &self,
        addr: impl Into<Address>,
        f: impl FnOnce(SymbolProperties) -> SymbolProperties,
    ) -> bool {
        let Some((_, curr_props)) = self.addr_to_sym.get(&addr.into()) else {
            return false;
        };

        curr_props.set(f(curr_props.get()));
        true
    }

    pub fn add_or_update_symbol_properties(
        &mut self,
        addr: impl Into<Address>,
        f: impl FnOnce(SymbolProperties) -> SymbolProperties,
    ) {
        let entry = self
            .addr_to_sym
            .entry(addr.into())
            .or_insert_with(|| (None, Cell::new(SymbolProperties::LOCAL)));

        entry.1.set(f(entry.1.get()));
    }

    pub fn address(&self, sym: impl AsRef<str>) -> Option<Address> {
        let sym = Ustr::from_existing(sym.as_ref())?;
        self.sym_to_addr.get(&sym).copied()
    }

    pub fn properties(&self, sym: impl AsRef<str>) -> Option<SymbolProperties> {
        let sym = Ustr::from_existing(sym.as_ref())?;
        self.sym_to_addr
            .get(&sym)
            .and_then(|addr| self.addr_to_sym.get(addr).map(|(_, props)| props.get()))
    }

    pub fn get_symbol(&self, index: usize) -> Option<Ustr> {
        self.indices
            .get(&index)
            .and_then(|&addr| self.addr_to_sym.get(&addr).and_then(|(sym, _)| *sym))
    }

    pub fn get_address(&self, index: usize) -> Option<Address> {
        self.indices.get(&index).copied()
    }

    pub fn get_properties(&self, index: usize) -> Option<SymbolProperties> {
        self.indices
            .get(&index)
            .and_then(|&addr| self.addr_to_sym.get(&addr).map(|(_, props)| props.get()))
    }

    pub fn get_symbol_with_properties(
        &self,
        index: usize,
    ) -> Option<(Option<Ustr>, SymbolProperties)> {
        self.indices.get(&index).and_then(|&addr| {
            self.addr_to_sym
                .get(&addr)
                .map(|(sym, props)| (*sym, props.get()))
        })
    }

    pub fn contains_address(&self, addr: impl Into<Address>) -> bool {
        self.addr_to_sym.contains_key(&addr.into())
    }

    pub fn contains_symbol(&self, sym: impl AsRef<str>) -> bool {
        let Some(sym) = Ustr::from_existing(sym.as_ref()) else {
            return false;
        };
        self.sym_to_addr.contains_key(&sym)
    }

    pub fn iter<'a>(
        &'a self,
    ) -> impl Iterator<Item = SymbolEntry> + 'a {
        self.addr_to_sym
            .iter()
            .map(|(&addr, (sym, props))| SymbolEntry {
                address: addr,
                symbol: *sym,
                properties: props.get(),
            })
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn len(&self) -> usize {
        self.indices.len()
    }
}

#[derive(Debug, Clone)]
pub struct ExternSymbols {
    base: Address,
    indices: BTreeMap<usize, Address>,
    sym_to_addr: UstrMap<Address>,
    addr_to_sym: BTreeMap<Address, (Option<Ustr>, Cell<SymbolProperties>)>,
    template: FunctionThunkTemplate,
}

impl ExternSymbols {
    pub fn new(base: impl Into<Address>, template: FunctionThunkTemplate) -> Self {
        Self {
            base: base.into(),
            indices: BTreeMap::new(),
            sym_to_addr: UstrMap::default(),
            addr_to_sym: BTreeMap::new(),
            template,
        }
    }

    pub fn add_symbol(
        &mut self,
        index: usize,
        addr: impl Into<Address>,
        symbol: impl Into<Option<Ustr>>,
    ) {
        Self::add_symbol_with(self, index, addr, symbol, SymbolProperties::LOCAL)
    }

    pub fn add_symbol_with(
        &mut self,
        index: usize,
        addr: impl Into<Address>,
        symbol: impl Into<Option<Ustr>>,
        props: SymbolProperties,
    ) {
        let addr = addr.into();
        let sym = symbol.into();

        let sym = sym.and_then(|sym| sym.is_empty().then(|| None).unwrap_or(Some(sym)));

        self.indices.insert(index, addr);

        self.addr_to_sym
            .insert(addr, (sym, Cell::new(props | SymbolProperties::EXTERN)));

        let Some(sym) = sym else {
            return;
        };

        self.sym_to_addr.insert(sym, addr);
    }

    pub fn base(&self) -> Address {
        self.base
    }

    pub fn last_address(&self) -> Address {
        if self.is_empty() {
            self.base()
        } else {
            self.base() + self.size() - 1usize
        }
    }

    pub fn symbol(&self, addr: impl Into<Address>) -> Option<(Option<Ustr>, SymbolProperties)> {
        self.addr_to_sym
            .get(&addr.into())
            .map(|(sym, props)| (*sym, props.get()))
    }

    pub fn symbol_properties(&self, addr: impl Into<Address>) -> Option<SymbolProperties> {
        self.addr_to_sym
            .get(&addr.into())
            .map(|(_, props)| props.get())
    }

    pub fn update_symbol_properties(
        &self,
        addr: impl Into<Address>,
        f: impl FnOnce(SymbolProperties) -> SymbolProperties,
    ) -> bool {
        let Some((_, curr_props)) = self.addr_to_sym.get(&addr.into()) else {
            return false;
        };

        curr_props.set(f(curr_props.get()));
        true
    }

    pub fn address(&self, sym: impl AsRef<str>) -> Option<Address> {
        let sym = Ustr::from_existing(sym.as_ref())?;
        self.sym_to_addr.get(&sym).copied()
    }

    pub fn properties(&self, sym: impl AsRef<str>) -> Option<SymbolProperties> {
        let sym = Ustr::from_existing(sym.as_ref())?;
        self.sym_to_addr
            .get(&sym)
            .and_then(|addr| self.addr_to_sym.get(addr).map(|(_, props)| props.get()))
    }

    pub fn get_symbol(&self, index: usize) -> Option<Ustr> {
        self.indices
            .get(&index)
            .and_then(|&addr| self.addr_to_sym.get(&addr).and_then(|(sym, _)| *sym))
    }

    pub fn get_address(&self, index: usize) -> Option<Address> {
        self.indices.get(&index).copied()
    }

    pub fn get_properties(&self, index: usize) -> Option<SymbolProperties> {
        self.indices
            .get(&index)
            .and_then(|&addr| self.addr_to_sym.get(&addr).map(|(_, props)| props.get()))
    }

    pub fn get_symbol_with_properties(
        &self,
        index: usize,
    ) -> Option<(Option<Ustr>, SymbolProperties)> {
        self.indices.get(&index).and_then(|&addr| {
            self.addr_to_sym
                .get(&addr)
                .map(|(sym, props)| (*sym, props.get()))
        })
    }

    pub fn contains_address(&self, addr: impl Into<Address>) -> bool {
        self.addr_to_sym.contains_key(&addr.into())
    }

    pub fn contains_symbol(&self, sym: impl AsRef<str>) -> bool {
        let Some(sym) = Ustr::from_existing(sym.as_ref()) else {
            return false;
        };
        self.sym_to_addr.contains_key(&sym)
    }

    pub fn iter<'a>(
        &'a self,
    ) -> impl Iterator<Item = SymbolEntry> + 'a {
        self.addr_to_sym
            .iter()
            .map(|(&addr, (sym, props))| SymbolEntry {
                address: addr,
                symbol: *sym,
                properties: props.get(),
            })
    }

    pub fn template(&self) -> &FunctionThunkTemplate {
        &self.template
    }

    pub fn size(&self) -> usize {
        self.indices.len() * self.template.len()
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn len(&self) -> usize {
        self.indices.len()
    }
}
