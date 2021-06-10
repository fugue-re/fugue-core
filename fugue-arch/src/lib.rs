use fugue_bytes::endian::Endian;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Architecture {
    name: String,
    endian: Endian,
    bits: usize,
    variant: String,
}

impl Architecture {
    pub fn new<N, V>(name: N, endian: Endian, bits: usize, variant: V) -> Self
    where N: Into<String>,
          V: Into<String> {
        Self {
            name: name.into(),
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

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn variant(&self) -> &str {
        &self.variant
    }
}
