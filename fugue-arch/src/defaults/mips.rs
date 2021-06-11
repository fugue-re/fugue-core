use crate::{Architecture, ArchitectureDef};
use unicase::UniCase;

#[derive(Default)]
pub struct MIPS;

impl Architecture for MIPS {
    fn supported_for(&self, def: &ArchitectureDef) -> Option<Box<dyn Architecture>> {
        let processor = UniCase::new(def.processor());
        let variant = UniCase::new(def.variant());

        if def.bits() != 32 {
            return None
        }

        if processor == UniCase::new("mips") || variant == UniCase::new("mipsel") {
            return Some(Box::new(MIPS))
        }

        None
    }
}
