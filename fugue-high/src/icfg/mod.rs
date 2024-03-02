use fugue_ir::disassembly::lift::ArenaVec;
use fugue_ir::disassembly::{IRBuilderArena, PCodeData};
use fugue_ir::error::Error;
use fugue_ir::Address;

use crate::lifter::{Lifter, PCode};

pub struct PCodeProperties<'a, 'b> {
    address: Address,
    bytes: &'b [u8],
    properties: (),
    operations: Option<ArenaVec<'a, PCodeData<'a>>>,
    delay_slots: u8,
    length: u8,
}

impl<'a, 'b> PCodeProperties<'a, 'b> {
    pub fn force_pcode(
        &mut self,
        lifter: &mut Lifter,
        irb: &'a IRBuilderArena,
    ) -> Result<(), Error> {
        if self.operations.is_some() {
            return Ok(());
        }

        self.operations = Some(lifter.lift(irb, self.address, self.bytes)?.operations);

        Ok(())
    }

    pub fn into_pcode(
        self,
        lifter: &mut Lifter,
        irb: &'a IRBuilderArena,
    ) -> Result<PCode<'a>, Error> {
        if let Some(operations) = self.operations {
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

pub trait LifterArch {
    type Error;

    fn properties<'a, 'b>(
        &mut self,
        lifter: &mut Lifter,
        irb: &'a IRBuilderArena,
        address: Address,
        bytes: &'b [u8],
    ) -> Result<PCodeProperties<'a, 'b>, Self::Error>;
}

impl LifterArch for Lifter<'_> {
    type Error = Error;

    fn properties<'a, 'b>(
        &mut self,
        lifter: &mut Lifter,
        irb: &'a IRBuilderArena,
        address: Address,
        bytes: &'b [u8],
    ) -> Result<PCodeProperties<'a, 'b>, Self::Error> {
        let PCode {
            address,
            operations,
            delay_slots,
            length,
        } = lifter.lift(irb, address, bytes)?;

        Ok(PCodeProperties {
            address,
            bytes,
            operations: Some(operations),
            properties: (),
            delay_slots,
            length,
        })
    }
}
