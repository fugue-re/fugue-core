//! concrete context
//! 
//! the concrete context is designed for use with the concrete evaluator and
//! as a basis for primarily model-based firmware rehosting.
//! 
//! it is responsible for tracking memory/register/temporary state data,
//! some processor state data (such as program counter edges), caching lifted
//! instructions, managing memory mapped access hooks, peripherals, etc.
use std::sync::Arc;

use nohash_hasher::IntMap;
use ahash::AHashMap as Map;
#[allow(unused_imports)]
use parking_lot::{ RwLock, RwLockReadGuard };

use fugue_ir::{
    convention::Convention, disassembly::{ lift::IRBuilderArena, Opcode }, space::AddressSpace, Address, AddressSpaceId, VarnodeData
};
use fugue_bv::BitVec;
use fugue_core::language::Language;
use fugue_core::ir::PCode;

use crate::context;
use crate::context::traits::*;
use crate::context::types::*;
use crate::context::traits::observer::{
    MemObserver, RegObserver, Observer, ObserverKey,
};
use crate::eval;
use crate::eval::traits::EvaluatorContext;

pub mod observer;
pub mod state;
pub use state::*;

type ObserverIdx = usize;

/// concrete context
/// 
/// a context for a concrete evaluator that holds all state information
#[derive(Clone)]
pub struct ConcreteContext<'irb> {
    // state data
    memory_map: ConcreteMemoryMap,
    regs: ConcreteRegisters,
    tmps: ConcreteTemps,
    observers: Map<ObserverKey, Vec<ObserverIdx>>,
    mem_observers: Vec<(Box<dyn MemObserver>, Vec<ObserverKey>)>,
    reg_observers: Vec<(Box<dyn RegObserver>, Vec<ObserverKey>)>,

    // meta
    pc: VarnodeData,
    // endian: Endian,
    lang: Language,
    translation_cache: Arc<RwLock<IntMap< u64, LiftResult<'irb> >>>,
}

impl<'irb> ConcreteContext<'irb> {

    /// creates a new concrete context
    pub fn new_with(lang: &Language) -> Self {
        Self {
            memory_map: ConcreteMemoryMap::new_with(lang.translator()),
            regs: ConcreteRegisters::new_with(lang.translator()),
            tmps: ConcreteTemps::new_with(lang.translator()),
            observers: Map::default(),
            mem_observers: Vec::new(),
            reg_observers: Vec::new(),

            pc: lang.translator().program_counter().clone(),
            // endian: if lang.translator().is_big_endian() { Endian::Big } else { Endian::Little },
            lang: lang.clone(),
            translation_cache: Arc::new(RwLock::new(IntMap::default())),
        }
    }

    /// get shared reference to the context's language
    pub fn language(&self) -> &Language {
        &self.lang
    }

    /// get shared reference to context's convention
    pub fn convention(&self) -> &Convention {
        self.lang.convention()
    }

    /// read current pc value
    /// 
    /// todo: make read/write important registers part of the trait
    pub fn get_pc(&self) -> Result<BitVec, context::Error> {
        self.regs.read_reg_by_vnd(&self.pc)
    }

    /// write to current pc value
    pub fn set_pc(&mut self, val: &BitVec) -> Result<(), context::Error> {
        let val = val.unsigned_cast(self.pc.bits());
        self.regs.write_vnd(&self.pc, &val)
    }

    /// read current sp value
    pub fn get_sp(&self) -> Result<BitVec, context::Error> {
        let vnd = self.convention().stack_pointer().varnode().clone();
        self.regs.read_reg_by_vnd(&vnd)
    }

    pub fn set_sp(&mut self, val: &BitVec) -> Result<(), context::Error> {
        let vnd = self.convention().stack_pointer().varnode().clone();
        self.regs.write_vnd(&vnd, val)
    }

    pub fn default_space(&self) -> &AddressSpace {
        self.lang.translator().manager().default_space_ref()
    }

    fn add_observer_keys(
        &mut self,
        space_id: AddressSpaceId,
        offset: u64,
        size: usize,
        access: AccessType,
        idx: &ObserverIdx,
    ) -> Result<Vec<ObserverKey>, context::Error> {
        let mut keys = vec![];
        for (name, access_type) in access.iter_names() {
            let key = ObserverKey { space_id, offset, size, access: access_type };
            if !self.observers.contains_key(&key) {
                self.observers.insert(key.clone(), vec![idx.clone()]);
            } else {
                self.observers.get_mut(&key).unwrap().push(idx.clone());
            }
            keys.push(key);
        }
        if keys.len() > 0 {
            Ok(keys)
        } else {
            Err(context::Error::Observer(String::from("no access type specified")))
        }
    }

