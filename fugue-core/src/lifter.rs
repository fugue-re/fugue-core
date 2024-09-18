use std::cell::Cell;
use std::fmt;

use fugue_ir::disassembly::lift::ArenaVec;
use fugue_ir::disassembly::Opcode;
use fugue_ir::disassembly::{ContextDatabase, IRBuilderArena, PCodeData, PCodeRaw};
use fugue_ir::error::Error;
use fugue_ir::il::instruction::Instruction;
use fugue_ir::translator::TranslationContext;
use fugue_ir::{Address, Translator};

use smallvec::SmallVec;
use thiserror::Error;

use crate::ir::{Insn, Location, PCode, ToAddress};

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

        const LIFTED      = 0b1000_0000_0000_0000;

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

impl LiftedInsnProperties {
    pub(crate) fn from_targets(targets: &[(u16, LiftedInsnTarget)]) -> Self {
        let mut prop = Self::empty();

        for (_, target) in targets.iter() {
            match target {
                LiftedInsnTarget::IntraBlk(_, true) => prop |= Self::FALL,
                LiftedInsnTarget::IntraBlk(_, false)
                | LiftedInsnTarget::InterBlk(_)
                | LiftedInsnTarget::Unresolved => prop |= Self::BRANCH,
                LiftedInsnTarget::InterSub(_) => prop |= Self::CALL,
                LiftedInsnTarget::InterRet(_, _) => prop |= Self::RETURN,
                _ => (),
            }
        }

        prop
    }
}

pub struct LiftedInsn<'input, 'lifter> {
    pub address: Address,
    pub bytes: &'input [u8],
    pub properties: Cell<LiftedInsnProperties>,
    pub operations: Option<ArenaVec<'lifter, PCodeData<'lifter>>>,
    pub targets: SmallVec<[(u16, LiftedInsnTarget); 2]>,
    pub delay_slots: u8,
    pub length: u8,
}

impl<'input, 'lifter> LiftedInsn<'input, 'lifter> {
    pub fn new_lazy(address: Address, bytes: &'input [u8], length: u8) -> Self {
        Self {
            address,
            bytes,
            properties: Default::default(),
            operations: Default::default(),
            targets: Default::default(),
            delay_slots: 0,
            length,
        }
    }

    pub fn new_lifted(
        lifter: &mut Lifter,
        irb: &'lifter IRBuilderArena,
        address: Address,
        bytes: &'input [u8],
    ) -> Result<Self, LifterError> {
        let PCode {
            address,
            operations,
            delay_slots,
            length,
        } = lifter
            .lift(irb, address, bytes)
            .map_err(LifterError::lift)?;

        let naddress = address + length as u32;
        let targets = LiftedInsnTarget::from_lifted(address, naddress, &operations);
        let properties =
            LiftedInsnProperties::from_targets(&targets) | LiftedInsnProperties::LIFTED;

        Ok(Self {
            address,
            bytes,
            properties: Cell::new(properties),
            operations: Some(operations),
            targets,
            delay_slots,
            length,
        })
    }

    pub fn address(&self) -> Address {
        self.address
    }

    pub fn properties(&self) -> LiftedInsnProperties {
        self.properties.get()
    }

    pub fn is_flow(&self) -> bool {
        self.properties().intersects(LiftedInsnProperties::FLOW)
    }

    pub fn has_fall(&self) -> bool {
        self.properties().contains(LiftedInsnProperties::FALL)
    }

