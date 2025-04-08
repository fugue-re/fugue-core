use std::borrow::Borrow;

use bitflags::bitflags;
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

pub trait Arch: Send + Sync + 'static {
    fn endian(&self) -> Endian {
        if self.language().is_little_endian() {
            Endian::Little
        } else {
            Endian::Big
        }
    }

    fn external_thunk_template(&self) -> FunctionThunkTemplate;

    #[allow(unused)]
    fn flags(&self) -> Vec<Flag> {
        Vec::with_capacity(0)
    }

    #[allow(unused)]
    fn frame_pointer(&self) -> Option<Varnode> {
        None
    }

    #[allow(unused)]
    fn gprs(&self) -> Vec<Varnode> {
        Vec::with_capacity(0)
    }

    #[allow(unused)]
    fn is_halt_intrinsic(&self, op: u16, _args: &[Varnode]) -> bool {
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
