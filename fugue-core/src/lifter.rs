use std::cell::{Cell, Ref, RefCell};

use fugue_ir::disassembly::lift::ArenaVec;
use fugue_ir::disassembly::{ContextDatabase, IRBuilderArena, PCodeData, PCodeRaw};
use fugue_ir::error::Error;
use fugue_ir::il::instruction::Instruction;
use fugue_ir::translator::TranslationContext;
use fugue_ir::{Address, Translator};

use smallvec::SmallVec;
use thiserror::Error;

use crate::ir::{Insn, PCode};

#[derive(Debug, Error)]
pub enum LifterError {
    #[error(transparent)]
    Decode(anyhow::Error),
    #[error(transparent)]
    Lift(anyhow::Error),
}

impl LifterError {
    pub fn decode<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Decode(e.into())
    }

    pub fn decode_with<M>(m: M) -> Self
    where
        M: std::fmt::Debug + std::fmt::Display + Send + Sync + 'static,
    {
        Self::Decode(anyhow::Error::msg(m))
    }

    pub fn lift<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Lift(e.into())
    }

    pub fn lift_with<M>(m: M) -> Self
    where
        M: std::fmt::Debug + std::fmt::Display + Send + Sync + 'static,
    {
        Self::Lift(anyhow::Error::msg(m))
    }
}

#[derive(Clone)]
#[repr(transparent)]
pub struct Lifter<'a>(TranslationContext<'a>);

impl<'a> Lifter<'a> {
    pub fn new(translator: &'a Translator) -> Self {
        Self(TranslationContext::new(translator))
    }

    pub fn new_with(translator: &'a Translator, ctx: ContextDatabase) -> Self {
        Self(TranslationContext::new_with(translator, ctx))
    }

    pub fn irb(&self, size: usize) -> IRBuilderArena {
        self.0.irb(size)
    }

    pub fn disassemble<'z>(
        &mut self,
        irb: &'z IRBuilderArena,
        address: impl Into<Address>,
        bytes: impl AsRef<[u8]>,
    ) -> Result<Insn<'z>, Error> {
        let address = address.into();
        let bytes = bytes.as_ref();

        let Instruction {
            mnemonic,
            operands,
            delay_slots,
            length,
            ..
        } = self.0.disassemble(irb, address, bytes)?;

        Ok(Insn {
            address,
            mnemonic,
            operands,
            delay_slots: delay_slots as u8,
            length: length as u8,
        })
    }

    pub fn lift<'z>(
        &mut self,
        irb: &'z IRBuilderArena,
        address: impl Into<Address>,
        bytes: impl AsRef<[u8]>,
    ) -> Result<PCode<'z>, Error> {
        let address = address.into();
        let bytes = bytes.as_ref();

        let PCodeRaw {
            operations,
            delay_slots,
            length,
            ..
        } = self.0.lift(irb, address, bytes)?;

        Ok(PCode {
            address,
            operations,
            delay_slots: delay_slots as u8,
            length: length as u8,
        })
    }

    pub fn translator(&self) -> &Translator {
        self.0.translator()
    }

    pub fn context(&self) -> &ContextDatabase {
        self.0.context()
    }

    pub fn reset(&mut self) {
        self.0.reset();
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

impl Default for LiftedInsnProperties {
    fn default() -> Self {
        Self::FALL
    }
}

pub struct LiftedInsn<'input, 'lifter> {
    pub address: Address,
    pub bytes: &'input [u8],
    pub properties: Cell<LiftedInsnProperties>,
    pub operations: RefCell<Option<ArenaVec<'lifter, PCodeData<'lifter>>>>,
    pub delay_slots: u8,
    pub length: u8,
}

impl<'input, 'lifter> LiftedInsn<'input, 'lifter> {
    pub fn address(&self) -> Address {
        self.address
    }

    pub fn properties(&self) -> LiftedInsnProperties {
        self.properties.get()
    }

    pub fn is_flow(&self) -> bool {
        self.properties().contains(LiftedInsnProperties::FLOW)
    }

    pub fn has_fall(&self) -> bool {
        self.properties().contains(LiftedInsnProperties::FALL)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len()]
    }

    pub fn len(&self) -> usize {
        self.length as _
    }

    pub fn pcode(
        &self,
        lifter: &mut Lifter,
        irb: &'lifter IRBuilderArena,
    ) -> Result<Ref<ArenaVec<'lifter, PCodeData<'lifter>>>, Error> {
        if let Some(operations) = self.try_pcode() {
            return Ok(operations);
        }

        self.operations
            .replace(Some(lifter.lift(irb, self.address, self.bytes)?.operations));

        self.pcode(lifter, irb)
    }

    pub fn try_pcode(&self) -> Option<Ref<ArenaVec<'lifter, PCodeData<'lifter>>>> {
        Ref::filter_map(self.operations.borrow(), |v| v.as_ref()).ok()
    }

    pub fn into_pcode(
        self,
        lifter: &mut Lifter,
        irb: &'lifter IRBuilderArena,
    ) -> Result<PCode<'lifter>, Error> {
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

    pub fn local_targets(
        &self,
        lifter: &mut Lifter,
        irb: &'lifter IRBuilderArena,
    ) -> Result<SmallVec<[Address; 2]>, Error> {
        todo!()
    }

    pub fn global_targets(
        &self,
        lifter: &mut Lifter,
        irb: &'lifter IRBuilderArena,
    ) -> Result<SmallVec<[Address; 2]>, Error> {
        todo!()
    }
}

pub trait InsnLifter {
    fn properties<'input, 'lifter>(
        &mut self,
        lifter: &mut Lifter,
        irb: &'lifter IRBuilderArena,
        address: Address,
        bytes: &'input [u8],
    ) -> Result<LiftedInsn<'input, 'lifter>, LifterError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultInsnLifter;

impl DefaultInsnLifter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn boxed(self) -> Box<dyn InsnLifter> {
        Box::new(self)
    }
}

impl InsnLifter for DefaultInsnLifter {
    fn properties<'input, 'lifter>(
        &mut self,
        lifter: &mut Lifter,
        irb: &'lifter IRBuilderArena,
        address: Address,
        bytes: &'input [u8],
    ) -> Result<LiftedInsn<'input, 'lifter>, LifterError> {
        let PCode {
            address,
            operations,
            delay_slots,
            length,
        } = lifter
            .lift(irb, address, bytes)
            .map_err(LifterError::lift)?;

        Ok(LiftedInsn {
            address,
            bytes,
            operations: RefCell::new(Some(operations)),
            properties: Cell::new(LiftedInsnProperties::default()),
            delay_slots,
            length,
        })
    }
}