    pub fn is_lifted(&self) -> bool {
        self.properties().contains(LiftedInsnProperties::LIFTED)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len()]
    }

    pub fn len(&self) -> usize {
        self.length as _
    }

    fn ensure_lifted<'a>(
        &'a mut self,
        lifter: &mut Lifter,
        irb: &'lifter IRBuilderArena,
    ) -> Result<&'a [PCodeData<'lifter>], LifterError> {
        let operations = self.operations.insert(
            lifter
                .lift(irb, self.address, self.bytes)
                .map_err(LifterError::lift)?
                .operations,
        );

        LiftedInsnTarget::from_lifted_into(
            self.address,
            self.address + self.length as u32,
            &operations,
            &mut self.targets,
        );

        self.properties.replace(
            LiftedInsnProperties::from_targets(&self.targets) | LiftedInsnProperties::LIFTED,
        );

        Ok(operations)
    }

    pub fn pcode<'a>(
        &'a mut self,
        lifter: &mut Lifter,
        irb: &'lifter IRBuilderArena,
    ) -> Result<&'a [PCodeData<'lifter>], LifterError> {
        if let Some(ref operations) = self.operations {
            return Ok(operations);
        }

        self.ensure_lifted(lifter, irb)
    }

    pub fn try_pcode(&self) -> Option<&[PCodeData<'lifter>]> {
        self.operations.as_deref()
    }

    pub fn into_pcode(
        self,
        lifter: &mut Lifter,
        irb: &'lifter IRBuilderArena,
    ) -> Result<PCode<'lifter>, Error> {
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

    pub fn targets<'a>(
        &'a mut self,
        lifter: &mut Lifter,
        irb: &'lifter IRBuilderArena,
    ) -> Result<&'a [(u16, LiftedInsnTarget)], LifterError> {
        if !self.is_lifted() {
            self.ensure_lifted(lifter, irb)?;
        }

        Ok(&self.targets)
    }

    pub fn try_targets(&self) -> Option<&[(u16, LiftedInsnTarget)]> {
        if self.is_lifted() {
            Some(&self.targets)
        } else {
            None
        }
    }

    pub fn iter_targets<'a>(&'a self) -> impl Iterator<Item = (LiftedInsnTargetKind, Address)> + 'a {
        use LiftedInsnTarget::*;
        use LiftedInsnTargetKind::*;

        self.targets.iter().filter_map(|(_, target)| match *target {
            IntraBlk(taken, _) if taken.position() == 0 => Some((Local, taken.address())),
            InterBlk(taken) => Some((Local, taken)),
            InterSub(Some(taken)) | InterRet(Some(taken), _) => Some((Global, taken)),
            _ => None,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LiftedInsnTargetKind {
    Local,
    Global,
}

impl LiftedInsnTargetKind {
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }

    pub fn is_global(&self) -> bool {
        matches!(self, Self::Global)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LiftedInsnTarget {
    IntraIns(Location, bool),
    IntraBlk(Location, bool),
    InterBlk(Address),
    InterSub(Option<Address>),
    InterRet(Option<Address>, bool),
    Intrinsic,
    Unresolved,
}

impl LiftedInsnTarget {
    fn from_lifted(
        address: Address,
        naddress: Address,
        opns: &[PCodeData],
    ) -> SmallVec<[(u16, Self); 2]> {
        let mut targets = SmallVec::new();
        Self::from_lifted_into(address, naddress, opns, &mut targets);
        targets
    }

    fn from_lifted_into(
        address: Address,
        naddress: Address,
        opns: &[PCodeData],
        targets: &mut SmallVec<[(u16, Self); 2]>,
    ) {
        let op_count = opns.len() as u16;

        let is_local = |loc: &Location| -> bool { loc.address() == address };
        let is_fall = |loc: &Location| -> bool { loc.address() == naddress };

        let nlocation = |i: u16| -> Location {
            if i >= op_count {
                Location::new(naddress.clone(), i - op_count)
            } else {
                Location::new(address.clone(), i)
            }
        };

        let ncall = |i: u16, loc: Option<Location>, targets: &mut SmallVec<[(u16, Self); 2]>| {
            let Some(loc) = loc else {
                targets.push((i, Self::InterSub(None)));
                return;
            };

            if loc.position() != 0 {
                targets.push((i, Self::IntraIns(loc, false)));
            } else {
                targets.push((i, Self::InterSub(Some(loc.address()))));
            }
        };

        let nbranch = |i: u16, loc: Option<Location>, targets: &mut SmallVec<[(u16, Self); 2]>| {
            let Some(loc) = loc else {
                targets.push((i, Self::Unresolved));
                return;
            };

            if is_local(&loc) {
                targets.push((i, Self::IntraIns(loc, false)));
            } else if is_fall(&loc) {
                targets.push((i, Self::IntraBlk(loc, false)));
            } else {
                targets.push((i, Self::InterBlk(loc.address())));
            }
        };

        let nfall = |i: u16, fall: Location, targets: &mut SmallVec<[(u16, Self); 2]>| {
            targets.push((
                i,
                if is_local(&fall) {
                    Self::IntraIns(fall, true)
                } else {
                    Self::IntraBlk(fall, true)
                },
            ));
        };

        for (i, stmt) in opns.iter().enumerate() {
            let i = i as u16;
            let next = nlocation(i + 1);
            match stmt.opcode {
                Opcode::Branch => {
                    let locn = Location::absolute_from(address, stmt.inputs[0], i);
                    nbranch(i, locn, targets);
                }
                Opcode::CBranch => {
                    let locn = Location::absolute_from(address, stmt.inputs[0], i);
                    nbranch(i, locn, targets);
                    nfall(i, next, targets);
                }
                Opcode::IBranch => {
                    let locn = stmt.inputs[0].to_address().map(Location::from);
                    nbranch(i, locn, targets);
                }
                Opcode::Call => {
                    let locn = Location::absolute_from(address, stmt.inputs[0], i);
                    ncall(i, locn, targets);
                    nfall(i, next, targets);
                }
                Opcode::ICall => {
                    let locn = stmt.inputs[0].to_address().map(Location::from);
                    ncall(i, locn, targets);
                    nfall(i, next, targets);
                }
                Opcode::Return => {
                    let addr = stmt.inputs[0].to_address();
                    targets.push((i, Self::InterRet(addr, i + 1 == op_count)));
                }
                Opcode::CallOther => {
                    targets.push((i, Self::Intrinsic));
                    nfall(i, next, targets);
                }
                _ => {
                    if i + 1 == op_count {
                        nfall(i, next, targets);
                    }
                }
            }
        }
    }
}

impl fmt::Display for LiftedInsnTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IntraIns(loc, _) => write!(f, "intra-instruction flow to {loc}"),
            Self::IntraBlk(loc, _) => write!(f, "intra-block flow to {loc}"),
            Self::InterBlk(tgt) => write!(f, "inter-block flow to {tgt}"),
            Self::InterSub(None) => write!(f, "unresolved inter-sub-routine flow"),
            Self::InterSub(Some(tgt)) => write!(f, "inter-sub-routine flow to {tgt}"),
            Self::InterRet(None, _last) => {
                write!(f, "unresolved inter-sub-routine flow via return")
            }
            Self::InterRet(Some(tgt), _last) => {
                write!(f, "inter-sub-routine flow to {tgt} via return")
            }
            Self::Intrinsic => write!(f, "intrinsic flow"),
            Self::Unresolved => write!(f, "unresolved"),
        }
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
        LiftedInsn::new_lifted(lifter, irb, address, bytes)
    }
}