    /// add an observer to context
    /// note: the observer will be consumed
    pub fn add_observer(
        &mut self,
        observer: Observer<'static>,
    ) -> Result<(), context::Error> {
        match observer {
            Observer::Mem(address, access, obs) => {
                let space = self.default_space();
                let idx = self.mem_observers.len();
                let keys = self.add_observer_keys(
                    space.id(),
                    address.offset(),
                    space.address_size(),
                    access,
                    &idx,
                )?;
                
                self.mem_observers.push((obs, keys));
                Ok(())
            }
            Observer::Reg(name, access, obs) => {
                let vnd = self.lang.translator().register_by_name(name)
                    .ok_or(context::Error::Observer(format!("register {} does not exist", name)))?;
                let idx = self.reg_observers.len();
                let keys = self.add_observer_keys(
                    vnd.space(),
                    vnd.offset(),
                    vnd.size(),
                    access,
                    &idx,
                )?;

                self.reg_observers.push((obs, keys));
                Ok(())
            }
            _ => { Err(context::Error::Observer(String::from("observer type not supported"))) }
        }
    }
}

impl<'irb> VarnodeContext<BitVec> for ConcreteContext<'irb> {
    fn read_vnd(&mut self, var: &VarnodeData) -> Result<BitVec, context::Error> {
        let key = ObserverKey {
            space_id: var.space(),
            offset: var.offset(),
            size: var.size(),
            access: AccessType::R,
        };
        let spc = var.space();
        if spc.is_constant() {
            Ok(BitVec::from_u64(var.offset(), var.bits()))
        } else if spc.is_register() {
            let val = self.regs.read_vnd(var)?;
            if let Some(observers_idx) = self.observers.get_mut(&key) {
                let name = self.lang.translator().registers().get(key.offset, key.size).unwrap();
                for idx in observers_idx.iter() {
                    if let Some((obs, _keys)) = self.reg_observers.get_mut(*idx) {
                        obs.update(name, var.offset(), var.size(), &val, AccessType::R)?;
                    }
                }
            }
            Ok(val)
        } else if spc.is_unique() {
            self.tmps.read_vnd(var)
        } else if spc.is_default() {
            let val = self.memory_map.read_vnd(var)?;
            if let Some(observers_idx) =  self.observers.get_mut(&key) {
                for idx in observers_idx.iter() {
                    if let Some((obs, _keys)) = self.mem_observers.get_mut(*idx) {
                        obs.update(&Address::from(key.offset), &val, AccessType::R)?;
                    }
                }
            }
            Ok(val)
        } else {
            Err(context::Error::InvalidVarnode(var.clone()))
        }
    }

    fn write_vnd(&mut self, var: &VarnodeData, val: &BitVec) -> Result<(), context::Error> {
        let key = ObserverKey {
            space_id: var.space(),
            offset: var.offset(),
            size: var.size(),
            access: AccessType::W,
        };
        let spc = var.space();
        if spc.is_constant() {
            panic!("cannot write to constant Varnode!");
        } else if spc.is_register() {
            self.regs.write_vnd(var, val)?;

            if let Some(observers_idx) = self.observers.get_mut(&key) {
                let name = self.lang.translator().registers().get(key.offset, key.size).unwrap();
                for idx in observers_idx.iter() {
                    if let Some((obs, _keys)) = self.reg_observers.get_mut(*idx) {
                        obs.update(name, var.offset(), var.size(), val, AccessType::W)?;
                    }
                }
            }
            Ok(())
        } else if spc.is_unique() {
            self.tmps.write_vnd(var, val)
        } else if spc.is_default() {
            self.memory_map.write_vnd(var, val)?;

            if let Some(observers_idx) =  self.observers.get_mut(&key) {
                for idx in observers_idx.iter() {
                    if let Some((obs, _keys)) = self.mem_observers.get_mut(*idx) {
                        obs.update(&Address::from(key.offset), val, AccessType::W)?;
                    }
                }
            }
            Ok(())
        } else {
            Err(context::Error::InvalidVarnode(var.clone()))
        }
    }
}

