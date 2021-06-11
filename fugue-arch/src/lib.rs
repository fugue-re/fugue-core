use fugue_bytes::endian::Endian;

pub mod defaults;

use std::fmt;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("unsupported architecture: {0}")]
    UnsupportedDefinition(ArchitectureDef),
}

pub trait Architecture: Send + Sync {
    // can be used to catch variants that aren't detected by exact
    // definition matches
    fn supported_for(&self, _def: &ArchitectureDef) -> Option<Box<dyn Architecture>> {
        None
    }
}

#[repr(transparent)]
pub struct ArchitectureRegistry(Vec<Box<dyn Architecture>>);

impl ArchitectureRegistry {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn register<A>(&mut self, architecture: A)
    where A: Architecture + 'static {
        self.0.push(Box::new(architecture))
    }

    pub fn register_defaults(&mut self) {
        self.register(defaults::arm::ARM::default());
        self.register(defaults::arm::AArch64::default());

        self.register(defaults::mips::MIPS::default());

        self.register(defaults::x86::X86::default());
        self.register(defaults::x86::X86_64::default());
    }

    pub fn match_definition(&self, def: &ArchitectureDef) -> Option<Box<dyn Architecture>> {
        self.0
            .iter()
            .rev()
            .find_map(|v| v.supported_for(def))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArchitectureDef {
    processor: String,
    endian: Endian,
    bits: usize,
    variant: String,
}

impl fmt::Display for ArchitectureDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "processor: {}, endian: {}, bits: {}, variant: {}",
            self.processor,
            if self.endian.is_big() { "big" } else { "little" },
            self.bits,
            self.variant,
        )
    }
}

impl ArchitectureDef {
    pub fn new<P, V>(processor: P, endian: Endian, bits: usize, variant: V) -> Self
    where P: Into<String>,
          V: Into<String> {
        Self {
            processor: processor.into(),
            endian,
            bits,
            variant: variant.into(),
        }
    }

    pub fn is_little(&self) -> bool {
        self.endian.is_little()
    }

    pub fn is_big(&self) -> bool {
        self.endian.is_big()
    }

    pub fn endian(&self) -> Endian {
        self.endian
    }

    pub fn bits(&self) -> usize {
        self.bits
    }

    pub fn processor(&self) -> &str {
        &self.processor
    }

    pub fn variant(&self) -> &str {
        &self.variant
    }
}
