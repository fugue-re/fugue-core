pub mod context;



#[allow(unused_imports)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::concrete::ConcreteContext;
    use fugue::high::eval::{
        Evaluator,
        EvaluatorContext,
        EvaluatorTarget,
    };
    use fugue::high::language::LanguageBuilder;
    use fugue::high::ir::Location;
    use fugue::ir::Address;
    use fugue::bv::BitVec;
    use fugue::ir::VarnodeData;
    use std::time::Instant;

    #[test]
    fn test_concrete_context() {
        // a program that computes (((3 ** 2) ** 2) ** 2)
        // compiled with xpack arm-none-eabi-gcc arm64 11.3.1 20220712
        // arm-none-eabi-gcc main.c -mcpu=cortex-m4 -mthumb -nostdlib
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
            0x78, 0x68,             // 10: ldr      r0, [r7, #4]
            0x00, 0xf0, 0x0c, 0xf8, // 12: bl       2e <square>
            0x78, 0x60,             // 16: str      r0, [r7, #4]
            0x3b, 0x68,             // 18: ldr      r3, [r7, #0]
            0x01, 0x33,             // 1a: adds     r3, #1
            0x3b, 0x60,             // 1c: str      r3, [r7, #0]
            0x3b, 0x68,             // 1e: ldr      r3, [r7, #0]
            0x02, 0x2b,             // 20: cmp      r3, #2
            0xf5, 0xdd,             // 22: ble.n    10 <main+0x10>
            0x7b, 0x68,             // 24: ldr      r3, [r7, #4]
            0x18, 0x46,             // 26: mov      r0, r3
            0x08, 0x37,             // 28: adds     r7, #8
            0xbd, 0x46,             // 2a: mov      sp, r7
            0x80, 0xbd,             // 2c: pop      {r7, pc}
            // 002e <square>:
            0x80, 0xb4,             // 2e: push     {r7}
            0x83, 0xb0,             // 30: sub      sp, #12
            0x00, 0xaf,             // 32: add      r7, sp, #0
            0x78, 0x60,             // 34: str      r0, [r7, #4]
            0x7b, 0x68,             // 36: ldr      r3, [r7, #4]
            0x03, 0xfb, 0x03, 0xf3, // 38: mul.w    r3, r3, r3
            0x18, 0x46,             // 3c: mov      r0, r3
            0x0c, 0x37,             // 3e: adds     r7, #12
            0xbd, 0x46,             // 40: mov      sp, r7
            0x80, 0xbc,             // 42: pop      {r7}
            0x70, 0x47,             // 44: bx       lr
        ];

        // in debug, path needs to be relative to fugue-core/fugue-core package
        // in run, path needs to be relative to top level fugue-core
        let lb = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lb.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");
        let mut lifter = lang.lifter();
        let context_lifter = lang.lifter();
        let translator = context_lifter.translator();
        let irb = lifter.irb(1024);

        // instantiate dummy context
        let mut context = ConcreteContext::new(
            &context_lifter,
            Address::from(0x0u32),
            vec![(0x0, 0x1000)]
        );

        // initialize lr with 0xFFFFFFFF to signify program return
        EvaluatorContext::write_vnd(
            &mut context, 
            &translator.register_by_name("lr").unwrap(),
            &BitVec::from_u32(0xFFFFFFFFu32, 32usize)
        ).expect("failed to write varnode!");

        // initialize sp
        EvaluatorContext::write_vnd(
            &mut context, 
            &translator.register_by_name("sp").unwrap(),
            &BitVec::from_usize(0x1000usize, 32usize)
        ).expect("failed to write varnode!");

        // initialize r0
        EvaluatorContext::write_vnd(
            &mut context, 
            &translator.register_by_name("r0").unwrap(),
            &BitVec::from_i32(5, 32usize)
        ).expect("failed to write varnode!");

        let mut evaluator = Evaluator::new(&context_lifter, &mut context);

        // prep for execution
        let mut offset = 0usize;
        let address = Address::from(0x0u32);

        // lift and execute
        while offset < program_mem.len() {
            let insn = lifter.disassemble(&irb, address + offset, &program_mem[offset..])
                .expect("couldn't disassemble instruction!");
            let pcode = lifter.lift(&irb, address + offset, &program_mem[offset..])
                .expect("couldn't lift instruction to pcode!");

            println!("--- insn @ {} | length {} ---", insn.address(), insn.len());
            println!("{} {}", insn.mnemonic(), insn.operands());
            println!();

            println!("--- pcode @ {} ---", pcode.address());
            let mut branch = false;
            for (i, op) in pcode.operations().iter().enumerate() {
                println!("{i:02} {}", op.display(lang.translator()));
                
                let target = evaluator
                    .step(Location::from(address + offset), op)
                    .expect("evaluator error!");

                match target {
                    EvaluatorTarget::Fall => {
                        println!("sp: {:?} r0: {:?} r3: {:?} r7: {:?}",
                            evaluator.read_reg("sp").unwrap(),
                            evaluator.read_reg("r0").unwrap(),
                            evaluator.read_reg("r3").unwrap(),
                            evaluator.read_reg("r7").unwrap()
                        );  
                    },
                    EvaluatorTarget::Branch(loc) |
                    EvaluatorTarget::Call(loc) |
                    EvaluatorTarget::Return(loc) => if loc.position != 0 {
                        panic!("Branch {:?}", loc)
                    } else {
                        offset = loc.address.offset() as usize;
                        branch = true;
                    },
                    // _ => panic!("unexpected instruction!")
                }
            }
            if !branch {
                offset += insn.len();
            }
            println!();
        }

        // i want this: VarnodeData { AddressSpaceId::register_id(0x3usize), 0x20u64, 4usize }
        // which should be r0, as defined in the .sla as 
        // <varnode_sym name="r0" id="0x3" scope="0x0" space="register" offset="0x20" size="4">
        // i'm dumb translator has a method to get it for me.
        // println!("result: {:?}", EvaluatorContext::read_vnd(&mut context, &translator.register_by_name("r0").unwrap()));
        println!("sp: {:?}", EvaluatorContext::read_vnd(&mut context, &translator.register_by_name("sp").unwrap()));
        println!("pc: {:?}", EvaluatorContext::read_vnd(&mut context, &translator.register_by_name("pc").unwrap()));
        // check the value on the stack
        println!(
            "r0 on stack: {:?}",
            // evaluator.read_mem(Address::from(0x1000usize - 12), 4)
            EvaluatorContext::read_vnd(
                &mut context, 
                &VarnodeData::new(
                    translator.manager().default_space_ref(), 
                    Address::from(0x1000u32 - 8).offset(),
                    4
                )
            )
        )
    }

    #[test]
    fn test_large_space() {

        // program that writes to memory starting at 
        // 0x4000 until 0x8000 (0x1000 int writes)
        // e.g. mem[0x4100] == 64 (0x100 / 4)
        let program_mem: &[u8] = &[
            // 0000 <main>:
            0x80, 0xb4,             // 00: push   {r7}
            0x85, 0xb0,             // 02: sub    sp, #20
            0x00, 0xaf,             // 04: add    r7, sp, #0
            0x4f, 0xf4, 0x80, 0x43, // 06: mov.w  r3, #16384  ; 0x4000
            0xbb, 0x60,             // 0a: str    r3, [r7, #8]
            0xbb, 0x68,             // 0c: ldr    r3, [r7, #8]
            0x7b, 0x60,             // 0e: str    r3, [r7, #4]
            0x00, 0x23,             // 10: movs   r3, #0
            0xfb, 0x60,             // 12: str    r3, [r7, #12]
            0x08, 0xe0,             // 14: b.n    8028 <main+0x28>
            0xfb, 0x68,             // 16: ldr    r3, [r7, #12]
            0x9b, 0x00,             // 18: lsls   r3, r3, #2
            0x7a, 0x68,             // 1a: ldr    r2, [r7, #4]
            0x13, 0x44,             // 1c: add    r3, r2
            0xfa, 0x68,             // 1e: ldr    r2, [r7, #12]
            0x1a, 0x60,             // 20: str    r2, [r3, #0]
            0xfb, 0x68,             // 22: ldr    r3, [r7, #12]
            0x01, 0x33,             // 24: adds   r3, #1
            0xfb, 0x60,             // 26: str    r3, [r7, #12]
            0xfb, 0x68,             // 28: ldr    r3, [r7, #12]
            0xb3, 0xf5, 0x80, 0x5f, // 2a: cmp.w  r3, #4096   ; 0x1000
            0xf2, 0xdb,             // 2e: blt.n  8016 <main+0x16>
            0xfb, 0x68,             // 30: ldr    r3, [r7, #12]
            0x18, 0x46,             // 32: mov    r0, r3
            0x14, 0x37,             // 34: adds   r7, #20
            0xbd, 0x46,             // 36: mov    sp, r7
            0x80, 0xbc,             // 38: pop    {r7}
            0x70, 0x47,             // 3a: bx lr
        ];

        // in debug, path needs to be relative to fugue-core/fugue-core package
        // in run, path needs to be relative to top level fugue-core
        let lb = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lb.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");
        let mut lifter = lang.lifter();
        let context_lifter = lang.lifter();
        let translator = context_lifter.translator();
        let irb = lifter.irb(0x1000);

        // instantiate dummy context to write to.
        let mut context = ConcreteContext::new(
            &context_lifter,
            Address::from(0x0u32),
            vec![(0x4000, 0x8000), (0xe000, 0x2000)]
        );

        // initialize lr with 0xFFFFFFFF to signify program return
        EvaluatorContext::write_vnd(
            &mut context, 
            &translator.register_by_name("lr").unwrap(),
            &BitVec::from_u32(0xFFFFFFFFu32, 32usize)
        ).expect("failed to write varnode!");

        // initialize sp
        EvaluatorContext::write_vnd(
            &mut context, 
            &translator.register_by_name("sp").unwrap(),
            &BitVec::from_usize(0x10000usize, 32usize)
        ).expect("failed to write varnode!");

        let mut evaluator = Evaluator::new(&context_lifter, &mut context);

        // prep for execution
        let mut offset = 0usize;
        let address = Address::from(0x0u32);

        println!("beginning execution...");
        let start = Instant::now();


        // lift and execute
        while offset < program_mem.len() {
            let insn = lifter.disassemble(&irb, address + offset, &program_mem[offset..])
                .expect("couldn't disassemble instruction!");
            let pcode = lifter.lift(&irb, address + offset, &program_mem[offset..])
                .expect("couldn't lift instruction to pcode!");

            // println!("--- insn @ {} | length {} ---", insn.address(), insn.len());
            // println!("{} {}", insn.mnemonic(), insn.operands());
            // println!();

            // println!("--- pcode @ {} ---", pcode.address());
            let mut branch = false;
            for (_i, op) in pcode.operations().iter().enumerate() {
                // println!("{i:02} {}", op.display(lang.translator()));
                
                let target = evaluator
                    .step(Location::from(address + offset), op)
                    .expect("evaluator error!");

                match target {
                    EvaluatorTarget::Fall => {
                        // println!("sp: {:?} r0: {:?} r3: {:?} r7: {:?}",
                        //     evaluator.read_reg("sp").unwrap(),
                        //     evaluator.read_reg("r0").unwrap(),
                        //     evaluator.read_reg("r3").unwrap(),
                        //     evaluator.read_reg("r7").unwrap()
                        // );
                    },
                    EvaluatorTarget::Branch(loc) |
                    EvaluatorTarget::Call(loc) |
                    EvaluatorTarget::Return(loc) => if loc.position != 0 {
                        panic!("Branch {:?}", loc)
                    } else {
                        offset = loc.address.offset() as usize;
                        branch = true;
                    },
                    // _ => panic!("unexpected instruction!")
                }
            }
            if !branch {
                offset += insn.len();
            }
            // println!();
        }

        let elapsed = start.elapsed();
        println!("elapsed: {:0.2?}", elapsed);
        
        println!("result: {:?}", EvaluatorContext::read_vnd(&mut context, &translator.register_by_name("r0").unwrap()));
        println!("sp: {:?}", EvaluatorContext::read_vnd(&mut context, &translator.register_by_name("sp").unwrap()));
        println!("pc: {:?}", EvaluatorContext::read_vnd(&mut context, &translator.register_by_name("pc").unwrap()));
        // check the value on the stack
        println!(
            "value at 0x4100: {:?}",
            // evaluator.read_mem(Address::from(0x1000usize - 12), 4)
            EvaluatorContext::read_vnd(
                &mut context, 
                &VarnodeData::new(
                    translator.manager().default_space_ref(), 
                    Address::from(0x4100u32).offset(),
                    4
                )
            )
        )
    }
}