/// the EvaluatorContext implementation for ConcreteContext will use the BitVec
/// as the associated Data type
impl<'irb> EvaluatorContext<'irb, BitVec> for ConcreteContext<'irb> {

    fn lift_block(
        &mut self,
        address: impl Into<Address>,
        irb: &'irb IRBuilderArena,
    ) -> TranslationBlock {
        let mut lifter = self.lang.lifter();
        let base = address.into();
        let mut offsets = vec![0usize];
        // the largest instruction in x86 is 15 bytes
        const MAX_INSN_SIZE: usize = 16;

        'lifting: loop {
            let offset = offsets.last().unwrap();
            let address = base + *offset as u64;

            let read_result = self.read_bytes(&address, MAX_INSN_SIZE);
            if let Err(err) = read_result {
                // read from memory failed
                self.translation_cache.write()
                    .insert(address.offset(), Err(err));
                break 'lifting;
            }
            let bytes = read_result.unwrap();
            let lift_result = lifter.lift(irb, address, bytes);
            if let Err(err) = lift_result {
                // lift failed
                self.translation_cache.write()
                    .insert(address.offset(), Err(err.into()));
                break 'lifting;
            } else {
                // lift succeeded
                let pcode = lift_result.unwrap();
                // update offsets for translation block
                offsets.push(offset + pcode.len());

                // check if the instruction is branching
                let mut is_branch = false;
                match pcode.operations.last().unwrap().opcode {
                    Opcode::Branch | Opcode::CBranch | Opcode::IBranch |
                    Opcode::Call | Opcode::ICall | Opcode::Return => {
                        // usually we can tell if the last opcode is branching
                        is_branch = true;
                    },
                    _ => {
                        // otherwise we need to check if the pc gets written to
                        // todo: there's probably a way to streamline this somehow, 
                        // maybe by only checking certain opcodes or having the PCode
                        // also keep track of the live varnodes
                        // that could be useful for checking liveness...
                        'inner: for pcodedata in pcode.operations.iter() {
                            if let Some(vnd) = pcodedata.output {
                                if vnd == self.pc {
                                    is_branch = true;
                                    break 'inner;
                                }
                            }
                        } // 'inner
                    },
                }

                self.translation_cache.write()
                    .insert(address.offset(), Ok(Arc::new(pcode)));

                if is_branch {
                    break 'lifting;
                }
            };
        } // 'lifting

        // after finished lifting, return a placeholder translation block
        // to represent what was lifted
        let size = offsets.pop().unwrap();
        let bytes = if size > 0 { 
            Vec::from(self.read_bytes(base, size).unwrap())
        } else {
            vec![]
        };
        
        TranslationBlock { base, insn_offsets: offsets, bytes }
    }

    fn fetch(&self, address: impl Into<Address>) -> Result<Arc<PCode<'irb>>, eval::Error> {
        let address = address.into();
        let lift_result = self.translation_cache.read()
            .get(&address.offset())
            .ok_or(eval::Error::TranslationCache(address.clone()))?
            .clone();

        lift_result.map_err(eval::Error::from)
    }

    fn fork(&self) -> Self {
        self.clone()
    }

}

impl<'irb> MemoryMapContext<BitVec> for ConcreteContext<'irb> {

    fn map_mem(
        &mut self,
        base: impl Into<Address>,
        size: usize,
    ) -> Result<(), context::Error> {
        self.memory_map.map_mem(base, size)
    }

    fn map_mmio(
        &mut self,
        base: impl Into<Address>,
        peripheral: Box<dyn crate::peripheral::traits::MappedPeripheralState>,
    ) -> Result<(), context::Error> {
        self.memory_map.map_mmio(base, peripheral)
    }

    fn read_bytes(&self, address: impl AsRef<Address>, size: usize) -> Result<&[u8], context::Error> {
        self.memory_map.read_bytes(address.as_ref(), size)
    }

    fn write_bytes(&mut self, address: impl AsRef<Address>, bytes: &[u8]) -> Result<(), context::Error> {
        self.memory_map.write_bytes(address.as_ref(), bytes)
    }

    fn read_mem(&self, address: impl AsRef<Address>, size: usize) -> Result<BitVec, context::Error> {
        self.memory_map.read_mem(address.as_ref(), size)
    }

    fn write_mem(&mut self, address: impl AsRef<Address>, data: &BitVec) -> Result<(), context::Error> {
        self.memory_map.write_mem(address.as_ref(), data)
    }
}

impl <'irb> RegisterContext<BitVec> for ConcreteContext<'irb> {

    fn read_reg(&self, name: &str) -> Result<BitVec, context::Error> {
        self.regs.read_reg(name.as_ref())
    }

    fn write_reg(&mut self, name: &str, data: &BitVec) -> Result<(), context::Error> {
        self.regs.write_reg(name.as_ref(), data)
    }
}





