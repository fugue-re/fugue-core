use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt::{Debug, Display};
use std::hash::{Hash, Hasher};

use bitflags::bitflags;
use clone_dyn::clone_dyn;
use fugue_lifter::{Language, Varnode};

use crate::loader::symbols::FunctionThunkTemplate;
use crate::types::Endian;

pub mod aarch64;
pub mod arm;
pub mod x86;
pub mod x86_64;

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FlagKind: u8 {
        const Z = 0b0000_0001;
        const C = 0b0000_0010;
        const N = 0b0000_0100;
        const V = 0b0000_1000;
        const P = 0b0001_0000;
        const A = 0b0010_0000;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct Flag {
    var: Varnode,
    kind: FlagKind,
}

impl Borrow<Varnode> for Flag {
    fn borrow(&self) -> &Varnode {
        &self.var
    }
}

impl PartialEq<&'_ Varnode> for Flag {
    fn eq(&self, other: &&'_ Varnode) -> bool {
        self.var == **other
    }
}

impl PartialEq<Varnode> for Flag {
    fn eq(&self, other: &Varnode) -> bool {
        self.var == *other
    }
}

impl Flag {
    pub const fn new(var: Varnode) -> Self {
        Self::new_with(var, FlagKind::empty())
    }

    pub const fn new_with(var: Varnode, kind: FlagKind) -> Self {
        Self { var, kind }
    }

    pub const fn z(var: Varnode) -> Self {
        Self::new_with(var, FlagKind::Z)
    }

    pub const fn c(var: Varnode) -> Self {
        Self::new_with(var, FlagKind::C)
    }

    pub const fn n(var: Varnode) -> Self {
        Self::new_with(var, FlagKind::N)
    }

    pub const fn v(var: Varnode) -> Self {
        Self::new_with(var, FlagKind::V)
    }

    pub const fn p(var: Varnode) -> Self {
        Self::new_with(var, FlagKind::P)
    }

    pub const fn a(var: Varnode) -> Self {
        Self::new_with(var, FlagKind::A)
    }

    pub fn kind(&self) -> FlagKind {
        self.kind
    }

    pub fn variable(&self) -> Varnode {
        self.var
    }
}

#[clone_dyn]
pub trait ArchImpl: Send + Sync + 'static {
    fn endian(&self) -> Endian {
        if self.language().is_little_endian() {
            Endian::Little
        } else {
            Endian::Big
        }
    }

    fn external_thunk_template(&self) -> FunctionThunkTemplate;

    fn flags(&self) -> &[Flag] {
        &[]
    }

    fn frame_pointer(&self) -> Option<Varnode> {
        None
    }

    fn gprs(&self) -> &[Varnode] {
        &[]
    }

    #[allow(unused)]
    fn is_halt_intrinsic(&self, op: u16, args: &[Varnode]) -> bool {
        false
    }

    #[allow(unused)]
    fn is_mapping_symbol(&self, symbol: &str) -> bool {
        false
    }

    #[allow(unused)]
    fn is_nonsense_pattern(&self, bytes: &[u8]) -> bool {
        false
    }

    #[allow(unused)]
    fn is_service_call(&self, op: u16, args: &[Varnode]) -> bool {
        false
    }

    #[allow(unused)]
    fn is_skip_intrinsic(&self, op: u16, args: &[Varnode]) -> bool {
        false
    }

    #[allow(unused)]
    fn is_trap_intrinsic(&self, op: u16, args: &[Varnode]) -> bool {
        false
    }

    fn language(&self) -> &'static Language;
}

#[derive(Clone)]
#[repr(transparent)]
pub struct Arch(Box<dyn ArchImpl>);

impl Debug for Arch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Arch")
            .field("language", self.0.language())
            .finish_non_exhaustive()
    }
}

impl Display for Arch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.language().id())
    }
}

impl PartialEq for Arch {
    fn eq(&self, other: &Self) -> bool {
        self.0.language().id() == other.0.language().id()
    }
}

impl Eq for Arch {}

impl PartialOrd for Arch {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Arch {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.language().id().cmp(other.0.language().id())
    }
}

impl Hash for Arch {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.language().id().hash(state);
    }
}

impl From<Box<dyn ArchImpl>> for Arch {
    fn from(arch: Box<dyn ArchImpl>) -> Self {
        Self(arch)
    }
}

impl Arch {
    pub fn new(language: &'static Language) -> Self {
        match language.processor() {
            "ARM" => arm::Arm::new(language),
            "AARCH64" => aarch64::AArch64::new(language),
            "x86" => {
                if language.address_bits() == 32 {
                    x86::X86::new(language)
                } else {
                    x86_64::X86_64::new(language)
                }
            }
            _ => {
                // NOTE: should be unreachable
                panic!("unsupported language: {}", language.id())
            }
        }
    }

    pub fn endian(&self) -> Endian {
        self.0.endian()
    }

    pub fn external_thunk_template(&self) -> FunctionThunkTemplate {
        self.0.external_thunk_template()
    }

    pub fn flags(&self) -> &[Flag] {
        self.0.flags()
    }

    pub fn frame_pointer(&self) -> Option<Varnode> {
        self.0.frame_pointer()
    }

    pub fn gprs(&self) -> &[Varnode] {
        self.0.gprs()
    }

    pub fn is_halt_intrinsic(&self, op: u16, args: &[Varnode]) -> bool {
        self.0.is_halt_intrinsic(op, args)
    }

    pub fn is_mapping_symbol(&self, symbol: &str) -> bool {
        self.0.is_mapping_symbol(symbol)
    }

    pub fn is_nonsense_pattern(&self, bytes: &[u8]) -> bool {
        self.0.is_nonsense_pattern(bytes)
    }

    pub fn is_service_call(&self, op: u16, args: &[Varnode]) -> bool {
        self.0.is_service_call(op, args)
    }

    pub fn is_skip_intrinsic(&self, op: u16, args: &[Varnode]) -> bool {
        self.0.is_skip_intrinsic(op, args)
    }

    pub fn is_trap_intrinsic(&self, op: u16, args: &[Varnode]) -> bool {
        self.0.is_trap_intrinsic(op, args)
    }

    pub fn language(&self) -> &'static Language {
        self.0.language()
    }
}
