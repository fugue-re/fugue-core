use fxhash::FxHashSet as Set;

use crate::disassembly::symbol::{Symbol, SymbolTable};

#[derive(Debug, Clone, Default)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct SymbolScope {
    pub(super) id: usize,
    pub(super) parent: usize,
    pub(super) tree: Set<usize>,
}

impl SymbolScope {
    pub fn add_symbol(&mut self, symbol: usize) {
        self.tree.insert(symbol);
    }

    pub fn iter(&self) -> impl Iterator<Item=&usize> {
        self.tree.iter()
    }

    pub fn find<'b>(&self, name: &str, table: &'b SymbolTable) -> Option<&'b Symbol> {
        self.tree.iter().find_map(|id| table.symbol(*id).and_then(|sym| {
            if sym.name() == name {
                Some(sym)
            } else {
                None
            }
        }))
    }
}
