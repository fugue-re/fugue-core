use std::cell::Cell;
use std::fmt;

use arrayvec::ArrayVec;

pub use fugue_lifter::{
    ContextBitRange, Language, Lifter, LifterBuilder, LifterBuilderError, LiftingContext, Op,
    PCodeOp,
};
use smallvec::SmallVec;
use thiserror::Error;

use crate::types::{Address, Location, ToAddress};

#[derive(Debug, Error)]
pub enum LifterError {
    #[error("invalid instruction at {0}")]
    InvalidInstruction(Address),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContextUpdate {
    bits: ContextBitRange,
    value: u32,
}

impl ContextUpdate {
    pub fn new(bits: ContextBitRange, value: u32) -> Self {
        Self { bits, value }
    }

    pub fn bits(&self) -> &ContextBitRange {
        &self.bits
    }

    pub fn value(&self) -> u32 {
        self.value
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContextUpdates(ArrayVec<ContextUpdate, 2>);

impl From<ContextUpdate> for ContextUpdates {
    fn from(value: ContextUpdate) -> Self {
        Self(ArrayVec::from_iter([value]))
    }
}

impl ContextUpdates {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn single(bits: ContextBitRange, value: u32) -> Self {
        ContextUpdate::new(bits, value).into()
    }

    #[inline]
    pub fn push(&mut self, value: ContextUpdate) {
        self.0.push(value);
    }

    #[inline]
    pub fn apply(&self, address: Address, context: &mut LiftingContext) {
        for ContextUpdate { bits, value } in self.0.iter() {
            tracing::trace!("setting context bits {bits:?} to {value} at {address}");
            context.set_variable_by_bits(bits, address.into(), *value);
        }
    }

    #[inline]
    pub fn apply_range(&self, from: Address, to: Option<Address>, context: &mut LiftingContext) {
        for ContextUpdate { bits, value } in self.0.iter() {
            tracing::trace!("setting context bits {bits:?} to {value} from {from} to {to:?}");
            context.set_variable_region_by_bits(bits, from.into(), to.map(Address::into), *value);
        }
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

pub struct LiftedInsn {
    pub address: Address,
    pub properties: Cell<LiftedInsnProperties>,
    pub operations: Vec<PCodeOp>,
    pub targets: SmallVec<[(u16, LiftedInsnTarget); 2]>,
    pub length: u8,
}

pub trait LifterExt {
    fn lift_insn(&mut self, address: Address, bytes: &[u8]) -> Result<LiftedInsn, LifterError>;
}

impl LifterExt for Lifter {
    fn lift_insn(&mut self, address: Address, bytes: &[u8]) -> Result<LiftedInsn, LifterError> {
        let mut operations = Vec::new();
        let Some(length) = self.lift(address.into(), bytes, &mut operations) else {
            return Err(LifterError::InvalidInstruction(address));
        };

        let next_address = address + length;

        let targets =
            LiftedInsnTarget::from_lifted(self.language(), address, next_address, &operations);

        let properties = LiftedInsnProperties::from_targets(&targets);

        Ok(LiftedInsn {
            address,
            properties: Cell::new(properties),
            operations,
            targets,
            length: length as _,
        })
    }
}

impl LiftedInsn {
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

    pub fn len(&self) -> usize {
        self.length as _
    }

    pub fn iter_targets<'a>(
        &'a self,
    ) -> impl Iterator<Item = (LiftedInsnTargetKind, Address)> + 'a {
        use LiftedInsnTarget::*;
        use LiftedInsnTargetKind::*;

        self.targets.iter().filter_map(|(_, target)| match *target {
            IntraBlk(taken, _) if taken.position() == 0 => Some((Local, taken.address())),
            InterBlk(taken) => Some((Local, taken)),
            InterSub(Some(taken)) | InterRet(Some(taken), _) => Some((Global, taken)),
            _ => None,
        })
    }

    pub fn display(&self, language: &'static Language) -> LiftedInsnFormatter {
        LiftedInsnFormatter::new(self, language)
    }
}

pub struct LiftedInsnFormatter<'a> {
    lifted: &'a LiftedInsn,
    language: &'static Language,
}

impl<'a> LiftedInsnFormatter<'a> {
    pub fn new(lifted: &'a LiftedInsn, language: &'static Language) -> Self {
        Self { lifted, language }
    }
}

impl fmt::Display for LiftedInsnFormatter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lifted = self.lifted;
        let language = self.language;

        write!(f, "{}", language.display(&lifted.operations))?;

        Ok(())
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
        language: &'static Language,
        address: Address,
        naddress: Address,
        opns: &[PCodeOp],
    ) -> SmallVec<[(u16, Self); 2]> {
        let mut targets = SmallVec::new();
        Self::from_lifted_into(language, address, naddress, opns, &mut targets);
        targets
    }

    fn from_lifted_into(
        language: &'static Language,
        address: Address,
        naddress: Address,
        opns: &[PCodeOp],
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
            let inputs = stmt.inputs();
            match stmt.op() {
                Op::Branch => {
                    let locn = Location::absolute_from(language, address, inputs[0], i);
                    nbranch(i, locn, targets);
                }
                Op::CBranch => {
                    let locn = Location::absolute_from(language, address, inputs[0], i);
                    nbranch(i, locn, targets);
                    nfall(i, next, targets);
                }
                Op::IBranch => {
                    let locn = inputs[0].to_address(language).map(Location::from);
                    nbranch(i, locn, targets);
                }
                Op::Call => {
                    let locn = Location::absolute_from(language, address, inputs[0], i);
                    ncall(i, locn, targets);
                    nfall(i, next, targets);
                }
                Op::ICall => {
                    let locn = inputs[0].to_address(language).map(Location::from);
                    ncall(i, locn, targets);
                    nfall(i, next, targets);
                }
                Op::Return => {
                    let addr = inputs[0].to_address(language);
                    targets.push((i, Self::InterRet(addr, i + 1 == op_count)));
                }
                Op::UserOp(_, _) => {
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
