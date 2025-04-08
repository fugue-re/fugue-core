use fugue_lifter::{Language, Varnode};

use crate::arch::Arch;
use crate::loader::symbols::FunctionThunkTemplate;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AArch64 {
    language: &'static Language,
}

impl Arch for AArch64 {
    fn external_thunk_template(&self) -> FunctionThunkTemplate {
        FunctionThunkTemplate::new([0xc0, 0x03, 0x5f, 0xd6]) // RET
    }

    fn is_halt_intrinsic(&self, op: u16, _args: &[Varnode]) -> bool {
        op == self.language.user_op_by_name("halt").unwrap()
    }

    fn is_nonsense_pattern(&self, bytes: &[u8]) -> bool {
        bytes == &[0x00u8, 0x00u8, 0x00u8, 0x00u8]
    }

    fn gprs(&self) -> Vec<Varnode> {
        vec![
            self.language.register_by_name("x0").unwrap(),
            self.language.register_by_name("x1").unwrap(),
            self.language.register_by_name("x2").unwrap(),
            self.language.register_by_name("x3").unwrap(),
            self.language.register_by_name("x4").unwrap(),
            self.language.register_by_name("x5").unwrap(),
            self.language.register_by_name("x6").unwrap(),
            self.language.register_by_name("x7").unwrap(),
            self.language.register_by_name("x8").unwrap(),
            self.language.register_by_name("x9").unwrap(),
            self.language.register_by_name("x10").unwrap(),
            self.language.register_by_name("x11").unwrap(),
            self.language.register_by_name("x12").unwrap(),
            self.language.register_by_name("x13").unwrap(),
            self.language.register_by_name("x14").unwrap(),
            self.language.register_by_name("x15").unwrap(),
            self.language.register_by_name("x16").unwrap(),
            self.language.register_by_name("x17").unwrap(),
            self.language.register_by_name("x18").unwrap(),
            self.language.register_by_name("x19").unwrap(),
            self.language.register_by_name("x20").unwrap(),
            self.language.register_by_name("x21").unwrap(),
            self.language.register_by_name("x22").unwrap(),
            self.language.register_by_name("x23").unwrap(),
            self.language.register_by_name("x24").unwrap(),
            self.language.register_by_name("x25").unwrap(),
            self.language.register_by_name("x26").unwrap(),
            self.language.register_by_name("x27").unwrap(),
            self.language.register_by_name("x28").unwrap(),
            self.language.register_by_name("x29").unwrap(),
            self.language.register_by_name("x30").unwrap(),
        ]
    }

    fn language(&self) -> &'static Language {
        self.language
    }
}

impl AArch64 {
    pub(crate) fn new(language: &'static Language) -> Self {
        Self { language }
    }
}
