use fugue_lifter::{Language, Varnode};

use crate::arch::{Arch, Flag};
use crate::loader::symbols::FunctionThunkTemplate;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct X86_64 {
    language: &'static Language,
}

impl Arch for X86_64 {
    fn external_thunk_template(&self) -> FunctionThunkTemplate {
        FunctionThunkTemplate::new([0xc3])
    }

    fn flags(&self) -> Vec<Flag> {
        vec![
            Flag::a(self.language.register_by_name("AF").unwrap()),
            Flag::c(self.language.register_by_name("CF").unwrap()),
            Flag::new(self.language.register_by_name("DF").unwrap()),
            Flag::v(self.language.register_by_name("OF").unwrap()),
            Flag::p(self.language.register_by_name("PF").unwrap()),
            Flag::n(self.language.register_by_name("SF").unwrap()),
            Flag::z(self.language.register_by_name("ZF").unwrap()),
        ]
    }

    fn frame_pointer(&self) -> Option<Varnode> {
        self.language.register_by_name("RBP")
    }

    fn gprs(&self) -> Vec<Varnode> {
        vec![
            self.language.register_by_name("RAX").unwrap(),
            self.language.register_by_name("RBX").unwrap(),
            self.language.register_by_name("RCX").unwrap(),
            self.language.register_by_name("RDX").unwrap(),
            self.language.register_by_name("RSI").unwrap(),
            self.language.register_by_name("RDI").unwrap(),
            self.language.register_by_name("RBP").unwrap(),
            self.language.register_by_name("RSP").unwrap(),
            self.language.register_by_name("R8").unwrap(),
            self.language.register_by_name("R9").unwrap(),
            self.language.register_by_name("R10").unwrap(),
            self.language.register_by_name("R11").unwrap(),
            self.language.register_by_name("R12").unwrap(),
            self.language.register_by_name("R13").unwrap(),
            self.language.register_by_name("R14").unwrap(),
            self.language.register_by_name("R15").unwrap(),
        ]
    }

    fn is_halt_intrinsic(&self, op: u16, _args: &[Varnode]) -> bool {
        op == self.language.user_op_by_name("halt").unwrap()
    }

    fn is_nonsense_pattern(&self, bytes: &[u8]) -> bool {
        const NONSENSE: &[&[u8]] = &[&[0x00u8, 0x00u8], &[0x00u8], &[0xf0u8]];
        NONSENSE.contains(&bytes)
    }

    fn is_skip_intrinsic(&self, op: u16, _args: &[Varnode]) -> bool {
        op == self.language.user_op_by_name("int3").unwrap()
            || op == self.language.user_op_by_name("ud2").unwrap()
    }

    fn is_trap_intrinsic(&self, op: u16, args: &[Varnode]) -> bool {
        op == self.language.user_op_by_name("int3").unwrap()
            || (op == self.language.user_op_by_name("swi").unwrap()
                && args.first().copied() == Some(Varnode::constant(0x3, 8)))
            || op
                == self
                    .language
                    .user_op_by_name("invalidInstructionException")
                    .unwrap()
    }

    fn language(&self) -> &'static Language {
        self.language
    }
}

impl X86_64 {
    pub(crate) fn new(language: &'static Language) -> Self {
        Self { language }
    }
}
