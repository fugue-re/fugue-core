// //! instruction fetcher
// //! 
// //! implements pcode instruction fettching and translation block caching

// use std::{sync::Arc, thread::current};
// use std::mem::drop;
// // use nohash_hasher::{ IntMap, IsEnabled };

// use fugue_ir::{
//     Address,
//     disassembly::lift::IRBuilderArena,
// };
// use fugue_core::{
//     lifter::Lifter,
//     ir::{ PCode, Location },
// };

// use crate::context::manager::ContextManager;
// use super::tgraph::TranslationGraph;
// use super::tblock::{
//     TranslationError,
//     TranslationBlock,
//     TranslatedInsn,
// };

// /// fetcher
// /// 
// /// struct to facilitate pre-fetching and caching instructions
// /// from a context manager
// pub struct Fetcher<'a> {
//     tgraph: TranslationGraph<'a>,
//     current_base: Option<Address>,
// }

// impl<'a> Fetcher<'a> {
//     pub fn new() -> Self {
//         Self {
//             tgraph: TranslationGraph::new(),
//             current_base: None,
//         }
//     }

//     // // return pcode from the current block or None if unable
//     // pub fn try_fetch_from_block<'z>(
//     //     tgraph: &'z TranslationGraph<'a>,
//     //     base: &Address,
//     //     address: &Address
//     // ) -> Option<&'z Result<Arc<TranslatedInsn<'a>>, TranslationError>>
//     //     where 'z: 'a
//     // {
//     //     let block = tgraph.get_block(base);
//     //     if let Some(tblock) = block {
//     //         if tblock.contains_address(address) {
//     //             return tblock.get_translated_insn(address);
//     //         }
//     //     }
//     //     None
//     // }

//     fn prefetch_new_block<'z>(
//         tgraph: &mut TranslationGraph<'z>,
//         current_base: &mut Option<Address>,
//         lifter: &mut Lifter,
//         irb: &'z IRBuilderArena,
//         address: Address,
//         context: &ContextManager,
//     ) -> () 
//     where
//         'z: 'a
//     {
//         let tblock = TranslationBlock::new_with(lifter, irb, address, context);
//         current_base.replace(address);
//         tgraph.add_block(tblock);
//     }

//     /// fetch pcode for an instruction
//     /// if the address is not yet in the tgraph, prefetch the new
//     /// translation block, add it to the graph, and then return pcode.
//     pub fn fetch_insn_pcode<'z>(
//         &mut self,
//         lifter: &mut Lifter,
//         irb: &'z IRBuilderArena,
//         address: &Address,
//         context: &ContextManager,
//     ) -> &Result<Arc<TranslatedInsn<'_>>, TranslationError> 
//     where
//         'z: 'a
//     {
//         // check if address in current block
//         if let Some(base) = self.current_base {
//             // if let Some(result) = Self::try_fetch_from_block(&self.tgraph, &base, address) {
//             //     return result
//             // }
//             if self.tgraph.contains_block(address) {
//                 let block = self.tgraph.get_block(base).unwrap();
//                 if block.contains_address(address) {
//                     // need the reborrow so that lifetimes are different and can be dropped
//                     // see https://stackoverflow.com/questions/53034769/
//                     // maybe could drop this and make it unsafe.
//                     let block = self.tgraph.get_block(base).unwrap();
//                     return block.get_translated_insn(address).unwrap();
//                 }
//             }
//         }
//         // no current block or address is not in current block
//         // need to fetch a new block and add it as a node to the graph
//         // we will NOT add an edge to the graph here. that should be done
//         // at a higher level to enable greater observability/control of analysis
//         Self::prefetch_new_block(&mut self.tgraph, &mut self.current_base, lifter, irb, address.clone(), context);
//         let base = self.current_base.as_ref().unwrap();
//         // Self::try_fetch_from_block(&self.tgraph, base, address).unwrap()
//         let block = self.tgraph.get_block(base).unwrap();
//         block.get_translated_insn(base).unwrap()
//     }
// }


// #[cfg(test)]
// mod test {
//     use super::*;
//     use crate::context::{
//         ContextType,
//         manager::ContextManager,
//     };
//     use fugue_core::{ir::Insn, language::LanguageBuilder};

//     #[test]
//     fn test_fetcher() {
//         let lang_builder = LanguageBuilder::new("../data/processors")
//             .expect("language builder not instantiated");
//         let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
//             .expect("language failed to build");

//         let mut lifter = lang.lifter();
//         let context_lifter = lang.lifter();
//         let irb = context_lifter.irb(1024);

//         // map concrete context memory
//         let mem_size = 0x1000usize;
//         let mut context_manager = ContextManager::new(&context_lifter);
//         context_manager.map_memory(
//             0x0u64,
//             mem_size,
//             Some(ContextType::Concrete)
//         ).expect("failed to map memory");

//         let program_mem: &[u8] = &[
//             // 0000 <main>:
//             0x80, 0xb5,             // 00: push     {r7, lr}
//             0x82, 0xb0,             // 02: sub      sp, #8
//             0x00, 0xaf,             // 04: add      r7, sp, #0
//             0x03, 0x23,             // 06: movs     r3, #3
//             0x7b, 0x60,             // 08: str      r3, [r7, #4]
//             0x00, 0x23,             // 0a: movs     r3, #0
//             0x3b, 0x60,             // 0c: str      r3, [r7, #0]
//             0x06, 0xe0,             // 0e: b.n      1e <main+0x1e>
//         ];

//         // load program
//         context_manager
//             .write_mem(Address::from(0u64), program_mem)
//             .expect("failed to write bytes");
        
//         let mut fetcher = Fetcher::new();
//         let result = fetcher.fetch_insn_pcode(&mut lifter, &irb, &Address::from(0u64), &context_manager).as_ref();

//         assert!(result.is_ok(), "{:?}", result);
//         let insn = result.expect("prefetch had translation error");
//         assert!(&insn.bytes[..] == &[0x80, 0xb5], "fetched wrong insn: {:?}", insn);
//     }
// }