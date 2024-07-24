//! concrete observer implementations
//! 

use fugue_core::lifter::Lifter;
use fugue_ir::Address;
use fugue_ir::disassembly::PCodeData;
use fugue_ir::translator::Translator;
use fugue_bv::BitVec;

use crate::eval;
use crate::eval::traits::*;



/// pcode step logging observer
#[derive(Clone)]
pub struct PCodeStdoutLogger {
    translator: Translator,
}

impl PCodeStdoutLogger {
    pub fn new_with(translator: &Translator) -> Self {
        Self {
            translator: translator.clone(),
        }
    }
}

impl observer::PCodeObserver for PCodeStdoutLogger {
    fn update(
        &mut self,
        pcode: &PCodeData,
        inputs: &Vec<BitVec>,
        output: &Option<BitVec>,
    ) -> Result<(), eval::Error> {
        
        // display the pcode data as follows:
        // <opcode>    : in1 [<varnode> = <value>]
        //             : in2 [<varnode> = <value>]
        //          <? : in3 [<varnode> = <value>]>
        //             : out [<varnode> = <value>]

        print!("{:<12}", format!("{:?}", pcode.opcode));
        
        for (i, input) in pcode.inputs.iter().enumerate() {
            let varnode_str = {
                if input.space().is_register() {
                    let name = self.translator.registers()
                        .get(input.offset(), input.size())
                        .unwrap();
                    format!("Register(name={}, offset={}, size={})", name, input.offset(), input.size())
                } else if input.space().is_constant() {
                    format!("Constant(value={:#x}, size={}", input.offset(), input.size())
                } else {
                    format!("Varnode(space={}, offset={:#x}, size={}", 
                        self.translator.manager().unchecked_space_by_id(input.space()).name(),
                        input.offset(),
                        input.size()
                    )
                }
            };

            print!(
                "{}: in{} [{:<50} := {:>15}]\n",
                if i == 0 { "" } else { "            " }, i,
                varnode_str,
                format!("{}", inputs[i]),
            );
        }

        if let Some(ref out) = pcode.output {
            let out_vnd_str = {
                if out.space().is_register() {
                    let name = self.translator.registers()
                        .get(out.offset(), out.size())
                        .unwrap();
                    format!("Register(name={}, offset={}, size={})", name, out.offset(), out.size())
                } else if out.space().is_constant() {
                    format!("Constant(value={:#x}, size={}", out.offset(), out.size())
                } else {
                    format!("Varnode(space={}, offset={:#x}, size={}", 
                        self.translator.manager().unchecked_space_by_id(out.space()).name(),
                        out.offset(),
                        out.size()
                    )
                }
            };
            print!(
                "            : out [{:<50} := {:>15}]\n",
                format!("{}", out.display(&self.translator)),
                format!("{}", output.as_ref().unwrap()),
            );
        } else {
            print!("            : out [None]\n");
        };

        Ok(())
    }
}

/// instruction step logging observer
#[derive(Clone)]
pub struct InsnStdoutLogger {
    translator: Translator,
}

impl InsnStdoutLogger {
    pub fn new_with(translator: &Translator) -> Self {
        Self {
            translator: translator.clone(),
        }
    }
}

impl observer::InsnObserver for InsnStdoutLogger {
    fn update(
        &mut self,
        address: &Address,
        insn_bytes: &[u8],
    ) -> Result<(), eval::Error> {
        // display the instruction mnemonic
        let mut lifter = Lifter::new(&self.translator);
        let arena = lifter.irb(1024);
        let insn = lifter.disassemble(&arena, *address, insn_bytes).unwrap();
        println!("{:x}: {:<6} {}", address.offset(), insn.mnemonic(), insn.operands());

        Ok(())
    }
}