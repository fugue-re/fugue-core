//! translation graph module
//! 
//! kind of like a CFG but for translation blocks
//! the graph will not be normalized (if it were it would be a CFG)
//! but it is good enough for caching purposes. 
//! uses petgraph::Graph for now, but petgraph::CSR may be more
//! efficient given the sparse nature of CFGs
use std::sync::Arc;
use fugue_ir::disassembly::IRBuilderArena;
use nohash_hasher::IntMap;

use petgraph::stable_graph::StableGraph;
use petgraph::graph::{
    NodeIndex,
};
// use petgraph::{
//     data::DataMap,
//     data::DataMapMut,
//     visit::NodeIndexable,
//     visit::NodeRef,
// };

use fugue_ir::Address;
use fugue_core::lifter::Lifter;
use crate::context::manager::ContextManager;
use super::tblock::{
    TranslationBlock,
    TranslationError,
    TranslatedInsn,
};

/// translation graph
/// populated with translation blocks, edges will be formed
/// as the engine continues execution
pub struct TranslationGraph<'a> {
    idx_map: IntMap<u64, NodeIndex>,
    graph: StableGraph<Arc<TranslationBlock<'a>>, ()>,
    current_base: Option<Address>,
    irb: &'a mut IRBuilderArena,
}

impl<'a> TranslationGraph<'a> {
    pub fn new_with<'z: 'a>(irb: &'z mut IRBuilderArena) -> Self {
        Self {
            idx_map: IntMap::default(),
            graph: StableGraph::new(),
            current_base: None,
            irb,
        }
    }

    /// add a translation block to the graph (without edges)
    /// note: adding an empty translation block will cause panic
    pub fn add_block<'z>(&mut self, block: TranslationBlock<'z>) -> ()
        where 'z : 'a
    {
        let base_addr_u64 = block.base().unwrap().offset();
        let idx = self.graph.add_node(Arc::new(block));
        self.idx_map.insert(base_addr_u64, idx);
    }

    /// get a shared reference to a translation block in the graph
    /// if it exists
    pub fn get_block(&self, address: impl AsRef<u64>) -> Option<&Arc<TranslationBlock<'a>>> {
        let idx = self.idx_map.get(&address.as_ref())?;
        self.graph.node_weight(*idx)
    }

    /// check if the graph contains this a block associated with the address
    pub fn contains_block(&self, address: impl AsRef<u64>) -> bool {
        self.idx_map.contains_key(address.as_ref())
    }

    /// create an edge between two translation blocks in the graph
    /// panic if the either block does not already exist
    pub fn add_edge(
        &mut self,
        predecessor_base: impl AsRef<u64>,
        successor_base: impl AsRef<u64>,
    ) -> () {
        let pred_u64 = predecessor_base.as_ref();
        let succ_u64 = successor_base.as_ref();
        let pred_idx = self.idx_map.get(pred_u64).unwrap();
        let succ_idx = self.idx_map.get(succ_u64).unwrap();
        self.graph.add_edge(*pred_idx, *succ_idx, ());
    }

    /// fetch an instruction
    pub fn fetch(
        &'a mut self,
        lifter: &mut Lifter,
        address: impl AsRef<Address>,
        context: &ContextManager,
    ) -> &Result<Arc<TranslatedInsn>, TranslationError> {
        let address = address.as_ref();
        if let Some(base) = self.current_base {
            // if let Some(result) = Self::try_fetch_from_block(&self.tgraph, &base, address) {
            //     return result
            // }
            if self.contains_block(address) {
                let block = self.get_block(base).unwrap();
                if block.contains_address(address) {
                    // need the reborrow so that lifetimes are different and can be dropped
                    // see https://stackoverflow.com/questions/53034769/
                    // maybe could drop this and make it unsafe.
                    let block = self.get_block(base).unwrap();
                    return block.get_translated_insn(address).unwrap();
                }
            }
        }
        // no current block or address is not in current block
        // need to fetch a new block and add it as a node to the graph
        // we will NOT add an edge to the graph here. that should be done
        // at a higher level to enable greater observability/control of analysis
        let tblock = TranslationBlock::new_with(lifter, self.irb, *address, context);
        self.current_base.replace(*address);
        let idx = self.graph.add_node(Arc::new(tblock));
        self.idx_map.insert(address.offset(), idx);
        let base = self.current_base.as_ref().unwrap();
        // Self::try_fetch_from_block(&self.tgraph, base, address).unwrap()
        let block = self.get_block(base).unwrap();
        block.get_translated_insn(base).unwrap()
    }
}




#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{
        ContextType,
        manager::ContextManager,
    };
    use fugue_core::language::LanguageBuilder;

    #[test]
    fn test_add_get_block() {
        let lang_builder = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");

        let mut lifter = lang.lifter();
        let context_lifter = lang.lifter();
        let mut irb = context_lifter.irb(1024);

        // map concrete context memory
        let mem_size = 0x1000usize;
        let mut context_manager = ContextManager::new(&context_lifter);
        context_manager.map_memory(
            0x0u64,
            mem_size,
            Some(ContextType::Concrete)
        ).expect("failed to map memory");

        let program_mem: &[u8] = &[
            // 0000 <main>:
            0x80, 0xb5,             // 00: push     {r7, lr}
            0x82, 0xb0,             // 02: sub      sp, #8
            0x00, 0xaf,             // 04: add      r7, sp, #0
            0x03, 0x23,             // 06: movs     r3, #3
            0x7b, 0x60,             // 08: str      r3, [r7, #4]
            0x00, 0x23,             // 0a: movs     r3, #0
            0x3b, 0x60,             // 0c: str      r3, [r7, #0]
            0x06, 0xe0,             // 0e: b.n      1e <main+0x1e>
        ];

        // load program
        context_manager
            .write_mem(Address::from(0u64), program_mem)
            .expect("failed to write bytes");

        let block = TranslationBlock::new_with(
            &mut lifter,
            &irb,
            Address::from(0u64),
            &context_manager,
        );

        let addrs_len = block.addrs.len();
        assert!(addrs_len == 8, "{:?}", block);

        let last_addr = block.addrs.last().unwrap();
        let Some(result) = block.insns.get(&last_addr.offset()) else {
            panic!("Fetch Failed!")
        };
        let result = result.as_ref();
        let insn = result
            .as_ref()
            .expect("Fetch Failed!");
        assert!(&insn.bytes[..] == &[0x06, 0xe0]);

        let mut tgraph = TranslationGraph::new_with(&mut irb);
        tgraph.add_block(block);

        let block = tgraph.get_block(&Address::from(0u32))
            .expect("no block found at address 0");

        assert!(block.addrs.len() == 8, "{:?}", block);
    }
}