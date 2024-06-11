use std::collections::VecDeque;

use fugue_ir::Address;

use crate::project::{Project, ProjectRawView};

pub struct ICFGBuilder<'a, R>
where
    R: ProjectRawView,
{
    project: &'a mut Project<R>,
    candidates: VecDeque<Address>,
}

impl<'a, R> ICFGBuilder<'a, R>
where
    R: ProjectRawView,
{
    pub fn new(project: &'a mut Project<R>) -> Self {
        Self {
            project,
            candidates: VecDeque::new(),
        }
    }

    pub fn add_candidate(&mut self, candidate: impl Into<Address>) {
        self.candidates.push_back(candidate.into());
    }

    pub fn add_candidates(&mut self, candidates: impl IntoIterator<Item = Address>) {
        self.candidates.extend(candidates);
    }
}

#[cfg(test)]
mod test {
    use std::cell::{Cell, RefCell};

    use fugue_ir::disassembly::IRBuilderArena;
    use fugue_ir::Address;

    use yaxpeax_arch::*;

    use yaxpeax_arm::armv7::DecodeError as ARMDecoderError;
    use yaxpeax_arm::armv7::InstDecoder as ARMInstDecoder;
    use yaxpeax_arm::armv7::Instruction as ARMInstruction;

    use crate::language::LanguageBuilder;
    use crate::lifter::*;

    #[test]
    #[ignore]
    fn test_arm32_props() -> anyhow::Result<()> {
        let lbuilder = LanguageBuilder::new("data")?;
        let language = lbuilder.build("ARM:LE:32:v7", "default")?;

        let memory = &[
            0x03, 0x00, 0x51, 0xE3, 0x0A, 0x00, 0x00, 0x9A, 0x00, 0x30, 0xA0, 0xE3, 0x01, 0x10,
            0x80, 0xE0, 0x03, 0x00, 0x80, 0xE2, 0x01, 0x20, 0xD0, 0xE4, 0x02, 0x30, 0x83, 0xE0,
            0x01, 0x00, 0x50, 0xE1, 0xFF, 0x30, 0x03, 0xE2, 0xFA, 0xFF, 0xFF, 0x1A, 0x00, 0x00,
            0x63, 0xE2, 0xFF, 0x00, 0x00, 0xE2, 0x1E, 0xFF, 0x2F, 0xE1, 0x00, 0x00, 0xA0, 0xE3,
            0x1E, 0xFF, 0x2F, 0xE1,
        ];

        let address = Address::from(0x00015E38u32);
        let mut off = 0usize;

        let mut lifter = language.lifter();
        let irb = lifter.irb(1024);

        struct ARMInsnLifter(ARMInstDecoder);

        impl ARMInsnLifter {
            pub fn new() -> Self {
                Self(ARMInstDecoder::armv7())
            }
        }

        impl<'a> InsnLifter<'a, ARMInstruction> for ARMInsnLifter {
            type Error = ARMDecoderError;

            fn properties<'b>(
                &mut self,
                _lifter: &mut Lifter,
                _irb: &'a IRBuilderArena,
                address: Address,
                bytes: &'b [u8],
            ) -> Result<LiftedInsn<'a, 'b, ARMInstruction>, Self::Error> {
                let mut reader = yaxpeax_arch::U8Reader::new(bytes);
                let insn = self.0.decode(&mut reader)?;
                let size = insn.len().to_const() as u8;

                Ok(LiftedInsn {
                    address,
                    bytes,
                    properties: Cell::new(LiftedInsnProperties::default()),
                    operations: RefCell::new(None),
                    delay_slots: 0,
                    length: size,
                    data: insn,
                })
            }
        }

        let mut plifter = ARMInsnLifter::new();

        while off < memory.len() {
            let lifted = plifter.properties(&mut lifter, &irb, address + off, &memory[off..])?;

            println!("--- insn @ {} ---", lifted.address());
            println!("{}", lifted.data());

            println!("--- pcode @ {} ---", lifted.address());
            for (i, op) in lifted.pcode(&mut lifter, &irb)?.iter().enumerate() {
                println!("{i:02} {}", op.display(language.translator()));
            }
            println!();

            off += lifted.len();
        }

        Ok(())
    }

    #[test]
    #[ignore]
    fn test_xtensa_props() -> anyhow::Result<()> {
        env_logger::try_init().ok();

        let lbuilder = LanguageBuilder::new("data")?;
        let language = lbuilder.build("Xtensa:LE:32:default", "default")?;

        let memory = &[
            0x36, 0x41, 0x00, 0x25, 0xFE, 0xFF, 0x0C, 0x1B, 0xAD, 0x02, 0x81, 0x4C, 0xFA, 0xE0,
            0x08, 0x00, 0x1D, 0xF0,
        ];

        let address = Address::from(0x40375C28u32);
        let mut off = 0usize;

        let mut lifter = language.lifter();
        let irb = lifter.irb(1024);

        let mut plifter = DefaultInsnLifter::new();

        while off < memory.len() {
            let lifted = plifter.properties(&mut lifter, &irb, address + off, &memory[off..])?;

            println!("--- pcode @ {} ---", lifted.address());
            for (i, op) in lifted.try_pcode().unwrap().iter().enumerate() {
                println!("{i:02} {}", op.display(language.translator()));
            }
            println!();

            off += lifted.len();
        }

        Ok(())
    }
}
