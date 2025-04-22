use fugue_lifter::arm::le::context::T_MODE;
use fugue_lifter::arm::le::register::{
    LR, PC, R0, R1, R10, R11, R12, R2, R3, R4, R5, R6, R7, R8, R9, SP,
};
use fugue_lifter::{Language, Varnode};

use crate::arch::{Arch, ArchImpl};
use crate::lifter::ContextUpdates;
use crate::loader::symbols::FunctionThunkTemplate;

const GPRS: &[Varnode] = &[
    R0, R1, R2, R3, R4, R5, R6, R7, R8, R9, R10, R11, R12, SP, LR, PC,
];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Arm {
    language: &'static Language,
}

impl ArchImpl for Arm {
    fn external_thunk_template(&self) -> FunctionThunkTemplate {
        if self.language.variant().ends_with("T") {
            let mut bytes = [0x70, 0x47];
            if self.language.is_big_endian() {
                bytes.reverse();
            }
            FunctionThunkTemplate::new_with(bytes, ContextUpdates::single(T_MODE, 1))
        } else {
            let mut bytes = [0x1e, 0xff, 0x2f, 0xe1];
            if self.language.is_big_endian() {
                bytes.reverse();
            }
            FunctionThunkTemplate::new_with(bytes, ContextUpdates::single(T_MODE, 0))
        }
    }

    fn gprs(&self) -> &[Varnode] {
        GPRS
    }

    fn language(&self) -> &'static Language {
        self.language
    }
}

impl Arm {
    pub(crate) fn new(language: &'static Language) -> Arch {
        Arch::from(Box::new(Self { language }) as Box<dyn ArchImpl>)
    }
}
