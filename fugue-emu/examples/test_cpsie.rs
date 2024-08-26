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
// 0000038c <app_util_enable_irq>:
    0x80, 0xb4,              // 38c: push	{r7}
    0x00, 0xaf,              // 38e: add	r7, sp, #0
    0x07, 0x4b,              // 390: ldr	r3, [pc, #28]	; (3b0 <app_util_enable_irq+0x24>)
    0x1b, 0x68,              // 392: ldr	r3, [r3, #0]
    0x01, 0x3b,              // 394: subs	r3, #1
    0x06, 0x4a,              // 396: ldr	r2, [pc, #24]	; (3b0 <app_util_enable_irq+0x24>)
    0x13, 0x60,              // 398: str	r3, [r2, #0]
    0x05, 0x4b,              // 39a: ldr	r3, [pc, #20]	; (3b0 <app_util_enable_irq+0x24>)
    0x1b, 0x68,              // 39c: ldr	r3, [r3, #0]
    0x00, 0x2b,              // 39e: cmp	r3, #0
    0x01, 0xd1,              // 3a0: bne.n	3a6 <app_util_enable_irq+0x1a>
    0x62, 0xb6,              // 3a2: cpsie	i
    0x00, 0xbf,              // 3a4: nop
    0x00, 0xbf,              // 3a6: nop
    0xbd, 0x46,              // 3a8: mov	sp, r7
    0x5d, 0xf8, 0x04, 0x7b,  // 3aa: ldr.w	r7, [sp], #4
    0x70, 0x47,              // 3ae: bx	lr
    0x34, 0x00, 0x00, 0x20,  // 3b0: .word	0x20000034
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
    context.map_mem(Address::from(0x20000000u64), 0x1000)
        .expect("map_mem() failed:");

    // load test program into memory
    context.write_bytes(Address::from(0x38Cu64), TEST_PROGRAM)
        .expect("write_bytes() failed to write program into memory");
    context.write_bytes(Address::from(0x20000034u64), &[0x01])
        .expect("write_bytes() failed to write program into memory");

    // initialize registers
    context.set_pc(&BitVec::from_u32(0x38Cu32, 32))
        .expect("failed to set pc");
    context.set_sp(&BitVec::from_u32(aligned_size as u32, 32))
        .expect("failed to set sp");
    context.write_reg("lr", &BitVec::from_u32(0x0u32, 32))
        .expect("failed to set lr");

    // initialize evaluator
    let mut evaluator = ConcreteEvaluator::new();
    evaluator.set_pc(Address::from(0x38Cu64));
    let insn_t = lifter.translator();
    let pcode_t = lifter.translator();
    let insn_logger = InsnStdoutLogger::new_with(insn_t);
    let pcode_logger = PCodeStdoutLogger::new_with(pcode_t);
    evaluator.register_observer(eval_observer::Observer::Insn(Box::new(insn_logger)))
        .expect("failed to register insn observer");
    evaluator.register_observer(eval_observer::Observer::PCode(Box::new(pcode_logger)))
        .expect("failed to register pcode observer");

    let halt_address = Address::from(0x0u64);
    let mut cycles = 0;
    while evaluator.pc().address() != halt_address {
        println!("pc: {:#x}", evaluator.pc().address().offset());
        if evaluator.pc().address.offset() == 0x3a2u64 {
            println!("breakpoint");
        }
        evaluator.step(&irb, &mut context)
            .expect("step failed:");
        cycles += 1;
    }
}