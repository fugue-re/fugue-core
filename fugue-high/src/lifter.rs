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

    pub fn reset(&mut self) {
        let translator = *self.0.borrow_translator();
        let mut ctx = translator.context_database();

        // we preserve the old context database
        self.0.with_ctx_mut(|old_ctx| mem::swap(old_ctx, &mut ctx));

        *self = Self::new_with(translator, ctx);
    }
}
