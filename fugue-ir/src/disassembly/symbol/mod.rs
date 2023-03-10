use std::borrow::Cow;

#[derive(Debug, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub enum Operand<'a> {
    Address(Address),
    Group(Vec<Operand<'a>>),
    Register(Cow<'a, str>),
    Value(i64),
}

#[derive(Debug, Clone, Hash, serde::Serialize, serde::Deserialize)]
#[repr(transparent)]
pub struct Operands<'a>(Vec<Operand<'a>>);

impl<'a> Operands<'a> {
    pub fn new() -> Self {
        Self(Vec::with_capacity(0))
    }

    pub fn push(&mut self, operand: impl Into<Operand<'a>>) {
        self.0.push(operand.into());
    }

    pub fn get(&self, index: usize) -> Option<&Operand<'a>> {
        self.0.get(index)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn append(&mut self, operands: Self) {
        if let Some(operand) = Operand::from_operands(operands) {
            self.push(operand);
        }
    }
}

impl<'a> Operand<'a> {
    pub fn varnode(name: &'a str, space: AddressSpaceId, offset: u64) -> Self {
        if space.is_constant() {
            Self::Value(offset as i64)
        } else if space.is_default() {
            Self::Address(Address::from_value(offset))
        } else {
            Self::Register(Cow::Borrowed(name))
        }
    }

    pub fn from_operands(mut operands: Operands<'a>) -> Option<Self> {
        if operands.0.is_empty() {
            None
        } else if operands.0.len() == 1 {
            operands.0.pop()
        } else {
            Some(Self::Group(operands.0))
        }
    }

    pub fn get(&self, index: usize) -> Option<&Operand<'a>> {
        if let Self::Group(group) = self {
            group.get(index)
        } else {
            None
        }
    }
}

impl<'a> From<i64> for Operand<'a> {
    fn from(v: i64) -> Self {
        Self::Value(v)
    }
}

impl<'a> From<&'a str> for Operand<'a> {
    fn from(s: &'a str) -> Self {
        Self::Register(Cow::Borrowed(s))
    }
}

impl<'a> From<String> for Operand<'a> {
    fn from(s: String) -> Self {
        Self::Register(Cow::Owned(s))
    }
}

impl<'a> From<AddressValue> for Operand<'a> {
    fn from(addr: AddressValue) -> Self {
        Self::Address(Address::from(addr))
    }
}

impl<'a> From<&'_ AddressValue> for Operand<'a> {
    fn from(addr: &AddressValue) -> Self {
        Self::Address(Address::from(addr))
    }
}

pub mod sub_table;
pub mod symbol;
pub mod symbol_scope;
pub mod symbol_table;

pub use sub_table::{Constructor, DecisionNode};
pub use symbol::{FixedHandle, Symbol, SymbolBuilder, SymbolKind};
pub use symbol_scope::SymbolScope;
pub use symbol_table::SymbolTable;

use crate::{Address, AddressSpaceId, AddressValue};
