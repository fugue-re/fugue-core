use std::fs;
use std::fs::File;
use std::io::Write;

use fugue_ir::{disassembly::Opcode, Address};
use fugue_bv::BitVec;
use fugue_bytes::Endian;
use fugue_core::{ir::PCode, language::LanguageBuilder};

use csv;
use serde::Deserialize;

fn main() {
    // set up context
    let lang_builder = LanguageBuilder::new("./data/processors")
        .expect("language builder not instantiated");
    let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
        .expect("language failed to build");
    let mut lifter = lang.lifter();
    let irb = lifter.irb(1024);

    let mut log = File::create("./fugue-emu/examples/data/disasm.log").unwrap();

    // import binary file
    let bytes = fs::read("./fugue-emu/examples/data/hello_world.bin")
        .expect("could not load binary file");

    // import cfg
    #[derive(Debug, Deserialize)]
    struct BBlock {
        addr: u64,
        size: usize,
    }

    let cfg_string = fs::read_to_string("./fugue-emu/examples/data/hello_world.cfg.txt")
        .expect("could not load cfg file");
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .delimiter(b'\t')
        .from_reader(cfg_string.as_bytes());
    let mut blocks = vec![];
    for result in reader.deserialize::<BBlock>() {
        blocks.push(result.expect("failed to parse block"));
    }

    // let mut insns = vec![];

    for block in blocks.iter() {
        let base = block.addr & !1;
        let size = block.size;
        let mut offset = 0usize;
        while offset < size {
            let address = base + offset as u64;
            let bytes = &bytes[address as usize ..];
            match lifter.disassemble(&irb, address, bytes) {
                Err(err) => {
                    let s = format!(
                        "{:#x}: {:x?} => {:?}", 
                        address,
                        &bytes[..4],
                        err,
                    );
                    writeln!(&mut log, "{}", s);
                    println!("{}", s);
                    break;
                }
                Ok(insn) => {
                    let s = format!("{:#x}: {}\t{}", address, insn.mnemonic(), insn.operands());
                    writeln!(&mut log, "{}", s);
                    println!("{}", s);

                    match lifter.lift(&irb, address, bytes) {
                        Err(err) => {
                            let s = format!(
                                "{:#x}: {:x?} => {:?}",
                                address,
                                &bytes[..4],
                                err,
                            );
                            writeln!(&mut log, "{}", s);
                            println!("{}", s);
                            break;
                        }
                        Ok(pcode) => {
                            for pcodeop in pcode.operations() {
                                if pcodeop.opcode == Opcode::CallOther {
                                    let s = format!("{}", pcodeop.display(lifter.translator()));
                                    writeln!(&mut log, "{}", s);
                                    println!("{}", s);
                                }
                                // let s = format!("{}", pcodeop.display(lifter.translator()));
                                // writeln!(&mut log, "{}", s);
                                // println!("{}", s);
                            }
                        }
                    }
                    offset += insn.len();
                }
            }
        }
    }

    // for block in blocks.iter() {
    //     let base = block.addr & !1;
    //     let size = block.size;
    //     let mut offset = 0usize;
    //     'lifting: while offset < size {
    //         let lift_result = lifter.lift(
    //             &irb,
    //             base + offset as u64,
    //             &bytes[offset..]
    //         );
    //         match lift_result {
    //             Err(err) => {
    //                 let s = format!("lift @ {:#x}: {:x?} => {:?}", base, &bytes[offset..offset+4], err);
    //                 writeln!(&mut log, "{}", s).unwrap();
    //                 println!("{}", s);
    //                 break 'lifting;
    //             }
    //             Ok(pcode) => {
    //                 offset += pcode.len();
    //                 insns.push(pcode);
    //             }
    //         }
    //     }
    // }

    // for pcode in insns.iter() {
    //     let disasm_result = lifter.disassemble(
    //         &irb,
    //         pcode.address,
    //         &bytes[pcode.address.offset() as usize..],
    //     );
    //     match disasm_result {
    //         Err(err) => {
    //             let s = format!(
    //                 "{:#x}: {:x?} => {:?}\npcode ops: {}", 
    //                 pcode.address.offset(),
    //                 &bytes[pcode.address.offset() as usize ..pcode.address.offset() as usize +4],
    //                 err,
    //                 pcode.operations().len(),
    //             );
    //             writeln!(&mut log, "{}", s).unwrap();
    //             println!("{}", s);
    //         }
    //         Ok(disasm) => {
    //             let mut call_other = false;
    //             for pcodeop in pcode.operations() {
    //                 match pcodeop.opcode {
    //                     Opcode::CallOther => {
    //                         call_other = true;
    //                         break;
    //                     }
    //                     _ => {}
    //                 }
    //                 // println!("\t{}", pcodeop.display(lifter.translator()));
    //             }
    //             if disasm.mnemonic == "bl" {
    //                 let s = format!(
    //                     "bl insn: disasm_len={}, pcode_len={}, disasm_delay_slots={}, pcode_delay_slots={}",
    //                     disasm.len(),
    //                     pcode.len(),
    //                     disasm.delay_slots(),
    //                     pcode.delay_slots(),
    //                 );
    //                 writeln!(&mut log, "{}", s).unwrap();
    //                 println!("{}", s);
    //             }
    //             // if call_other {
    //                 let s = format!(
    //                     "{:#x}: {:x?}\t{}\t{}",
    //                     pcode.address.offset(),
    //                     &bytes[disasm.address.offset() as usize ..disasm.address.offset() as usize + disasm.len()],
    //                     disasm.mnemonic,
    //                     disasm.operands,
    //                 );
    //                 writeln!(&mut log, "{}", s).unwrap();
    //                 println!("{}", s);
    //             // }
    //         }
    //     }
    // }
}