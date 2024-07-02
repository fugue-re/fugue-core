//! concrete observer implementations
//! 

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
            print!(
                "{}: in{} [{:<50} := {:?}]\n",
                if i == 0 { "" } else { "            " }, i,
                format!("{}", input.display(&self.translator)),
                inputs[i]
            );
        }

        if let Some(ref out) = pcode.output {
            print!(
                "            : out [{:<50} := {:?}]\n",
                format!("{}", out.display(&self.translator)),
                output.as_ref().unwrap(),
            );
        } else {
            print!("            : out [None]\n");
        };

        Ok(())
    }
}