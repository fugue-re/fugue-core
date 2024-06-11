use std::cell::{Cell, RefCell};

use fugue_ir::disassembly::IRBuilderArena;
use fugue_ir::Address;

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
use crate::lifter::{InsnLifter, LiftedInsn, LiftedInsnProperties, Lifter, LifterError};

#[sealed::sealed]
pub trait X86Arch: Arch {
    fn should_lift(insn: &Self::Instruction) -> bool;
}

#[sealed::sealed]
impl X86Arch for x86_32 {
    fn should_lift(insn: &Self::Instruction) -> bool {
        use yaxpeax_x86::protected_mode::Opcode;

        return match insn.opcode() {
            Opcode::JO
            | Opcode::JB
            | Opcode::JZ
            | Opcode::JA
            | Opcode::JS
            | Opcode::JP
            | Opcode::JL
            | Opcode::JG
            | Opcode::JMP
            | Opcode::JNO
            | Opcode::JNB
            | Opcode::JNZ
            | Opcode::JNA
            | Opcode::JNS
            | Opcode::JNP
            | Opcode::JGE
            | Opcode::JLE
            | Opcode::JMPF
            | Opcode::JMPE
            | Opcode::JECXZ => true,
            Opcode::CALL | Opcode::CALLF => true,
            Opcode::RETF | Opcode::RETURN => true,
            _ => false,
        };
    }
}

#[sealed::sealed]
impl X86Arch for x86_64 {
    fn should_lift(insn: &Self::Instruction) -> bool {
        use yaxpeax_x86::amd64::Opcode;

        return match insn.opcode() {
            Opcode::JO
            | Opcode::JB
            | Opcode::JZ
            | Opcode::JA
            | Opcode::JS
            | Opcode::JP
            | Opcode::JL
            | Opcode::JG
            | Opcode::JMP
            | Opcode::JNO
            | Opcode::JNB
            | Opcode::JNZ
            | Opcode::JNA
            | Opcode::JNS
            | Opcode::JNP
            | Opcode::JGE
            | Opcode::JLE
            | Opcode::JMPF
            | Opcode::JMPE => true,
            Opcode::CALL | Opcode::CALLF => true,
            Opcode::RETF | Opcode::RETURN => true,
            _ => false,
        };
    }
}

pub struct X86InsnLifter<D>
where
    D: X86Arch,
{
    decoder: D::Decoder,
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

impl<D> X86InsnLifter<D>
where
    D: X86Arch + 'static,
    for<'b> U8Reader<'b>: Reader<D::Address, D::Word>,
    <D::Address as AddressBase>::Diff: TryInto<u8>,
    <<D::Address as AddressBase>::Diff as TryInto<u8>>::Error:
        std::error::Error + Send + Sync + 'static,
{
    pub fn boxed<'a>(self) -> Box<dyn InsnLifter<'a>> {
        Box::new(self)
    }
}

impl<'a, D> InsnLifter<'a> for X86InsnLifter<D>
where
    D: X86Arch,
    for<'b> U8Reader<'b>: Reader<D::Address, D::Word>,
    <D::Address as AddressBase>::Diff: TryInto<u8>,
    <<D::Address as AddressBase>::Diff as TryInto<u8>>::Error:
        std::error::Error + Send + Sync + 'static,
{
    fn properties<'b>(
        &mut self,
        lifter: &mut Lifter,
        irb: &'a IRBuilderArena,
        address: Address,
        bytes: &'b [u8],
    ) -> Result<LiftedInsn<'a, 'b>, LifterError> {
        let mut reader = yaxpeax_arch::U8Reader::new(bytes);

        let insn = self
            .decoder
            .decode(&mut reader)
            .map_err(LifterError::decode)?;

        let size = insn
            .len()
            .to_const()
            .try_into()
            .map_err(LifterError::decode)?;

        if D::should_lift(&insn) {
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
                properties: Cell::new(LiftedInsnProperties::default()),
                operations: RefCell::new(Some(operations)),
                delay_slots,
                length,
            })
        } else {
            Ok(LiftedInsn {
                address,
                bytes,
                properties: Cell::new(LiftedInsnProperties::default()),
                operations: RefCell::new(None),
                delay_slots: 0,
                length: size,
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
