use fugue::high::eval::{
    DummyContext,
    Evaluator,
    EvaluatorContext,
    EvaluatorTarget,
};
use fugue::high::language::LanguageBuilder;
use fugue::high::ir::Location;
use fugue::ir::Address;
use fugue::bv::BitVec;
use fugue_ir::VarnodeData;





fn main() {

    // a function to square r0
    let program_mem: &[u8] = &[
        // 0x80, 0xb4,             // push     {r7}
        0x83, 0xb0,             // sub      sp, #12
        0x00, 0xaf,             // add      r7, sp, #0
        0x78, 0x60,             // str      r0, [r7, #4]
        0x7b, 0x68,             // ldr      r3, [r7, #4]
        0x03, 0xfb, 0x03, 0xf3, // mul.w    r3, r3, r3
        0x18, 0x46,             // mov      r0, r3
        0x0c, 0x37,             // adds     r7, #12
        0xbd, 0x46,             // mov      sp, r7
        // 0x80, 0xbc,             // pop      {r7}
        // 0x70, 0x47,             // bx lr
    ];
    // push does sp = sp - 4*bitcount

    // in debug, path needs to be relative to fugue-core/fugue-core package
    // in run, path needs to be relative to top level fugue-core
    let lb = LanguageBuilder::new("data/processors")
        .expect("language builder not instantiated");
    let lang = lb.build("ARM:LE:32:Cortex", "default")
        .expect("language failed to build");
    let mut lifter = lang.lifter();
    let context_lifter = lang.lifter();
    let translator = context_lifter.translator();
    let irb = lifter.irb(1024);

    // instantiate dummy context
    let mut context = DummyContext::new(
        &context_lifter,
        Address::from(0x0u32),
        0x1000
    );

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

        println!("--- insn @ {} ---", insn.address());
        println!("{} {}", insn.mnemonic(), insn.operands());
        println!();

        println!("--- pcode @ {} ---", pcode.address());
        for (i, op) in pcode.operations().iter().enumerate() {
            println!("{i:02} {}", op.display(lang.translator()));
            
            let target = evaluator
                .step(Location::from(address + offset), op)
                .expect("evaluator error!");

            match target {
                EvaluatorTarget::Fall => println!("sp: {:?} r0: {:?} r3: {:?} r7: {:?}",
                    evaluator.read_reg("sp").unwrap(),
                    evaluator.read_reg("r0").unwrap(),
                    evaluator.read_reg("r3").unwrap(),
                    evaluator.read_reg("r7").unwrap()
                ),
                _ => panic!("unexpected branch instruction!")
            }
        }
        println!();

        offset += insn.len()
    }

    // i want this: VarnodeData { AddressSpaceId::register_id(0x3usize), 0x20u64, 4usize }
    // which should be r0, as defined in the .sla as 
    // <varnode_sym name="r0" id="0x3" scope="0x0" space="register" offset="0x20" size="4">
    // i'm dumb translator has a method to get it for me.
    println!("result: {:?}", EvaluatorContext::read_vnd(&mut context, &translator.register_by_name("r0").unwrap()));
    println!("sp: {:?}", EvaluatorContext::read_vnd(&mut context, &translator.register_by_name("sp").unwrap()));

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