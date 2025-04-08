use fugue_lifter::{Language, Varnode};

use crate::arch::{Arch, Flag};
use crate::loader::symbols::FunctionThunkTemplate;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct X86 {
    language: &'static Language,
}

impl Arch for X86 {
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
        self.language.register_by_name("EBP")
    }

    fn gprs(&self) -> Vec<Varnode> {
        vec![
            self.language.register_by_name("EAX").unwrap(),
            self.language.register_by_name("EBX").unwrap(),
            self.language.register_by_name("ECX").unwrap(),
            self.language.register_by_name("EDX").unwrap(),
            self.language.register_by_name("ESI").unwrap(),
            self.language.register_by_name("EDI").unwrap(),
            self.language.register_by_name("EBP").unwrap(),
            self.language.register_by_name("ESP").unwrap(),
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

impl X86 {
    pub(crate) fn new(language: &'static Language) -> Self {
        Self { language }
    }
}
