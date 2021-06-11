use crate::{Architecture, ArchitectureDef};
use unicase::UniCase;

#[derive(Default)]
pub struct X86;

#[derive(Default)]
pub struct X86_64;

impl Architecture for X86 {
    fn supported_for(&self, def: &ArchitectureDef) -> Option<Box<dyn Architecture>> {
        let processor = UniCase::new(def.processor());
        let variant = UniCase::new(def.variant());

        if def.bits() != 32 {
            return None
        }

        if processor == UniCase::new("metapc") || variant == UniCase::new("x86") {
            return Some(Box::new(X86_64))
        }

        None
    }
}

impl Architecture for X86_64 {
    fn supported_for(&self, def: &ArchitectureDef) -> Option<Box<dyn Architecture>> {
        let processor = UniCase::new(def.processor());
        let variant = UniCase::new(def.variant());

        if def.bits() != 64 {
            return None
        }

        if processor == UniCase::new("metapc") || variant == UniCase::new("x86-64") {
            return Some(Box::new(X86_64))
        }

        None
    }
}
