//! instruction fetcher
//! 
//! implements pcode instruction fettching and translation block caching

use std::sync::Arc;
// use nohash_hasher::{ IntMap, IsEnabled };

use fugue_ir::{
    Address,
    disassembly::lift::IRBuilderArena,
};
use fugue_core::{
    lifter::Lifter,
    ir::{ PCode, Location },
};

use crate::context::manager::ContextManager;
use super::tgraph::TranslationGraph;
use super::tblock::{
    TranslationError,
    TranslationBlock,
    TranslatedInsn,
};

/// fetcher
/// 
/// struct to facilitate pre-fetching and caching instructions
/// from a context manager
struct Fetcher<'a> {
    tgraph: TranslationGraph<'a>,
    curr_blk_address: Option<Address>,
}

impl<'a> Fetcher<'a> {
    pub fn new() -> Self {
        Self {
            tgraph: TranslationGraph::new(),
            curr_blk_address: None,
        }
    }

    // return pcode from the current block or None if unable
    pub fn try_fetch_from_block<'z>(
        tgraph: &'z TranslationGraph<'a>,
        base: &Address,
        address: &Address
    ) -> Option<&'z Result<Arc<TranslatedInsn<'a>>, TranslationError>>
        where 'z: 'a
    {
        let block = tgraph.get_block(base);
        if let Some(tblock) = block {
            if tblock.contains_address(address) {
                return tblock.get_translated_insn(address);
            }
        }
        None
    }

    // fn prefetch<'z>(
    //     &mut self,
    //     lifter: &mut Lifter,
    //     irb: &'z IRBuilderArena,
    //     base: Address,
    //     context: &ContextManager,
    // ) -> ()
    //     where 'z: 'a
    // {
        
    // }

    /// fetch pcode for an instruction
    /// if the address is not yet in the tgraph, prefetch the new
    /// translation block, add it to the graph, and then return pcode.
    pub fn fetch_insn_pcode<'z>(
        &mut self,
        lifter: &mut Lifter,
        irb: &'z IRBuilderArena,
        address: &Address,
        context: &ContextManager,
    ) -> &'a Result<Arc<TranslatedInsn>, TranslationError>
        where 'z: 'a
    {
        // check if address in current block
        if let Some(base) = self.curr_blk_address {
            // if let Some(result) = Self::try_fetch_from_block(&self.tgraph, &base, address) {
            //     return result
            // }
            let block = self.tgraph.get_block(base);
            if let Some(tblock) = block {
                if tblock.contains_address(address) {
                    return tblock.get_translated_insn(address).unwrap();
                }
            }
        }
        // no current block or address is not in current block
        // need to fetch a new block and add it as a node to the graph
        // we will NOT add an edge to the graph here. that should be done
        // at a higher level to enable greater observability/control of analysis
        let base = address.clone();
        {
            let tblock = TranslationBlock::new_with(lifter, irb, base, context);
            self.curr_blk_address = Some(base);
            self.tgraph.add_block(tblock);
        }
        let base = self.curr_blk_address.as_ref().unwrap();
        Self::try_fetch_from_block(&self.tgraph, base, address).unwrap()
    }
}