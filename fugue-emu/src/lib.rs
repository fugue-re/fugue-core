pub mod context;
pub mod engine;
pub mod emu;

#[allow(unused_imports)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;
    use crate::engine;
    use crate::emu::Clocked;
    use crate::context::{
        ContextType,
        ContextError,
        MappedContext,
    };
    use crate::context::concrete::ConcreteMemory;
    use crate::context::manager::ContextManager;
    use crate::engine::{
        Engine,
        EngineType,
    };
    use fugue::high::eval::{
        Evaluator,
        EvaluatorContext,
        EvaluatorTarget,
    };
    use fugue::high::language::LanguageBuilder;
    use fugue::high::ir::Location;
    use fugue::ir::{
        Address,
        VarnodeData,
        space::{
            Space,
            AddressSpace,
            SpaceKind
        },
    };
    use fugue::bytes::Endian;
    use fugue::bv::BitVec;
    use std::time::Instant;

    #[test]
    fn test_manager_initialize() {

        let lang_builder = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");


        // give language builder a context_lifter to make accesses easier
        let context_lifter = lang.lifter();
        #[allow(unused)]
        let context_manager = ContextManager::new(context_lifter);
    }

    #[test]
    fn test_memory_concrete_read_write() {
        let mut memory = ConcreteMemory::new(
            0x0u32, 
            Endian::Little,
            0x1000,
        );

        let vnd = VarnodeData::new(
            &AddressSpace::Space(
                Space::new(
                    SpaceKind::Default,
                    "dummy",
                    1usize,
                    4usize,
                    0x0usize,
                    None,
                    0usize,
                )
            ),
            0x400u64,
            4usize,
        );

        let write_val = BitVec::from_u32(0xdeadbeefu32, 32);

        // test read/write varnode
        memory
            .write_vnd(&vnd, &write_val)
            .expect("failed to write to memory");
        let read_val = memory
            .read_vnd(&vnd)
            .expect("failed to read from memory");

        assert_eq!(read_val, write_val);

        // test read/write bytes
        let bytes = &[0x12, 0x34, 0x56, 0x78];
        memory
            .write_bytes(Address::from(0x200u64), bytes)
            .expect("failed to write bytes to memory");
        let read_bytes = memory
            .read_bytes(Address::from(0x200u64), 4usize)
            .expect("failed to read bytes from memory");

        assert_eq!(read_bytes, bytes);

        // test write bytes, read varnode and vice versa
        let vnd = VarnodeData::new(
            &AddressSpace::Space(
                Space::new(
                    SpaceKind::Default,
                    "dummy",
                    1usize,
                    4usize,
                    0x0usize,
                    None,
                    0usize,
                )
            ),
            0x800u64,
            4usize,
        );
        let bytes = &[0xef, 0xbe, 0xad, 0xde];
        memory
            .write_bytes(Address::from(0x800u64), bytes)
            .expect("failed to write bytes to memory");
        let read_val = memory
            .read_vnd(&vnd)
            .expect("failed to read varnode from memory");
        
        assert_eq!(read_val, BitVec::from_le_bytes(bytes));

        let bytes = &[0xde, 0xc0, 0xad, 0xde];
        let write_val = BitVec::from_le_bytes(bytes);
        memory
            .write_vnd(&vnd, &write_val)
            .expect("failed to write varnode");
        let read_bytes = memory
            .read_bytes(Address::from(0x800u64), 4usize)
            .expect("failed to read bytes from memory");

        assert_eq!(write_val, BitVec::from_le_bytes(&read_bytes));

    }

    #[test]
    fn test_manager_map_memory() {

        let lang_builder = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");

        // give language builder a context_lifter to make accesses easier
        let context_lifter = lang.lifter();

        let mut context_manager = ContextManager::new(context_lifter);
        
        // test mapping mapping in parts
        context_manager.map_memory(0x0u32, 0x4000, None)
            .expect("failed to map memory [0x0000, 0x4000)")
            .map_memory(0x4000u32, 0x4000, Some(context::ContextType::Concrete))
            .expect("failed to map memory [0x4000, 0x8000)");

        // test mapping errors
        context_manager.map_memory(0x1000u32, 0x2000, None)
            .expect_err("[0x0000, 0x4000) should already be mapped");
        context_manager.map_memory(0x400u32, 0x1000, None)
            .expect_err("expected error: base 0x400 is not aligned");
        context_manager.map_memory(0x8000u32, 0x400, None)
            .expect_err("expected error: size 0x400 is not aligned");
    }

    #[test]
    fn test_manager_read_write_memory() {
        let lang_builder = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");

        // give language builder a context_lifter to make accesses easier
        let context_lifter = lang.lifter();

        let mut context_manager = ContextManager::new(context_lifter);
        
        // test mapping mapping in parts
        context_manager.map_memory(0x0u32, 0x4000, None)
            .expect("failed to map memory [0x0000, 0x4000)");

        // test write bytes, read varnode and vice versa
        let vnd = VarnodeData::new(
            &AddressSpace::Space(
                Space::new(
                    SpaceKind::Default,
                    "dummy",
                    1usize,
                    4usize,
                    0x0usize,
                    None,
                    0usize,
                )
            ),
            0x800u64,
            4usize,
        );
        let bytes = &[0xef, 0xbe, 0xad, 0xde];
        context_manager
            .get_mut_context_at(0x800u64)
            .expect("failed to get context at 0x800")
            .write_bytes(Address::from(0x800u64), bytes)
            .expect("failed to write bytes to memory");
        let read_val = context_manager
            .read_vnd(&vnd)
            .expect("failed to read varnode from memory");
        
        assert_eq!(read_val, BitVec::from_le_bytes(bytes));

        let bytes = &[0xde, 0xc0, 0xad, 0xde];
        let write_val = BitVec::from_le_bytes(bytes);
        context_manager
            .write_vnd(&vnd, &write_val)
            .expect("failed to write varnode");
        let read_bytes = context_manager
            .get_mut_context_at(0x800u64)
            .expect("failed to get context at 0x800")
            .read_bytes(Address::from(0x800u64), 4usize)
            .expect("failed to read bytes from memory");

        assert_eq!(write_val, BitVec::from_le_bytes(&read_bytes));
    }

    #[test]
    fn test_engine_initialize() {
        let lang_builder = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");

        let mut engine_lifter = lang.lifter();
        let context_lifter = lang.lifter();

        #[allow(unused)]
        let mut context_manager = ContextManager::new(context_lifter);

        #[allow(unused)]
        let mut engine = Engine::new(
            &mut engine_lifter, 
            EngineType::Concrete,
            None,
        );
    }

    #[test]
    fn test_engine_fetch() {
        // set up language
        let lang_builder = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");

        // initalize lifters for contex and engine
        let mut engine_lifter = lang.lifter();
        let context_lifter = lang.lifter();

        // map concrete context memory
        let mut context_manager = ContextManager::new(context_lifter);
        context_manager.map_memory(
            0x0u64,
            0x1000usize,
            Some(ContextType::Concrete)
        ).expect("failed to map memory");

        // initialize engine
        let mut engine = Engine::new(
            &mut engine_lifter, 
            EngineType::Concrete,
            None,
        );

        let insn_bytes: &[u8] = &[
            0x70, 0x47,             // 00: bx lr
        ];

        context_manager
            .write_bytes(Address::from(0x400u64), insn_bytes)
            .expect("failed to write bytes");

        engine.pc.set_pc(0x400u64, &mut context_manager)
            .expect("failed to set pc");

        let pc_loc = engine.pc.get_pc_loc(&mut context_manager);
        
        assert!(pc_loc.address == 0x400u64);

        let pcode = engine.icache
            .fetch(
                &engine.lifter,
                &pc_loc, 
                &mut context_manager, 
                engine.engine_type
            )
            .expect("failed to fetch instruction");

        assert!(pcode.operations.len() > 0, "pcode: {:?}", pcode);
    }

    #[test]
    fn test_engine_step() {

        // set up language
        let lang_builder = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");

        // initalize lifters for contex and engine
        let mut engine_lifter = lang.lifter();
        let context_lifter = lang.lifter();

        // map concrete context memory
        let mut context_manager = ContextManager::new(context_lifter);
        context_manager.map_memory(
            0x0u64,
            0x1000usize,
            Some(ContextType::Concrete)
        ).expect("failed to map memory");

        // initialize engine
        let mut engine = Engine::new(
            &mut engine_lifter, 
            EngineType::Concrete,
            None,
        );

        let insn_bytes: &[u8] = &[
            0x0c, 0x20,  // 00: movs    r0, #12
            0x04, 0x21,  // 02: movs    r1, #4
            0x08, 0x44,  // 04: add     r0, r1
            0x88, 0x42,  // 06: cmp     r0, r1
            0x18, 0xbf,  // 08: it      ne
            0x02, 0x30,  // 0a: addne   r0, #2
        ];

        context_manager
            .write_bytes(Address::from(0u64), insn_bytes)
            .expect("failed to write bytes");

        // check pc defaults to 0
        let pc_loc = engine.pc.get_pc_loc(&mut context_manager);
        assert!(pc_loc.address == 0u64);

        for i in 0..6 {
            assert!(
                engine.step(&mut context_manager).is_ok(),
                "failed at step {}", i
            );
        }

        // check pc incremented
        let pc_loc = engine.pc.get_pc_loc(&mut context_manager);
        assert!(pc_loc.address == 0xcu64);
    }

    // // 
    // // general tests
    // // 

    // #[test]
    // fn test_concrete_context() {
    //     // a program that computes (((3 ** 2) ** 2) ** 2)
    //     // compiled with xpack arm-none-eabi-gcc arm64 11.3.1 20220712
    //     // arm-none-eabi-gcc main.c -mcpu=cortex-m4 -mthumb -nostdlib
    //     let program_mem: &[u8] = &[
    //         // 0000 <main>:
    //         0x80, 0xb5,             // 00: push     {r7, lr}
    //         0x82, 0xb0,             // 02: sub      sp, #8
    //         0x00, 0xaf,             // 04: add      r7, sp, #0
    //         0x03, 0x23,             // 06: movs     r3, #3
    //         0x7b, 0x60,             // 08: str      r3, [r7, #4]
    //         0x00, 0x23,             // 0a: movs     r3, #0
    //         0x3b, 0x60,             // 0c: str      r3, [r7, #0]
    //         0x06, 0xe0,             // 0e: b.n      1e <main+0x1e>
    //         0x78, 0x68,             // 10: ldr      r0, [r7, #4]
    //         0x00, 0xf0, 0x0c, 0xf8, // 12: bl       2e <square>
    //         0x78, 0x60,             // 16: str      r0, [r7, #4]
    //         0x3b, 0x68,             // 18: ldr      r3, [r7, #0]
    //         0x01, 0x33,             // 1a: adds     r3, #1
    //         0x3b, 0x60,             // 1c: str      r3, [r7, #0]
    //         0x3b, 0x68,             // 1e: ldr      r3, [r7, #0]
    //         0x02, 0x2b,             // 20: cmp      r3, #2
    //         0xf5, 0xdd,             // 22: ble.n    10 <main+0x10>
    //         0x7b, 0x68,             // 24: ldr      r3, [r7, #4]
    //         0x18, 0x46,             // 26: mov      r0, r3
    //         0x08, 0x37,             // 28: adds     r7, #8
    //         0xbd, 0x46,             // 2a: mov      sp, r7
    //         0x80, 0xbd,             // 2c: pop      {r7, pc}
    //         // 002e <square>:
    //         0x80, 0xb4,             // 2e: push     {r7}
    //         0x83, 0xb0,             // 30: sub      sp, #12
    //         0x00, 0xaf,             // 32: add      r7, sp, #0
    //         0x78, 0x60,             // 34: str      r0, [r7, #4]
    //         0x7b, 0x68,             // 36: ldr      r3, [r7, #4]
    //         0x03, 0xfb, 0x03, 0xf3, // 38: mul.w    r3, r3, r3
    //         0x18, 0x46,             // 3c: mov      r0, r3
    //         0x0c, 0x37,             // 3e: adds     r7, #12
    //         0xbd, 0x46,             // 40: mov      sp, r7
    //         0x80, 0xbc,             // 42: pop      {r7}
    //         0x70, 0x47,             // 44: bx       lr
    //     ];

    //     // in debug, path needs to be relative to fugue-core/fugue-core package
    //     // in run, path needs to be relative to top level fugue-core
    //     let lb = LanguageBuilder::new("../data/processors")
    //         .expect("language builder not instantiated");
    //     // convention is defined by sleigh. "default" should usually work.
    //     let lang = lb.build("ARM:LE:32:Cortex", "default")
    //         .expect("language failed to build");
    //     let mut lifter = lang.lifter();
    //     let context_lifter = lang.lifter();
    //     let translator = context_lifter.translator();
    //     let irb = lifter.irb(1024);

    //     // instantiate dummy context
    //     let mut context = ConcreteContext::new(
    //         &context_lifter,
    //         Address::from(0x0u32),
    //         vec![(0x0, 0x1000)]
    //     );

    //     // initialize lr with 0xFFFFFFFF to signify program return
    //     EvaluatorContext::write_vnd(
    //         &mut context, 
    //         &translator.register_by_name("lr").unwrap(),
    //         &BitVec::from_u32(0xFFFFFFFFu32, 32usize)
    //     ).expect("failed to write varnode!");

    //     // initialize sp
    //     EvaluatorContext::write_vnd(
    //         &mut context, 
    //         &translator.register_by_name("sp").unwrap(),
    //         &BitVec::from_usize(0x1000usize, 32usize)
    //     ).expect("failed to write varnode!");

    //     // initialize r0
    //     EvaluatorContext::write_vnd(
    //         &mut context, 
    //         &translator.register_by_name("r0").unwrap(),
    //         &BitVec::from_i32(5, 32usize)
    //     ).expect("failed to write varnode!");

    //     let mut evaluator = Evaluator::new(&context_lifter, &mut context);

    //     // prep for execution
    //     let mut offset = 0usize;
    //     let address = Address::from(0x0u32);

    //     // lift and execute
    //     while offset < program_mem.len() {
    //         let insn = lifter.disassemble(&irb, address + offset, &program_mem[offset..])
    //             .expect("couldn't disassemble instruction!");
    //         let pcode = lifter.lift(&irb, address + offset, &program_mem[offset..])
    //             .expect("couldn't lift instruction to pcode!");

    //         println!("--- insn @ {} | length {} ---", insn.address(), insn.len());
    //         println!("{} {}", insn.mnemonic(), insn.operands());
    //         println!();

    //         println!("--- pcode @ {} ---", pcode.address());
    //         let mut branch = false;
    //         for (i, op) in pcode.operations().iter().enumerate() {
    //             println!("{i:02} {}", op.display(lang.translator()));
                
    //             let target = evaluator
    //                 .step(Location::from(address + offset), op)
    //                 .expect("evaluator error!");

    //             match target {
    //                 EvaluatorTarget::Fall => {
    //                     println!("sp: {:?} r0: {:?} r3: {:?} r7: {:?}",
    //                         evaluator.read_reg("sp").unwrap(),
    //                         evaluator.read_reg("r0").unwrap(),
    //                         evaluator.read_reg("r3").unwrap(),
    //                         evaluator.read_reg("r7").unwrap()
    //                     );  
    //                 },
    //                 EvaluatorTarget::Branch(loc) |
    //                 EvaluatorTarget::Call(loc) |
    //                 EvaluatorTarget::Return(loc) => if loc.position != 0 {
    //                     panic!("Branch {:?}", loc)
    //                 } else {
    //                     offset = loc.address.offset() as usize;
    //                     branch = true;
    //                 },
    //                 // _ => panic!("unexpected instruction!")
    //             }
    //         }
    //         if !branch {
    //             offset += insn.len();
    //         }
    //         println!();
    //     }

    //     // i want this: VarnodeData { AddressSpaceId::register_id(0x3usize), 0x20u64, 4usize }
    //     // which should be r0, as defined in the .sla as 
    //     // <varnode_sym name="r0" id="0x3" scope="0x0" space="register" offset="0x20" size="4">
    //     // i'm dumb translator has a method to get it for me.
    //     // println!("result: {:?}", EvaluatorContext::read_vnd(&mut context, &translator.register_by_name("r0").unwrap()));
    //     println!("sp: {:?}", EvaluatorContext::read_vnd(&mut context, &translator.register_by_name("sp").unwrap()));
    //     println!("pc: {:?}", EvaluatorContext::read_vnd(&mut context, &translator.register_by_name("pc").unwrap()));
    //     // check the value on the stack
    //     println!(
    //         "r0 on stack: {:?}",
    //         // evaluator.read_mem(Address::from(0x1000usize - 12), 4)
    //         EvaluatorContext::read_vnd(
    //             &mut context, 
    //             &VarnodeData::new(
    //                 translator.manager().default_space_ref(), 
    //                 Address::from(0x1000u32 - 8).offset(),
    //                 4
    //             )
    //         )
    //     )
    // }

    // #[test]
    // fn test_large_space() {

    //     // program that writes to memory starting at 
    //     // 0x4000 until 0x8000 (0x1000 int writes)
    //     // e.g. mem[0x4100] == 64 (0x100 / 4)
    //     let program_mem: &[u8] = &[
    //         // 0000 <main>:
    //         0x80, 0xb4,             // 00: push   {r7}
    //         0x85, 0xb0,             // 02: sub    sp, #20
    //         0x00, 0xaf,             // 04: add    r7, sp, #0
    //         0x4f, 0xf4, 0x80, 0x43, // 06: mov.w  r3, #16384  ; 0x4000
    //         0xbb, 0x60,             // 0a: str    r3, [r7, #8]
    //         0xbb, 0x68,             // 0c: ldr    r3, [r7, #8]
    //         0x7b, 0x60,             // 0e: str    r3, [r7, #4]
    //         0x00, 0x23,             // 10: movs   r3, #0
    //         0xfb, 0x60,             // 12: str    r3, [r7, #12]
    //         0x08, 0xe0,             // 14: b.n    8028 <main+0x28>
    //         0xfb, 0x68,             // 16: ldr    r3, [r7, #12]
    //         0x9b, 0x00,             // 18: lsls   r3, r3, #2
    //         0x7a, 0x68,             // 1a: ldr    r2, [r7, #4]
    //         0x13, 0x44,             // 1c: add    r3, r2
    //         0xfa, 0x68,             // 1e: ldr    r2, [r7, #12]
    //         0x1a, 0x60,             // 20: str    r2, [r3, #0]
    //         0xfb, 0x68,             // 22: ldr    r3, [r7, #12]
    //         0x01, 0x33,             // 24: adds   r3, #1
    //         0xfb, 0x60,             // 26: str    r3, [r7, #12]
    //         0xfb, 0x68,             // 28: ldr    r3, [r7, #12]
    //         0xb3, 0xf5, 0x80, 0x5f, // 2a: cmp.w  r3, #4096   ; 0x1000
    //         0xf2, 0xdb,             // 2e: blt.n  8016 <main+0x16>
    //         0xfb, 0x68,             // 30: ldr    r3, [r7, #12]
    //         0x18, 0x46,             // 32: mov    r0, r3
    //         0x14, 0x37,             // 34: adds   r7, #20
    //         0xbd, 0x46,             // 36: mov    sp, r7
    //         0x80, 0xbc,             // 38: pop    {r7}
    //         0x70, 0x47,             // 3a: bx lr
    //     ];

    //     // in debug, path needs to be relative to fugue-core/fugue-core package
    //     // in run, path needs to be relative to top level fugue-core
    //     let lb = LanguageBuilder::new("../data/processors")
    //         .expect("language builder not instantiated");
    //     let lang = lb.build("ARM:LE:32:Cortex", "default")
    //         .expect("language failed to build");
    //     let mut lifter = lang.lifter();
    //     let context_lifter = lang.lifter();
    //     let translator = context_lifter.translator();
    //     let irb = lifter.irb(0x1000);

    //     // instantiate dummy context to write to.
    //     let mut context = ConcreteContext::new(
    //         &context_lifter,
    //         Address::from(0x0u32),
    //         vec![(0x4000, 0x8000), (0xe000, 0x2000)]
    //     );

    //     // initialize lr with 0xFFFFFFFF to signify program return
    //     EvaluatorContext::write_vnd(
    //         &mut context, 
    //         &translator.register_by_name("lr").unwrap(),
    //         &BitVec::from_u32(0xFFFFFFFFu32, 32usize)
    //     ).expect("failed to write varnode!");

    //     // initialize sp
    //     EvaluatorContext::write_vnd(
    //         &mut context, 
    //         &translator.register_by_name("sp").unwrap(),
    //         &BitVec::from_usize(0x10000usize, 32usize)
    //     ).expect("failed to write varnode!");

    //     let mut evaluator = Evaluator::new(&context_lifter, &mut context);

    //     // prep for execution
    //     let mut offset = 0usize;
    //     let address = Address::from(0x0u32);

    //     println!("beginning execution...");
    //     let start = Instant::now();


    //     // lift and execute
    //     while offset < program_mem.len() {
    //         let insn = lifter.disassemble(&irb, address + offset, &program_mem[offset..])
    //             .expect("couldn't disassemble instruction!");
    //         let pcode = lifter.lift(&irb, address + offset, &program_mem[offset..])
    //             .expect("couldn't lift instruction to pcode!");

    //         // println!("--- insn @ {} | length {} ---", insn.address(), insn.len());
    //         // println!("{} {}", insn.mnemonic(), insn.operands());
    //         // println!();

    //         // println!("--- pcode @ {} ---", pcode.address());
    //         let mut branch = false;
    //         for (_i, op) in pcode.operations().iter().enumerate() {
    //             // println!("{i:02} {}", op.display(lang.translator()));
                
    //             let target = evaluator
    //                 .step(Location::from(address + offset), op)
    //                 .expect("evaluator error!");

    //             match target {
    //                 EvaluatorTarget::Fall => {
    //                     // println!("sp: {:?} r0: {:?} r3: {:?} r7: {:?}",
    //                     //     evaluator.read_reg("sp").unwrap(),
    //                     //     evaluator.read_reg("r0").unwrap(),
    //                     //     evaluator.read_reg("r3").unwrap(),
    //                     //     evaluator.read_reg("r7").unwrap()
    //                     // );
    //                 },
    //                 EvaluatorTarget::Branch(loc) |
    //                 EvaluatorTarget::Call(loc) |
    //                 EvaluatorTarget::Return(loc) => if loc.position != 0 {
    //                     panic!("Branch {:?}", loc)
    //                 } else {
    //                     offset = loc.address.offset() as usize;
    //                     branch = true;
    //                 },
    //                 // _ => panic!("unexpected instruction!")
    //             }
    //         }
    //         if !branch {
    //             offset += insn.len();
    //         }
    //         // println!();
    //     }

    //     let elapsed = start.elapsed();
    //     println!("elapsed: {:0.2?}", elapsed);
        
    //     println!("result: {:?}", EvaluatorContext::read_vnd(&mut context, &translator.register_by_name("r0").unwrap()));
    //     println!("sp: {:?}", EvaluatorContext::read_vnd(&mut context, &translator.register_by_name("sp").unwrap()));
    //     println!("pc: {:?}", EvaluatorContext::read_vnd(&mut context, &translator.register_by_name("pc").unwrap()));
    //     // check the value on the stack
    //     println!(
    //         "value at 0x4100: {:?}",
    //         // evaluator.read_mem(Address::from(0x1000usize - 12), 4)
    //         EvaluatorContext::read_vnd(
    //             &mut context, 
    //             &VarnodeData::new(
    //                 translator.manager().default_space_ref(), 
    //                 Address::from(0x4100u32).offset(),
    //                 4
    //             )
    //         )
    //     )
    // }
}
