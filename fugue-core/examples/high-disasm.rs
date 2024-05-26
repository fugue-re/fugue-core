#![allow(unused_variables)]
use fugue_core::language::LanguageBuilder;
use fugue_ir::Address;

fn main() {
    // create the language builder, which will use the path specified
    // to search for processor sleigh specifications
    // in debug, path needs to be relative to fugue-core/fugue-core package
    // in run, path needs to be relative to top level fugue-core
    let lb = LanguageBuilder::new("data/processors")
        .expect("language builder not instantiated");

    // create the language based on the ghidra language string
    // and the convention
    // where do i find the language convention? apparently in default compiler_specs?
    // it seems to get loaded from 
    let lang = lb.build("ARM:LE:32:Cortex", "default")
        .expect("language failed to build");
    let mut lifter = lang.lifter();
    let ir_builder = lifter.irb(0x1000);

    // let bytes = b"\xff\xf7\xad\xff";
    // let bytes = b"\x80\xb4
    //             \x83\xb0
    //             \x00\xaf
    //             \x78\x60
    //             \x7b\x68
    //             \x03\xfb\x03\xf3
    //             \x18\x46
    //             \x0c\x37
    //             \xbd\x46
    //             \x80\xbc
    //             \x70\x47";
    let bytes = &[
        // main:
        0x80, 0xb5,             // push     {r7, lr}
        0x82, 0xb0,             // sub      sp, #8
        0x00, 0xaf,             // add      r7, sp, #0
        0x03, 0x23,             // movs     r3, #3
        0x7b, 0x60,             // str      r3, [r7, #4]
        0x00, 0x23,             // movs     r3, #0
        0x3b, 0x60,             // str      r3, [r7, #0]
        0x06, 0xe0,             // b.n      <main + 0x1e>
        0x78, 0x68,             // ldr      r0, [r7, #4]
        0xff, 0xf7, 0xfe, 0xff, // bl       0x2e <square>
        0x78, 0x60,             // str      r0, [r7, #4]
        0x3b, 0x68,             // ldr      r3, [r7, #0]
        0x01, 0x33,             // adds     r3, #1
        0x3b, 0x60,             // str      r3, [r7, #0]
        0x3b, 0x68,             // ldr      r3, [r7, #0]
        0x02, 0x2b,             // cmp      r3, #2
        0xf5, 0xdd,             // ble.n    <main + 0x10>
        0x7b, 0x68,             // ldr      r3, [r7, #4]
        0x18, 0x46,             // mov      r0, r3
        0x08, 0x37,             // adds     r7, #8
        0xbd, 0x46,             // mov      sp, r7
        0x80, 0xbd,             // pop      {r7, pc}
        // square:
        0x80, 0xb4,             // push     {r7}
        0x83, 0xb0,             // sub      sp, #12
        0x00, 0xaf,             // add      r7, sp, #0
        0x78, 0x60,             // str      r0, [r7, #4]
        0x7b, 0x68,             // ldr      r3, [r7, #4]
        0x03, 0xfb, 0x03, 0xf3, // mul.w    r3, r3, r3
        0x18, 0x46,             // mov      r0, r3
        0x0c, 0x37,             // adds     r7, #12
        0xbd, 0x46,             // mov      sp, r7
        0x80, 0xbc,             // pop      {r7}
        0x70, 0x47,             // bx lr
    ];
    let address = Address::from(0x0u64);
    let mut off = 0usize;

    while off < bytes.len() {
        let insn = lifter.disassemble(&ir_builder, address, &bytes[off..]).unwrap();
        let pcode = lifter.lift(&ir_builder, address, &bytes[off..]).unwrap();

        println!("--- insn @ {} ---", insn.address());
        println!("{} {}", insn.mnemonic(), insn.operands());
        println!();

        println!("--- pcode @ {} ---", pcode.address());
        for (i, op) in pcode.operations().iter().enumerate() {
            println!("{i:02} {}", op.display(lang.translator()));
        }
        println!();

        off += insn.len();
    }
}
