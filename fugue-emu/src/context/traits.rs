//! context traits
//! 
//! defines various traits related to emulation contexts
use dyn_clone::{ DynClone, clone_trait_object };

use fugue_ir::{ Address, VarnodeData };
use fugue_bytes::Endian;

use crate::eval::traits::EvaluatorContext;
use crate::peripheral::traits::MappedPeripheralState;

use crate::context;

/// varnode context trait
/// 
/// all contexts need to be accessible via varnode
pub trait VarnodeContext<Data> {

    /// read data at the location of the specified varnode
    fn read_vnd(&mut self, var: &VarnodeData) -> Result<Data, context::Error>;

    /// write data to the location of the specified varnode
    fn write_vnd(&mut self, var: &VarnodeData, val: &Data) -> Result<(), context::Error>;
}

/// emulation context trait
/// 
/// this trait defines the high-level interface that a full context should
/// implement.
pub trait Context<'irb, Data>:
    EvaluatorContext<'irb, Data>
    + MemoryMapContext<Data>
    + RegisterContext<Data>
    + UniqueContext<Data>
    + Clone 
{ }

/// memory map context trait
/// 
/// describes the functionality of a context that implements a memory map
pub trait MemoryMapContext<Data>: VarnodeContext<Data> + Clone {
    /// the alignment size should be a multiple of 2 (and be fairly large)
    const ALIGNMENT_SIZE: u64 = context::types::DEFAULT_ALIGNMENT_SIZE;
    // todo: add peripheral alignment

    /// add a new region of memory to the memory map at the give base address
    /// and of the given size
    /// 
    /// an implementation should check both the base and size parameters
    /// against the trait's configured alignment size
    fn map_mem(
        &mut self,
        base: impl Into<Address>,
        size: usize,
    ) -> Result<(), context::Error>;

    /// add a peripheral to the memory map as mmio
    /// 
    /// peripheral base address and size need not be aligned to memory alignment size
    /// (peripheral address ranges tend to be smaller)
    fn map_mmio(
        &mut self,
        base: impl Into<Address>,
        peripheral: Box<dyn MappedPeripheralState>,
    ) -> Result<(), context::Error>;

    /// read a slice of bytes from memory at specified address
    fn read_bytes(&self, address: impl AsRef<Address>, size: usize) -> Result<&[u8], context::Error>;

    /// write bytes to memory at specified address
    fn write_bytes(&mut self, address: impl AsRef<Address>, bytes: &[u8]) -> Result<(), context::Error>;

    /// read data from memory at specified address
    fn read_mem(&self, address: impl AsRef<Address>, size: usize) -> Result<Data, context::Error>;

    /// write data to memory at specified address
    fn write_mem(&mut self, address: impl AsRef<Address>, data: &Data) -> Result<(), context::Error>;
}

/// mapped context trait
/// 
/// any state/context/peripheral that can be mapped into the emulationed
/// device's memory map must implement this trait
/// 
/// the offset parameters are expected to be translated prior to being passed.
/// translation should generally look like converting an Address into a usize,
/// then subtracting the base address of the corresponding context
/// 
/// note that while the mapped context trait does not explicitly declare that
/// an implementation must be cloneable, in practice they must be in order to 
/// enable backup and restore functionality
pub trait MappedContext<Data> {

    /// return the base address of the mapped context
    fn base(&self) -> Address;

    /// return the memory region size of the mapped context
    fn size(&self) -> usize;

    /// read a slice of bytes from memory at specified address
    fn read_bytes(&self, address: &Address, size: usize) -> Result<&[u8], context::Error>;

    /// write bytes to memory at specified address
    fn write_bytes(&mut self, address: &Address, bytes: &[u8]) -> Result<(), context::Error>;

    /// read data from memory at specified address
    fn read_mem(&self, address: &Address, size: usize, endian: Endian) -> Result<Data, context::Error>;

    /// write data to memory at specified address
    fn write_mem(&mut self, address: &Address, data: &Data, endian: Endian) -> Result<(), context::Error>;
}

/// register context trait
/// 
/// register states must implement accessibility based on register name
pub trait RegisterContext<Data>: VarnodeContext<Data> + Clone {

    /// read register data (returns cloned data)
    fn read_reg(&self, name: &str) -> Result<Data, context::Error>;

    /// write register with data
    fn write_reg(&mut self, name: &str, data: &Data) -> Result<(), context::Error>;
}

/// pcode temporaries context trait
/// 
/// must be made accessible via varnode
pub trait UniqueContext<Data>: VarnodeContext<Data> + Clone { }


/// context observer traits
/// 
/// observer traits may or may not need to be implemented
/// on a per-evaluator basis
/// 
/// context observers should only be triggered by varnode-based
/// accesses, i.e., only calls to context.read/write_vnd() should
/// trigger them
pub mod observer {
    use super::*;
    use fugue_bv::BitVec;
    use fugue_ir::AddressSpaceId;
    use crate::context;

    /// observer key type for hash map access
    #[derive(Clone, PartialEq, Eq, Hash)]
    pub struct ObserverKey {
        pub space_id: AddressSpaceId,
        pub offset: u64,
        pub size: usize,
        pub access: context::AccessType,
    }
    
    /// generic wrapper for context observers
    #[derive(Clone)]
    pub enum Observer<'a> {
        /// memory observer
        Mem(&'a Address, context::AccessType, Box<dyn MemObserver>),
        /// register observer
        Reg(&'a str, context::AccessType, Box<dyn RegObserver>),
        /// pcode temporaries observer
        Temp(&'a VarnodeData, context::AccessType, Box<dyn TempObserver>),
    }

    /// by default, memory observers should be updated after the corresponding
    /// operation has been performed
    pub trait MemObserver: DynClone {
        /// on update, the memory observer will be passed a shared reference
        /// to the value that was either written or read, the address,
        ///  and the memory access type
        fn update(
            &mut self,
            address: &Address,
            val: &BitVec,
            access: context::AccessType,
        ) -> Result<(), context::Error>;
    }
    clone_trait_object!(MemObserver);

    /// by default register observers should be updated after the corresponding
    /// operation has been performed
    pub trait RegObserver: DynClone {
        /// on update, the register observer will be passed a shared reference
        /// to the register name, offset, and size; as well as the value
        /// that was written or read, and the access type
        fn update(
            &mut self,
            name: &str,
            offset: u64,
            size: usize,
            value: &BitVec,
            access: context::AccessType,
        ) -> Result<(), context::Error>;
    }
    clone_trait_object!(RegObserver);

    /// by default, temporaries observers should be updated after the corresonding
    /// operation has been performed
    pub trait TempObserver: DynClone {
        /// on update the temporaries observer will be passed the offset and size
        /// of the temporary being modified, the value that was written or read,
        /// and the access type
        fn update(
            &mut self,
            offset: u64,
            size: usize,
            value: &BitVec,
            access: context::AccessType,
        ) -> Result<(), context::Error>;
    }
    clone_trait_object!(TempObserver);
}