use std::cell::{Cell, RefCell};

use fugue_ir::disassembly::IRBuilderArena;
use fugue_ir::Address;

use thiserror::Error;

use yaxpeax_arch::*;
pub use yaxpeax_x86::{x86_32, x86_64};

pub use yaxpeax_x86::amd64::{
    DecodeError as X86_64DecoderError, InstDecoder as X86_64InstDecoder,
    Instruction as X86_64Instruction,
};
pub use yaxpeax_x86::protected_mode::{
    DecodeError as X86_32DecoderError, InstDecoder as X86_32InstDecoder,
    Instruction as X86_32Instruction,
};

use crate::ir::PCode;
use crate::lifter::{InsnLifter, LiftedInsn, LiftedInsnProperties, Lifter};

pub trait X86Arch: Arch {
    fn should_lift(insn: &Self::Instruction) -> bool;
}

impl X86Arch for x86_32 {
    fn should_lift(_insn: &Self::Instruction) -> bool {
        true
    }
}

impl X86Arch for x86_64 {
    fn should_lift(_insn: &Self::Instruction) -> bool {
        true
    }
}

pub struct X86InsnLifter<D>
where
    D: X86Arch,
{
    decoder: D::Decoder,
}

#[derive(Debug, Error)]
pub enum X86LifterError {
    #[error(transparent)]
    Decoder(anyhow::Error),
    #[error(transparent)]
    Lifter(#[from] fugue_ir::error::Error),
}

impl X86LifterError {
    pub fn decoder<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Decoder(e.into())
    }
}

impl<D> X86InsnLifter<D>
where
    D: X86Arch,
{
    pub fn new() -> X86InsnLifter<D> {
        Self::new_with(D::Decoder::default())
    }

    pub fn new_32() -> X86InsnLifter<x86_32> {
        X86InsnLifter::new_with(X86_32InstDecoder::default())
    }

    pub fn new_64() -> X86InsnLifter<x86_64> {
        X86InsnLifter::new_with(X86_64InstDecoder::default())
    }

    pub fn new_with(decoder: D::Decoder) -> Self {
        Self { decoder }
    }
}

impl<'a, D> InsnLifter<'a, D::Instruction> for X86InsnLifter<D>
where
    D: X86Arch,
    D::Instruction: 'a,
    for<'b> U8Reader<'b>: Reader<D::Address, D::Word>,
    <D::Address as AddressBase>::Diff: TryInto<u8>,
    <<D::Address as AddressBase>::Diff as TryInto<u8>>::Error:
        std::error::Error + Send + Sync + 'static,
{
    type Error = X86LifterError;

    fn properties<'b>(
        &mut self,
        lifter: &mut Lifter,
        irb: &'a IRBuilderArena,
        address: Address,
        bytes: &'b [u8],
    ) -> Result<LiftedInsn<'a, 'b, D::Instruction>, Self::Error> {
        let mut reader = yaxpeax_arch::U8Reader::new(bytes);

        let insn = self
            .decoder
            .decode(&mut reader)
            .map_err(X86LifterError::decoder)?;

        let size = insn
            .len()
            .to_const()
            .try_into()
            .map_err(X86LifterError::decoder)?;

        if D::should_lift(&insn) {
            let PCode {
                address,
                operations,
                delay_slots,
                length,
            } = lifter.lift(irb, address, bytes)?;

            Ok(LiftedInsn {
                address,
                bytes,
                properties: Cell::new(LiftedInsnProperties::default()),
                operations: RefCell::new(Some(operations)),
                delay_slots,
                length,
                data: insn,
            })
        } else {
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
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[ignore]
    #[allow(unused)]
    fn test() -> anyhow::Result<()> {
        let mut t0 = X86InsnLifter::<x86_32>::new();
        t0.properties(todo!(), todo!(), todo!(), todo!());
        Ok(())
    }
}
