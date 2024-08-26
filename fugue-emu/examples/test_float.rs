
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


// function taken from sample nrf52 gpiote to enable an irq
// compiled with xpack arm-none-eabi-gcc arm64 11.3.1 20220712
pub static TEST_PROGRAM: &[u8] = &[
// 00000000 <_start>:
    0x00, 0xf0, 0x2d, 0xf8,  //    0: bl            5e <main>
// 00000004 <exit>:
    0xfe, 0xe7,              //    4: b.n           4 <exit>
// 00000006 <operations>:
    0x80, 0xb4,              //    6: push          {r7}
    0x85, 0xb0,              //    8: sub           sp, #20
    0x00, 0xaf,              //    a: add           r7, sp, #0
    0x87, 0xed, 0x01, 0x0a,  //    c: vstr          s0, [r7, #4]
    0xc7, 0xed, 0x00, 0x0a,  //   10: vstr          s1, [r7]
    0x4f, 0xf0, 0x00, 0x03,  //   14: mov.w         r3, #0
    0xfb, 0x60,              //   18: str           r3, [r7, #12]
    0x97, 0xed, 0x03, 0x7a,  //   1a: vldr          s14, [r7, #12]
    0xd7, 0xed, 0x01, 0x7a,  //   1e: vldr          s15, [r7, #4]
    0x77, 0xee, 0x27, 0x7a,  //   22: vadd.f32      s15, s14, s15
    0xc7, 0xed, 0x03, 0x7a,  //   26: vstr          s15, [r7, #12]
    0x97, 0xed, 0x03, 0x7a,  //   2a: vldr          s14, [r7, #12]
    0xd7, 0xed, 0x00, 0x7a,  //   2e: vldr          s15, [r7]
    0x77, 0xee, 0x67, 0x7a,  //   32: vsub.f32      s15, s14, s15
    0xc7, 0xed, 0x03, 0x7a,  //   36: vstr          s15, [r7, #12]
    0x97, 0xed, 0x03, 0x7a,  //   3a: vldr          s14, [r7, #12]
    0xd7, 0xed, 0x01, 0x7a,  //   3e: vldr          s15, [r7, #4]
    0x67, 0xee, 0x27, 0x7a,  //   42: vmul.f32      s15, s14, s15
    0xc7, 0xed, 0x03, 0x7a,  //   46: vstr          s15, [r7, #12]
    0xfb, 0x68,              //   4a: ldr           r3, [r7, #12]
    0x07, 0xee, 0x90, 0x3a,  //   4c: vmov          s15, r3
    0xb0, 0xee, 0x67, 0x0a,  //   50: vmov.f32      s0, s15
    0x14, 0x37,              //   54: adds          r7, #20
    0xbd, 0x46,              //   56: mov           sp, r7
    0x5d, 0xf8, 0x04, 0x7b,  //   58: ldr.w         r7, [sp], #4
    0x70, 0x47,              //   5c: bx            lr
// 0000005e <main>:
    0x80, 0xb5,              //   5e: push          {r7, lr}
    0x82, 0xb0,              //   60: sub           sp, #8
    0x00, 0xaf,              //   62: add           r7, sp, #0
    0xf0, 0xee, 0x04, 0x0a,  //   64: vmov.f32      s1, #4	; 0x40200000  2.5
    0xb1, 0xee, 0x0e, 0x0a,  //   68: vmov.f32      s0, #30	; 0x40f00000  7.5
    0xff, 0xf7, 0xcb, 0xff,  //   6c: bl            6 <operations>
    0x87, 0xed, 0x01, 0x0a,  //   70: vstr          s0, [r7, #4]
    0x00, 0xbf,              //   74: nop         
    0x08, 0x37,              //   76: adds          r7, #8
    0xbd, 0x46,              //   78: mov           sp, r7
    0x80, 0xbd,              //   7a: pop           {r7, pc}
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
    context.write_bytes(Address::from(0x0u64), TEST_PROGRAM)
        .expect("write_bytes() failed to write program into memory");
    
    // initialize registers
    context.set_pc(&BitVec::from_u32(0x38Cu32, 32))
        .expect("failed to set pc");
    context.set_sp(&BitVec::from_u32(aligned_size as u32, 32))
        .expect("failed to set sp");

    // initialize evaluator
    let mut evaluator = ConcreteEvaluator::new();
    evaluator.set_pc(Address::from(0x0u64));
    let insn_t = lifter.translator();
    let pcode_t = lifter.translator();
    let insn_logger = InsnStdoutLogger::new_with(insn_t);
    let pcode_logger = PCodeStdoutLogger::new_with(pcode_t);
    evaluator.register_observer(eval_observer::Observer::Insn(Box::new(insn_logger)))
        .expect("failed to register insn observer");
    evaluator.register_observer(eval_observer::Observer::PCode(Box::new(pcode_logger)))
        .expect("failed to register pcode observer");

    let halt_address = Address::from(0x4u64);
    let mut cycles = 0;
    while evaluator.pc().address() != halt_address {
        println!("pc: {:#x}", evaluator.pc().address().offset());
        // if evaluator.pc().address.offset() == 0x3a2u64 {
        //     println!("breakpoint");
        // }
        evaluator.step(&irb, &mut context)
            .expect("step failed:");
        cycles += 1;
    }

    println!("cycles: {}", cycles);
}