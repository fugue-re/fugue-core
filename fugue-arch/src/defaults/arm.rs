use crate::{Architecture, ArchitectureDef};
use unicase::UniCase;

#[derive(Default)]
pub struct ARM {
    is_thumb: bool
}

impl ARM {
    #[inline(always)]
    pub fn is_thumb(&self) -> bool {
        self.is_thumb
    }
}

#[derive(Default)]
pub struct AArch64;
pub type ARM64 = AArch64;

impl Architecture for ARM {
    fn supported_for(&self, def: &ArchitectureDef) -> Option<Box<dyn Architecture>> {
        let processor = UniCase::new(def.processor());
        let variant = UniCase::new(def.variant());

        if processor == UniCase::new("thumb") || variant == UniCase::new("thumb") {
            return Some(Box::new(ARM { is_thumb: true }))
        }

        if processor == UniCase::new("arm") {
            return Some(Box::new(ARM { is_thumb: false }))
        }

        None
    }
}

impl Architecture for AArch64 {
    fn supported_for(&self, def: &ArchitectureDef) -> Option<Box<dyn Architecture>> {
        let processor = UniCase::new(def.processor());
        let variant = UniCase::new(def.variant());

        if def.bits() != 64 {
            return None
        }

        if processor == UniCase::new("aarch64") || variant == UniCase::new("arm64") {
            return Some(Box::new(AArch64))
        }

        None
    }
}
