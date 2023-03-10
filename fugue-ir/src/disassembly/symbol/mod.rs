pub mod sub_table;
pub mod symbol;
pub mod symbol_scope;
pub mod symbol_table;

pub use sub_table::{Constructor, DecisionNode};
pub use symbol::{FixedHandle, Symbol, SymbolBuilder, SymbolKind};
pub use symbol_scope::SymbolScope;
pub use symbol_table::SymbolTable;

use crate::{Address, AddressSpaceId, AddressValue};
use crate::disassembly::lift::{ArenaVec, IRBuilderArena};

#[derive(Debug, Clone, Hash)]
pub enum Operand<'a, 'z> {
    Address(Address),
    Group(ArenaVec<'z, Operand<'a, 'z>>),
    Register(&'a str),
    Value(i64),
}

#[derive(Debug, Clone, Hash)]
#[repr(transparent)]
pub struct Operands<'a, 'z>(ArenaVec<'z, Operand<'a, 'z>>);

impl<'a, 'z> Operands<'a, 'z> {
    pub fn new(arena: &'z IRBuilderArena) -> Self {
        Self(ArenaVec::new_in(arena.inner()))
    }

    pub fn push(&mut self, operand: impl Into<Operand<'a, 'z>>) {
        self.0.push(operand.into());
    }

    pub fn get(&self, index: usize) -> Option<&Operand<'a, 'z>> {
        self.0.get(index)
    }

    pub fn into_iter(self) -> impl ExactSizeIterator<Item=Operand<'a, 'z>> {
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
            self.push(operand);
        }
    }
}

impl<'a, 'z> Operand<'a, 'z> {
    pub fn varnode(name: &'a str, space: AddressSpaceId, offset: u64) -> Self {
        if space.is_constant() {
            Self::Value(offset as i64)
        } else if space.is_default() {
            Self::Address(Address::from_value(offset))
        } else {
            Self::Register(name)
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

impl<'a, 'z> From<i64> for Operand<'a, 'z> {
    fn from(v: i64) -> Self {
        Self::Value(v)
    }
}

impl<'a, 'z> From<&'a str> for Operand<'a, 'z> {
    fn from(s: &'a str) -> Self {
        Self::Register(s)
    }
}

impl<'a, 'z> From<AddressValue> for Operand<'a, 'z> {
    fn from(addr: AddressValue) -> Self {
        Self::Address(Address::from(addr))
    }
}

impl<'a, 'z> From<&'_ AddressValue> for Operand<'a, 'z> {
    fn from(addr: &AddressValue) -> Self {
        Self::Address(Address::from(addr))
    }
}
