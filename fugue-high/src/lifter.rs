use std::cell::{Cell, Ref, RefCell};
use std::mem;

use fugue_ir::disassembly::lift::{ArenaString, ArenaVec};
use fugue_ir::disassembly::{ContextDatabase, IRBuilderArena, PCodeData, ParserContext};
use fugue_ir::error::Error;
use fugue_ir::{Address, Translator};

use ouroboros::self_referencing;

#[derive(Debug)]
pub struct Insn<'a> {
    pub address: Address,
    pub mnemonic: ArenaString<'a>,
    pub operands: ArenaString<'a>,
    pub delay_slots: u8,
    pub length: u8,
}

impl<'a> Insn<'a> {
    pub fn address(&self) -> Address {
        self.address
    }

    pub fn mnemonic(&self) -> &str {
        &self.mnemonic
    }

    pub fn operands(&self) -> &str {
        &self.operands
    }

    pub fn delay_slots(&self) -> usize {
        self.delay_slots as _
    }

    pub fn len(&self) -> usize {
        self.length as _
    }
}

#[derive(Debug)]
pub struct PCode<'a> {
    pub address: Address,
    pub operations: ArenaVec<'a, PCodeData<'a>>,
    pub delay_slots: u8,
    pub length: u8,
}

impl<'a> PCode<'a> {
    pub fn address(&self) -> Address {
        self.address
    }

    pub fn operations(&self) -> &[PCodeData<'a>] {
        &self.operations
    }

    pub fn delay_slots(&self) -> usize {
        self.delay_slots as _
    }

    pub fn len(&self) -> usize {
        self.length as _
    }
}

#[self_referencing]
struct LifterInner<'a> {
    translator: &'a Translator,
    irb: IRBuilderArena,
    ctx: ContextDatabase,
    #[borrows(irb)]
    #[covariant]
    pctx: ParserContext<'a, 'this>,
}

#[repr(transparent)]
pub struct Lifter<'a>(LifterInner<'a>);

impl<'a> Clone for Lifter<'a> {
    fn clone(&self) -> Self {
        // we recreate based on the current context database
        let translator = *self.0.borrow_translator();
        let ctx = self.0.borrow_ctx().clone();

        Self::new_with(translator, ctx)
    }

    fn clone_from(&mut self, source: &Self) {
        // we only need to copy the context database
        let sctx = source.0.borrow_ctx().clone();
        self.0.with_ctx_mut(|ctx| *ctx = sctx);
    }
}

impl<'a> Lifter<'a> {
    pub fn new(translator: &'a Translator) -> Self {
        Self::new_with(translator, translator.context_database())
    }

    pub fn new_with(translator: &'a Translator, ctx: ContextDatabase) -> Self {
        let irb = IRBuilderArena::with_capacity(4096);

        Self(LifterInner::new(translator, irb, ctx, |irb| {
            ParserContext::empty(irb, translator.manager())
        }))
    }

    pub fn irb(&self, size: usize) -> IRBuilderArena {
        IRBuilderArena::with_capacity(size)
    }

    pub fn disassemble<'z>(
        &mut self,
        irb: &'z IRBuilderArena,
        address: impl Into<Address>,
        bytes: impl AsRef<[u8]>,
    ) -> Result<Insn<'z>, Error> {
        let bytes = bytes.as_ref();

        self.0.with_mut(|slf| {
            let address = address.into();
            let address_val = slf.translator.address(address.into());

            let (mnemonic, operand_str, delay_slots, length) = slf.translator.disassemble_aux(
                slf.ctx,
                slf.pctx,
                slf.irb,
                address_val,
                bytes,
                |fmt, delay_slots, length| -> Result<_, Error> {
                    let mnemonic = fmt.mnemonic_str(irb);
                    let operand_str = fmt.operands_str(irb);

                    Ok((mnemonic, operand_str, delay_slots, length))
                },
            )?;

            if length as usize > bytes.len() {
                return Err(Error::Disassembly(
                    fugue_ir::disassembly::Error::InstructionResolution,
                ));
            }

            Ok(Insn {
                address,
                mnemonic,
                operands: operand_str,
                delay_slots: delay_slots as u8,
                length: length as u8,
            })
        })
    }

    pub fn lift<'z>(
        &mut self,
        irb: &'z IRBuilderArena,
        address: impl Into<Address>,
        bytes: impl AsRef<[u8]>,
    ) -> Result<PCode<'z>, Error> {
        let bytes = bytes.as_ref();

        self.0.with_mut(|slf| {
            let address = address.into();
            let address_val = slf.translator.address(address.into());
            let mut irbb = irb.builder(&slf.translator);

            let pcode_raw = slf.translator.lift_pcode_raw_with(
                slf.ctx,
                slf.pctx,
                slf.irb,
                &mut irbb,
                address_val,
                bytes,
            )?;

            if pcode_raw.length as usize > bytes.len() {
                return Err(Error::Disassembly(
                    fugue_ir::disassembly::Error::InstructionResolution,
                ));
            }

            Ok(PCode {
                address,
                operations: pcode_raw.operations,
                delay_slots: pcode_raw.delay_slots as u8,
                length: pcode_raw.length as u8,
            })
        })
    }

    pub fn translator(&self) -> &Translator {
        self.0.borrow_translator()
    }

    pub fn reset(&mut self) {
        let translator = *self.0.borrow_translator();
        let mut ctx = translator.context_database();

        // we preserve the old context database
        self.0.with_ctx_mut(|old_ctx| mem::swap(old_ctx, &mut ctx));

        *self = Self::new_with(translator, ctx);
    }
}

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
    pub address: Address,
    pub bytes: &'b [u8],
    pub properties: Cell<LiftedInsnProperties>,
    pub operations: RefCell<Option<ArenaVec<'a, PCodeData<'a>>>>,
    pub delay_slots: u8,
    pub length: u8,
    pub data: T,
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

