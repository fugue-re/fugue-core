use std::cell::{Cell, Ref, RefCell};

use fugue_ir::disassembly::lift::ArenaVec;
use fugue_ir::disassembly::{IRBuilderArena, PCodeData};
use fugue_ir::error::Error;
use fugue_ir::Address;

use crate::lifter::{Lifter, PCode};

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct LiftedInsnProperties: u16 {
        const FALL        = 0b0000_0000_0000_0001;
        const BRANCH      = 0b0000_0000_0000_0010;
        const CALL        = 0b0000_0000_0000_0100;
        const RETURN      = 0b0000_0000_0000_1000;

        const INDIRECT    = 0b0000_0000_0001_0000;

        const BRANCH_DEST = 0b0000_0000_0010_0000;
        const CALL_DEST   = 0b0000_0000_0100_0000;

        // 1. instruction's address referenced as an immediate
        //    on the rhs of an assignment
        // 2. the instruction is a fall from padding
        const MAYBE_TAKEN = 0b0000_0000_1000_0000;

        // instruction is a semantic NO-OP
        const NOP         = 0b0000_0001_0000_0000;

        // instruction is a trap (e.g., UD2)
        const TRAP        = 0b0000_0010_0000_0000;

        // instruction falls into invalid
        const INVALID     = 0b0000_0100_0000_0000;

        // is contained within a function
        const IN_FUNCTION = 0b0000_1000_0000_0000;

        // is jump table target
        const IN_TABLE    = 0b0001_0000_0000_0000;

        // treat as invalid if repeated
        const NONSENSE    = 0b0010_0000_0000_0000;

        const HALT        = 0b0100_0000_0000_0000;

        const UNVIABLE    = Self::TRAP.bits() | Self::INVALID.bits();

        const DEST        = Self::BRANCH_DEST.bits() | Self::CALL_DEST.bits();
        const FLOW        = Self::BRANCH.bits() | Self::CALL.bits() | Self::RETURN.bits();

        const TAKEN       = Self::DEST.bits() | Self::MAYBE_TAKEN.bits();
    }
}

pub struct LiftedInsn<'a, 'b, T: 'a = ()> {
    address: Address,
    bytes: &'b [u8],
    properties: Cell<LiftedInsnProperties>,
    operations: RefCell<Option<ArenaVec<'a, PCodeData<'a>>>>,
    delay_slots: u8,
    length: u8,
    data: T,
}

impl<'a, 'b, T: 'a> LiftedInsn<'a, 'b, T> {
    pub fn address(&self) -> Address {
        self.address
    }

    pub fn properties(&self) -> LiftedInsnProperties {
        self.properties.get()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len()]
    }

    pub fn data(&self) -> &T {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut T {
        &mut self.data
    }

    pub fn len(&self) -> usize {
        self.length as _
    }

    pub fn pcode(
        &self,
        lifter: &mut Lifter,
        irb: &'a IRBuilderArena,
    ) -> Result<Ref<ArenaVec<'a, PCodeData<'a>>>, Error> {
        if let Some(operations) = self.try_pcode() {
            return Ok(operations);
        }

        self.operations
            .replace(Some(lifter.lift(irb, self.address, self.bytes)?.operations));

        self.pcode(lifter, irb)
    }

    pub fn try_pcode(&self) -> Option<Ref<ArenaVec<'a, PCodeData<'a>>>> {
        Ref::filter_map(self.operations.borrow(), |v| v.as_ref()).ok()
    }

    pub fn into_pcode(
        self,
        lifter: &mut Lifter,
        irb: &'a IRBuilderArena,
    ) -> Result<PCode<'a>, Error> {
        if let Some(operations) = self.operations.into_inner() {
            return Ok(PCode {
                address: self.address,
                operations,
                delay_slots: self.delay_slots,
                length: self.length,
            });
        }

        lifter.lift(irb, self.address, self.bytes)
    }
}

pub trait InsnLifter<'a, T: 'a = ()> {
    type Error;

    fn properties<'b>(
        &mut self,
        lifter: &mut Lifter,
        irb: &'a IRBuilderArena,
        address: Address,
        bytes: &'b [u8],
    ) -> Result<LiftedInsn<'a, 'b, T>, Self::Error>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultInsnLifter;

impl DefaultInsnLifter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<'a> InsnLifter<'a> for DefaultInsnLifter {
    type Error = Error;

    fn properties<'b>(
        &mut self,
        lifter: &mut Lifter,
        irb: &'a IRBuilderArena,
        address: Address,
        bytes: &'b [u8],
    ) -> Result<LiftedInsn<'a, 'b>, Self::Error> {
        let PCode {
            address,
            operations,
            delay_slots,
            length,
        } = lifter.lift(irb, address, bytes)?;

        Ok(LiftedInsn {
            address,
            bytes,
            operations: RefCell::new(Some(operations)),
            properties: Cell::new(LiftedInsnProperties::default()),
            delay_slots,
            length,
            data: (),
        })
    }
}

#[cfg(test)]
mod test {
    use fugue_ir::Address;

    use yaxpeax_arch::*;

    use yaxpeax_arm::armv7::DecodeError as ARMDecoderError;
    use yaxpeax_arm::armv7::InstDecoder as ARMInstDecoder;
    use yaxpeax_arm::armv7::Instruction as ARMInstruction;

    use super::*;
    use crate::language::LanguageBuilder;

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