#[cfg(test)]
mod tests {
    use fugue_core::language::LanguageBuilder;
    use fugue_bytes::Endian;
    use crate::peripheral::traits::MappedPeripheralState;
    use crate::peripheral::generic::dummy::DummyPeripheral;
    use crate::tests::TEST_PROGRAM;
    use super::*;

    /// test basic functionality of context operations
    #[test]
    fn test_context_operations() {
        // test initialization
        let lang_builder = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");
        let lifter = lang.lifter();
        let mut irb = lifter.irb(1024);
        let mut context = ConcreteContext::new_with(&lang);

        assert_eq!(
            context.language().convention().name(),
            "default"
        );
        assert_eq!(
            context.language().translator().architecture(),
            &fugue_arch::ArchitectureDef::new("ARM", Endian::Little, 32usize, "Cortex")
        );

        // test map_mem()
        let mem_base = Address::from(0x0u64);
        let aligned_size = 0x2000usize;
        let unaligned_size = 0x500usize;

        context.map_mem(mem_base, aligned_size)
            .expect("map_mem() failed:");
        context.map_mem(mem_base + aligned_size as u64, unaligned_size)
            .expect_err("map_mem() should have failed with UnalignedSize");
        context.map_mem(mem_base + 0x1000u64, aligned_size)
            .expect_err("map_mem() should have failed with MapConflict");
        context.map_mem(mem_base + 0x500u64, aligned_size)
            .expect_err("map_mem() should have failed with UnalignedAddress");

        // test map mmio
        let peripheral_state = DummyPeripheral::new_with(Address::from(0x8000u64), 0x1000usize);
        context.map_mmio(peripheral_state.base(), Box::new(peripheral_state))
            .expect("map_mmio() failed:");

        // test read/write bytes
        context.write_bytes(mem_base, TEST_PROGRAM)
            .expect("write_bytes() failed to write program into memory");
        let bytes = context.read_bytes(Address::from(0x0u64), TEST_PROGRAM.len())
            .expect("read_bytes() failed to read program from memory");
        assert_eq!(bytes, TEST_PROGRAM, "read/write bytes mismatch");

        context.write_bytes(Address::from(0x5000u64), TEST_PROGRAM)
            .expect_err("write_bytes() should have failed with Unmapped");
        context.read_bytes(Address::from(0x5000u64), 0x1000usize)
            .expect_err("read_bytes() should have failed with Unmapped");

        // test read/write bitvectors
        let addr = Address::from(TEST_PROGRAM.len() as u64);
        let loop_insn = [0xfe, 0xe7];
        let loop_insn_bv = BitVec::from_le_bytes(&loop_insn);
        context.write_mem(&addr, &loop_insn_bv)
            .expect("write_mem() failed to write BitVec");
        let bv = context.read_mem(&addr, 2)
            .expect("read_mem() failed to read memory");
        assert_eq!(loop_insn_bv, bv, "read/write bitvec mismatch");

        context.write_mem(Address::from(0x5000u64), &loop_insn_bv)
            .expect_err("write_mem() should have failed with Unmapped");
        context.read_mem(Address::from(0x5000u64), 2)
            .expect_err("read_mem() should have failed with Unmapped");

        // test read/write registers
        let stop_address = TEST_PROGRAM.len() as u64;
        let r0_val = BitVec::from(5).unsigned_cast(32);
        let sp_val = BitVec::from(aligned_size).unsigned_cast(32);
        let lr_val = BitVec::from(stop_address).unsigned_cast(32);
        context.write_reg("r0", &r0_val)
            .expect("write_reg() failed to write r0");
        context.write_reg("sp", &sp_val)
            .expect("write_reg() failed to write sp");
        context.write_reg("lr", &lr_val)
            .expect("write_reg() failed to write lr");

        assert_eq!(
            r0_val, context.read_reg("r0").expect("read_reg() failed to read r0"),
            "read/write r0 value mismatch"
        );
        assert_eq!(
            sp_val, context.read_reg("sp").expect("read_reg() failed to read sp"),
            "read/write sp value mismatch"
        );
        assert_eq!(
            lr_val, context.read_reg("lr").expect("read_reg() failed to read lr"),
            "read/write lr value mismatch"
        );

        context.write_reg("rax", &r0_val)
            .expect_err("write_reg() should have failed with InvalidRegisterName");
        context.read_reg("rax")
            .expect_err("read_reg() should have failed with InvalidRegisterName");

        // test lift block
        let tb = context.lift_block(Address::from(0x6u64), &mut irb);

        assert_eq!(
            &tb.bytes, &TEST_PROGRAM[0x6..0x16],
            "failed to lift first translation block correctly",
        );

        // test fetch
        let pcode = context.fetch(Address::from(0x6u64))
            .expect("failed to fetch instruction at address 0x6");

        assert!(pcode.operations.len() > 0, "pcode: {:?}", pcode);

        context.fetch(addr.clone())
            .expect_err("fetch() should have failed with Fetch error");

    }

