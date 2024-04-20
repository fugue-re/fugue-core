#![allow(unused_imports)]

use fugue;
use fugue::arch::ArchitectureDef;
use fugue::bytes::Endian;
use fugue::ir::{
    Translator,
    Address,
    AddressValue,
    LanguageDB,
    il::pcode::Operand,
    disassembly::IRBuilderArena
};

#[test]
fn test_load_languagedb() {
    // let ldefs_path = String::from("tests/languages/ARM/ARM.ldefs");
    let spec_path = String::from("tests/languages");
    let ldb = LanguageDB::from_directory_with(spec_path, true).unwrap();
    // let ldb = LanguageDB::from_file_with(ldefs_path, true).unwrap();
    assert_eq!(ldb.len(), 26);
}

#[test]
fn translation() {
    let mut translator = match Translator::from_file(
        "pc", 
        &ArchitectureDef::new(
            "ARM", 
            Endian::Little, 
            32, 
            "Cortex"),
        &Default::default(), 
        "tests/languages/ARM/ARM7_le.sla",
    ) {
        Err(err) => {panic!("{:?}", err)},
        Ok(translator) => translator,
    };

    translator.set_variable_default("TMode", 1);
    translator.set_variable_default("LRset", 0);
    translator.set_variable_default("spsr", 0);

    let bytes: [u8; 4] = *b"\xff\xf7\xad\xff";
    
    let mut context_db = translator.context_database();
    let ir_builder = IRBuilderArena::with_capacity(4096);

    let addr = translator.address(0x0u64);
    let mut offset: usize = 0;
    
    while offset < bytes.len() {
        let insn = translator.lift_pcode_raw(
            &mut context_db, 
            &ir_builder,
            addr + offset,
            &bytes[offset..]
        ).unwrap();
        println!("{}", insn.display(&translator));
        offset += insn.length();
    }
}