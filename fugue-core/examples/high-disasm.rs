#![allow(unused_variables)]
use fugue::high::language::LanguageBuilder;
use fugue::ir::Address;

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
    let bytes = b"\x80\xb4
                \x83\xb0
                \x00\xaf
                \x78\x60
                \x7b\x68
                \x03\xfb\x03\xf3
                \x18\x46
                \x0c\x37
                \xbd\x46
                \x80\xbc
                \x70\x47";
    let address = Address::from(0x0u64);

    let insn = lifter.disassemble(&ir_builder, address, &bytes).unwrap();
    let pcode = lifter.lift(&ir_builder, address, &bytes).unwrap();

    println!("--- insn @ {} ---", insn.address());
    println!("{} {}", insn.mnemonic(), insn.operands());
    println!();

    println!("--- pcode @ {} ---", pcode.address());
    for (i, op) in pcode.operations().iter().enumerate() {
        println!("{i:02} {}", op.display(lang.translator()));
    }
    println!();
}