    #[test]
    fn test_vnd_context_trait_impl() {
        use fugue_bv::BitVec;
        #[allow(unused_imports)]
        use fugue_ir::{
            VarnodeData,
            space::{ AddressSpace, Space, SpaceKind },
        };

        // initialization
        let lang_builder = LanguageBuilder::new("../data/processors")
            .expect("language builder not instantiated");
        let lang = lang_builder.build("ARM:LE:32:Cortex", "default")
            .expect("language failed to build");
        let lifter = lang.lifter();
        #[allow(unused_variables)]
        let irb = lifter.irb(1024);
        let mut context = ConcreteContext::new_with(&lang);

        assert_eq!(
            context.language().convention().name(),
            "default"
        );
        assert_eq!(
            context.language().translator().architecture(),
            &fugue_arch::ArchitectureDef::new("ARM", Endian::Little, 32usize, "Cortex")
        );

        // map memory
        let mem_base = Address::from(0x0u64);
        let aligned_size = 0x2000usize;

        context.map_mem(mem_base, aligned_size)
            .expect("map_mem() failed:");
        
        // declare test varnodes
        let r0_vnd = context.lang.translator()
            .register_by_name("r0")
            .expect("register named r0 not found");
        let ng_vnd = context.lang.translator()
            .register_by_name("NG")
            .expect("register named NG not found");
        println!("{}", ng_vnd.display(context.lang.translator()));
        let mem0x1000_vnd = VarnodeData::new(
            context.lang.translator().manager().default_space_ref(),
            0x1000u64,
            4usize,
        );
        let const42_vnd = VarnodeData::new(
            context.lang.translator().manager().constant_space_ref(),
            42u64,
            4usize,
        );
        let unique0_vnd = VarnodeData::new(
            context.lang.translator().manager().unique_space_ref(),
            0x0u64,
            4usize,
        );

        // test write_vnd()
        let r0_val = BitVec::from_u64(0x12345678u64, 32);
        context.write_vnd(&r0_vnd, &r0_val)
            .expect("failed to write r0");

        let ng_val = BitVec::from_usize(1usize, ng_vnd.bits());
        context.write_vnd(&ng_vnd, &ng_val)
            .expect("failed to write NG");

        let mem0x1000_val = BitVec::from_u64(0xdeadbeef, 32);
        context.write_vnd(&mem0x1000_vnd, &mem0x1000_val)
            .expect("failed to write mem[0x1000]");

        let unique0_val = BitVec::from_u64(0x11223344, 32);
        context.write_vnd(&unique0_vnd, &unique0_val)
            .expect("failed to write to tmp context at 0");

        // write to constant will panic
        let const42_val = BitVec::from_u64(const42_vnd.offset(), const42_vnd.bits());

        // test read_vnd()
        let r0_rval = context.read_vnd(&r0_vnd)
            .expect("failed to read r0");
        let ng_rval = context.read_vnd(&ng_vnd)
            .expect("failed to read NG flag");
        let ng_rval2 = context.read_vnd(&ng_vnd)
            .expect("failed to read NG flag");
        let mem0x1000_rval = context.read_vnd(&mem0x1000_vnd)
            .expect("failed to read mem[0x1000]");
        let unique0_rval = context.read_vnd(&unique0_vnd)
            .expect("failed to read tmp at offset 0");
        let const42_rval = context.read_vnd(&const42_vnd)
            .expect("failed to read constant");

        assert_eq!(ng_val, ng_rval, "NG read/write value mismatch");
        assert_eq!(ng_rval, ng_rval2, "NG read value changes unexpectedly");
        assert_eq!(r0_val, r0_rval, "r0 read/write value mismatch");
        assert_eq!(mem0x1000_val, mem0x1000_rval, "mem read/write at 0x1000 value mismatch");
        assert_eq!(unique0_val, unique0_rval, "unique read/write at offset 0 value mismatch");
        assert_eq!(const42_val, const42_rval, "const 42 read mismatch actual");
    }
}