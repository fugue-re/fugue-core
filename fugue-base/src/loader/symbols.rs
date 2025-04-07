use std::collections::BTreeMap;

use ustr::{Ustr, UstrMap};

use crate::types::Address;

pub struct LocalSymbols {
    indices: BTreeMap<usize, Address>,
    sym_to_addr: UstrMap<Address>,
    addr_to_sym: BTreeMap<Address, Ustr>,
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
        let addr = addr.into();
        let sym = symbol.into();

        self.indices.insert(index, addr);
        if let Some(sym) = sym {
            self.sym_to_addr.insert(sym, addr);
            self.addr_to_sym.insert(addr, sym);
        }
    }

    pub fn symbol(&self, addr: impl Into<Address>) -> Option<Ustr> {
        self.addr_to_sym.get(&addr.into()).copied()
    }

    pub fn address(&self, sym: impl AsRef<str>) -> Option<Address> {
        let sym = Ustr::from_existing(sym.as_ref())?;
        self.sym_to_addr.get(&sym).copied()
    }

    pub fn get_address(&self, index: usize) -> Option<Address> {
        self.indices.get(&index).copied()
    }

    pub fn get_symbol(&self, index: usize) -> Option<Ustr> {
        self.indices
            .get(&index)
            .and_then(|&addr| self.addr_to_sym.get(&addr).copied())
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

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (Address, Ustr)> + 'a {
        self.addr_to_sym.iter().map(|(addr, sym)| (*addr, *sym))
    }

    pub fn len(&self) -> usize {
        self.indices.len()
    }
}

pub struct ExternSymbols {
    base: Address,
    indices: BTreeMap<usize, Address>,
    sym_to_addr: UstrMap<Address>,
    addr_to_sym: BTreeMap<Address, Ustr>,
    template: &'static [u8],
}

impl ExternSymbols {
    pub fn new(base: impl Into<Address>, template: &'static [u8]) -> Self {
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
        let addr = addr.into();
        let sym = symbol.into();

        self.indices.insert(index, addr);
        if let Some(sym) = sym {
            self.sym_to_addr.insert(sym, addr);
            self.addr_to_sym.insert(addr, sym);
        }
    }

    pub fn base(&self) -> Address {
        self.base
    }

    pub fn symbol(&self, addr: impl Into<Address>) -> Option<Ustr> {
        self.addr_to_sym.get(&addr.into()).copied()
    }

    pub fn address(&self, sym: impl AsRef<str>) -> Option<Address> {
        let sym = Ustr::from_existing(sym.as_ref())?;
        self.sym_to_addr.get(&sym).copied()
    }

    pub fn get_address(&self, index: usize) -> Option<Address> {
        self.indices.get(&index).copied()
    }

    pub fn get_symbol(&self, index: usize) -> Option<Ustr> {
        self.indices
            .get(&index)
            .and_then(|&addr| self.addr_to_sym.get(&addr).copied())
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

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (Address, Ustr)> + 'a {
        self.addr_to_sym.iter().map(|(addr, sym)| (*addr, *sym))
    }

    pub fn template(&self) -> &'static [u8] {
        self.template
    }

    pub fn size(&self) -> usize {
        self.indices.len() * self.template.len()
    }

    pub fn len(&self) -> usize {
        self.indices.len()
    }
}
