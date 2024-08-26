//! cortex-m system control space
//! 
//! based on armv7-m architecture reference manual DDIO403E
use nohash_hasher::IntMap;

use fugue_core::eval::fixed_state::FixedState;
use fugue_ir::Address;
use fugue_bv::BitVec;
use fugue_bytes::Endian;

use crate::context::AccessType;
use crate::sim::traits::Clocked;
use crate::peripheral;
use crate::peripheral::traits::MappedPeripheralState;


/// system control space
/// 
/// see manual section 3.2
#[derive(Clone)]
pub struct SCS(FixedState);

impl SCS {

}

impl Clocked for SCS {}

impl MappedPeripheralState for SCS {

    /// scs base address defined by architecture
    fn base(&self) -> Address {
        Address::from(0xE000E000u32)
    }

    /// scs address range defined by architecture [0xE000E000, 0xE000EFFF]
    fn size(&self) -> usize {
        0x1000usize
    }

    fn read_bytes(&self, address: &Address, size: usize) -> Result<&[u8], peripheral::Error> {
        let offset = address.offset() & 0xFFF;

        Ok(())
    }

    fn write_bytes(&mut self, address: &Address, bytes: &[u8]) -> Result<(), peripheral::Error> {
        
    }

    fn read_mem(&self, address: &Address, size: usize, endian: Endian) -> Result<BitVec, peripheral::Error> {
        
    }

    fn write_mem(&mut self, address: &Address, data: &BitVec, endian: Endian) -> Result<(), peripheral::Error> {
        
    }
}