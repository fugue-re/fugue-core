use fugue_lifter::{Language, Varnode};

use crate::arch::Arch;
use crate::lifter::ContextUpdates;
use crate::loader::symbols::FunctionThunkTemplate;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Arm {
    language: &'static Language,
}

impl Arch for Arm {
    fn external_thunk_template(&self) -> FunctionThunkTemplate {
        let tmode = self.language.context_variable_by_name("TMode").unwrap();

        if self.language.variant().ends_with("T") {
            let mut bytes = [0x70, 0x47];
            if self.language.is_big_endian() {
                bytes.reverse();
            }
            FunctionThunkTemplate::new_with(bytes, ContextUpdates::single(tmode, 1))
        } else {
            let mut bytes = [0x1e, 0xff, 0x2f, 0xe1];
            if self.language.is_big_endian() {
                bytes.reverse();
            }
            FunctionThunkTemplate::new_with(bytes, ContextUpdates::single(tmode, 0))
        }
    }

    fn gprs(&self) -> Vec<Varnode> {
        vec![
            self.language.register_by_name("r0").unwrap(),
            self.language.register_by_name("r1").unwrap(),
            self.language.register_by_name("r2").unwrap(),
            self.language.register_by_name("r3").unwrap(),
            self.language.register_by_name("r4").unwrap(),
            self.language.register_by_name("r5").unwrap(),
            self.language.register_by_name("r6").unwrap(),
            self.language.register_by_name("r7").unwrap(),
            self.language.register_by_name("r8").unwrap(),
            self.language.register_by_name("r9").unwrap(),
            self.language.register_by_name("r10").unwrap(),
            self.language.register_by_name("r11").unwrap(),
            self.language.register_by_name("r12").unwrap(),
            self.language.register_by_name("sp").unwrap(),
            self.language.register_by_name("lr").unwrap(),
            self.language.register_by_name("pc").unwrap(),
        ]
    }

    fn language(&self) -> &'static Language {
        self.language
    }
}

impl Arm {
    pub(crate) fn new(language: &'static Language) -> Self {
        Self { language }
    }
}
