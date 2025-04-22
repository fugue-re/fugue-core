use fugue_lifter::x86::register::{
    AF, CF, DF, EAX, EBP, EBX, ECX, EDI, EDX, ESI, ESP, OF, PF, SF, ZF,
};
use fugue_lifter::x86::user_op::{INVALID_INSTRUCTION_EXCEPTION, SWI};
use fugue_lifter::{Language, Varnode};

use crate::arch::{Arch, ArchImpl, Flag};
use crate::loader::symbols::FunctionThunkTemplate;

const FLAGS: &[Flag] = &[
    Flag::a(AF),
    Flag::c(CF),
    Flag::new(DF),
    Flag::v(OF),
    Flag::p(PF),
    Flag::n(SF),
    Flag::z(ZF),
];
const GPRS: &[Varnode] = &[EAX, EBX, ECX, EDX, ESI, EDI, EBP, ESP];
const NONSENSE: &[&[u8]] = &[&[0x00u8, 0x00u8], &[0x00u8], &[0xf0u8]];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct X86 {
    language: &'static Language,
}

impl ArchImpl for X86 {
    fn external_thunk_template(&self) -> FunctionThunkTemplate {
        FunctionThunkTemplate::new([0xc3])
    }

    fn flags(&self) -> &[Flag] {
        FLAGS
    }

    fn frame_pointer(&self) -> Option<Varnode> {
        Some(EBP)
    }

    fn gprs(&self) -> &[Varnode] {
        GPRS
    }

    fn is_nonsense_pattern(&self, bytes: &[u8]) -> bool {
        NONSENSE.contains(&bytes)
    }

    fn is_skip_intrinsic(&self, op: u16, args: &[Varnode]) -> bool {
        op == SWI && args.first().copied() == Some(Varnode::constant(0x3, 8)) // int3
            || op == INVALID_INSTRUCTION_EXCEPTION // ud2
    }

    fn is_trap_intrinsic(&self, op: u16, args: &[Varnode]) -> bool {
        op == SWI && args.first().copied() == Some(Varnode::constant(0x3, 8)) // int3
            || op == INVALID_INSTRUCTION_EXCEPTION // ud2
    }

    fn language(&self) -> &'static Language {
        self.language
    }
}

impl X86 {
    pub(crate) fn new(language: &'static Language) -> Arch {
        Arch::from(Box::new(Self { language }) as Box<dyn ArchImpl>)
    }
}
