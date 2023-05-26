pub mod sub_table;
pub mod symbol;
pub mod symbol_scope;
pub mod symbol_table;

use std::ops::Range;

pub use sub_table::{Constructor, DecisionNode};
pub use symbol::{FixedHandle, Symbol, SymbolBuilder, SymbolKind};
pub use symbol_scope::SymbolScope;
pub use symbol_table::SymbolTable;

use crate::disassembly::lift::{ArenaVec, IRBuilderArena};
use crate::{Address, AddressSpaceId, AddressValue};

#[derive(Debug, Clone, Hash)]
pub enum Operand<'a, 'z> {
    Address(Address, Option<Range<u32>>),
    Group(ArenaVec<'z, Operand<'a, 'z>>),
    Register(&'a str, Option<Range<u32>>),
    Value(i64, Option<Range<u32>>),
}

#[derive(Debug, Clone, Hash)]
#[repr(transparent)]
pub struct Operands<'a, 'z>(ArenaVec<'z, Operand<'a, 'z>>);

impl<'a, 'z> Operands<'a, 'z> {
    pub fn new(arena: &'z IRBuilderArena) -> Self {
        Self(ArenaVec::new_in(arena.inner()))
    }

    pub fn push(&mut self, operand: impl IntoOperand<'a, 'z>) {
        self.push_with(operand, None)
    }

    pub fn push_with(
        &mut self,
        operand: impl IntoOperand<'a, 'z>,
        bits: impl Into<Option<Range<u32>>>,
    ) {
        self.0.push(operand.into_operand_with(bits.into()));
    }

    pub fn get(&self, index: usize) -> Option<&Operand<'a, 'z>> {
        self.0.get(index)
    }

    pub fn into_iter(self) -> impl ExactSizeIterator<Item = Operand<'a, 'z>> {
        self.0.into_iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn append(&mut self, operands: Self) {
        if let Some(operand) = Operand::from_operands(operands) {
            self.0.push(operand);
        }
    }
}

impl<'a, 'z> Operand<'a, 'z> {
    pub fn varnode(name: &'a str, space: AddressSpaceId, offset: u64) -> Self {
        if space.is_constant() {
            Self::Value(offset as i64, None)
        } else if space.is_default() {
            Self::Address(Address::from_value(offset), None)
        } else {
            Self::Register(name, None)
        }
    }

    pub fn from_operands(mut operands: Operands<'a, 'z>) -> Option<Self> {
        if operands.0.is_empty() {
            None
        } else if operands.0.len() == 1 {
            operands.0.pop()
        } else {
            Some(Self::Group(operands.0))
        }
    }

    pub fn get(&self, index: usize) -> Option<&Operand<'a, 'z>> {
        if let Self::Group(group) = self {
            group.get(index)
        } else {
            None
        }
    }
}

pub trait IntoOperand<'a, 'z> {
    fn into_operand_with(self, bits: Option<Range<u32>>) -> Operand<'a, 'z>;
}

impl<'a, 'z> IntoOperand<'a, 'z> for Operand<'a, 'z> {
    fn into_operand_with(self, _bits: Option<Range<u32>>) -> Operand<'a, 'z> {
        self
    }
}

impl<'a, 'z> IntoOperand<'a, 'z> for i64 {
    fn into_operand_with(self, bits: Option<Range<u32>>) -> Operand<'a, 'z> {
        Operand::Value(self, bits)
    }
}

impl<'a, 'z> IntoOperand<'a, 'z> for &'a str {
    fn into_operand_with(self, bits: Option<Range<u32>>) -> Operand<'a, 'z> {
        Operand::Register(self, bits)
    }
}

impl<'a, 'z> IntoOperand<'a, 'z> for AddressValue {
    fn into_operand_with(self, bits: Option<Range<u32>>) -> Operand<'a, 'z> {
        Operand::Address(Address::from(self), bits)
    }
}

impl<'a, 'z> IntoOperand<'a, 'z> for &'_ AddressValue {
    fn into_operand_with(self, bits: Option<Range<u32>>) -> Operand<'a, 'z> {
        Operand::Address(Address::from(self), bits)
    }
}

#[derive(Debug, Clone, Hash)]
pub enum Token<'a> {
    Address(Address),
    Symbol(&'a str),
    Register(&'a str),
    Value(i64),
}

#[derive(Debug, Clone, Hash)]
#[repr(transparent)]
pub struct Tokens<'a, 'z>(ArenaVec<'z, Token<'a>>);

impl<'a, 'z> Tokens<'a, 'z> {
    pub fn new(arena: &'z IRBuilderArena) -> Self {
        Self(ArenaVec::new_in(arena.inner()))
    }

    pub fn push(&mut self, token: impl Into<Token<'a>>) {
        self.0.push(token.into());
    }

    pub fn get(&self, index: usize) -> Option<&Token<'a>> {
        self.0.get(index)
    }

    pub fn iter<'t>(&'t self) -> impl ExactSizeIterator<Item = &'t Token<'a>> {
        self.0.iter()
    }

    pub fn into_iter(self) -> impl ExactSizeIterator<Item = Token<'a>> + 'z {
        self.0.into_iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn append(&mut self, mut tokens: Self) {
        self.0.append(&mut tokens.0)
    }
}

impl<'a> Token<'a> {
    pub fn varnode(name: &'a str, space: AddressSpaceId, offset: u64) -> Self {
        if space.is_constant() {
            Self::Value(offset as i64)
        } else if space.is_default() {
            Self::Address(Address::from_value(offset))
        } else {
            Self::Register(name)
        }
    }

    pub fn register(name: &'a str) -> Self {
        Self::Register(name)
    }

    pub fn symbol(name: &'a str) -> Self {
        Self::Symbol(name)
    }
}

impl<'a> From<i64> for Token<'a> {
    fn from(v: i64) -> Self {
        Self::Value(v)
    }
}

impl<'a> From<AddressValue> for Token<'a> {
    fn from(addr: AddressValue) -> Self {
        Self::Address(Address::from(addr))
    }
}

impl<'a> From<&'_ AddressValue> for Token<'a> {
    fn from(addr: &AddressValue) -> Self {
        Self::Address(Address::from(addr))
    }
}
