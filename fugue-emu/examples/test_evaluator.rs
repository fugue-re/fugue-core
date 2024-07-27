
use fugue_emu::context::AccessType;
use fugue_ir::Address;
use fugue_bv::BitVec;
use fugue_bytes::Endian;
use fugue_core::language::LanguageBuilder;

use fugue_emu::context::concrete::*;
use fugue_emu::context::traits::*;
use fugue_emu::context::traits::observer as context_observer;
use fugue_emu::context::concrete::observer::RegAccessLogger;
use fugue_emu::eval;
use fugue_emu::eval::concrete::*;
use fugue_emu::eval::traits::Evaluator;
use fugue_emu::eval::traits::observer as eval_observer;
use fugue_emu::eval::concrete::observer::{ PCodeStdoutLogger, InsnStdoutLogger };


// a program that computes (((3 ** 2) ** 2) ** 2)
// compiled with xpack arm-none-eabi-gcc arm64 11.3.1 20220712
// arm-none-eabi-gcc main.c -mcpu=cortex-m4 -mthumb -nostdlib
pub static TEST_PROGRAM: &[u8] = &[
    // 00000000 <_start>:
    0x00, 0xf0, 0x01, 0xf8, //  0: bl  6 <main>
    // 00000004 <exit>:
    0xfe, 0xe7,             //  4: b.n 4 <exit>
    // 00000006 <main>:
    0x80, 0xb5,             //  6: push     {r7, lr}
    0x82, 0xb0,             //  8: sub      sp, #8
    0x00, 0xaf,             //  a: add      r7, sp, #0
    0x03, 0x23,             //  c: movs     r3, #3
    0x7b, 0x60,             //  e: str      r3, [r7, #4]
    0x00, 0x23,             // 10: movs     r3, #0
    0x3b, 0x60,             // 12: str      r3, [r7, #0]
    0x06, 0xe0,             // 14: b.n      24 <main+0x1e>
    0x78, 0x68,             // 16: ldr      r0, [r7, #4]
    0x00, 0xf0, 0x0c, 0xf8, // 18: bl       34 <square>
    0x78, 0x60,             // 1c: str      r0, [r7, #4]
    0x3b, 0x68,             // 1e: ldr      r3, [r7, #0]
    0x01, 0x33,             // 20: adds     r3, #1
    0x3b, 0x60,             // 22: str      r3, [r7, #0]
    0x3b, 0x68,             // 24: ldr      r3, [r7, #0]
    0x02, 0x2b,             // 26: cmp      r3, #2
    0xf5, 0xdd,             // 28: ble.n    16 <main+0x10>
    0x7b, 0x68,             // 2a: ldr      r3, [r7, #4]
    0x18, 0x46,             // 2c: mov      r0, r3
    0x08, 0x37,             // 2e: adds     r7, #8
    0xbd, 0x46,             // 30: mov      sp, r7
    0x80, 0xbd,             // 32: pop      {r7, pc}
    // 00000034 <square>:
    0x80, 0xb4,             // 34: push     {r7}
    0x83, 0xb0,             // 36: sub      sp, #12
    0x00, 0xaf,             // 38: add      r7, sp, #0
    0x78, 0x60,             // 3a: str      r0, [r7, #4]
    0x7b, 0x68,             // 3c: ldr      r3, [r7, #4]
    0x03, 0xfb, 0x03, 0xf3, // 3e: mul.w    r3, r3, r3
    0x18, 0x46,             // 42: mov      r0, r3
    0x0c, 0x37,             // 44: adds     r7, #12
    0xbd, 0x46,             // 46: mov      sp, r7
    0x80, 0xbc,             // 48: pop      {r7}
    0x70, 0x47,             // 4a: bx       lr
];

fn main() {
    // set up context
    let lang_builder = LanguageBuilder::new("./data/processors")
        .expect("language builder not instantiated");
    let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
        .expect("language failed to build");
    let lifter = lang.lifter();
    let irb = lifter.irb(1024);
    let mut context = ConcreteContext::new_with(&lang);

    assert_eq!(
        context.language().convention().name(),
        "default"
    );
    assert_eq!(
        context.language().translator().architecture(),
        &fugue_arch::ArchitectureDef::new("ARM", Endian::Little, 32usize, "Cortex")
    );

    let mem_base = Address::from(0x0u64);
    let aligned_size = 0x2000usize;

    context.map_mem(mem_base, aligned_size)
        .expect("map_mem() failed:");

    // load test program into memory
    context.write_bytes(mem_base, TEST_PROGRAM)
        .expect("write_bytes() failed to write program into memory");

    // initialize registers
    context.set_pc(&BitVec::from_u32(0x0u32, 64))
        .expect("failed to set pc");
    context.set_sp(&BitVec::from_u32(aligned_size as u32, 64))
        .expect("failed to set sp");

    // add register logger
    context.add_observer(
        context_observer::Observer::Reg("NG", AccessType::R | AccessType::W, RegAccessLogger::new_boxed())
    ).expect("failed to add register logger observer");

    // initialize evaluator
    let mut evaluator = ConcreteEvaluator::new();
    let insn_t = lifter.translator();
    let pcode_t = lifter.translator();
    let insn_logger = InsnStdoutLogger::new_with(insn_t);
    let pcode_logger = PCodeStdoutLogger::new_with(pcode_t);
    evaluator.register_observer(eval_observer::Observer::Insn(Box::new(insn_logger)))
        .expect("failed to register insn observer");
    evaluator.register_observer(eval_observer::Observer::PCode(Box::new(pcode_logger)))
        .expect("failed to register pcode observer");

    // debugging the CMP instruction issue
    evaluator.register_breakpoint(&Address::from(0x28u64), |context| {
        let ng_val = context.read_reg("NG")
            .map_err(eval::Error::from)?;
        println!("NG val: {:?}", ng_val);
        Ok(())
    }).expect("failed to register breakpoint");


    let halt_address = Address::from(0x4u64);
    let mut cycles = 0;
    while evaluator.pc().address() != halt_address {
        println!("pc: {:#x}", evaluator.pc().address().offset());
        if evaluator.pc().address.offset() == 0x26u64 {
            println!("breakpoint");
        }
        evaluator.step(&irb, &mut context)
            .expect("step failed:");
        cycles += 1;
    }

    // should've executed a bunch of instructions
    assert!(cycles > 10, "instructions executed: {}", cycles);

    let retval = context.read_reg("r0")
        .expect("failed to read register r0");

    assert_eq!(retval.to_i32().unwrap(), 6561, "retval: {:?}, cycles: {}", retval, cycles);
}
