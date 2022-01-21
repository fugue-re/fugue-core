use fugue_bytes::endian::Endian;

use std::fmt;
use std::str::FromStr;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct ArchitectureDef {
    processor: String,
    endian: Endian,
    bits: usize,
    variant: String,
}

#[derive(Debug, Error)]
pub enum ArchDefParseError {
    #[error("could not parse processor name")]
    ParseProcessor,
    #[error("could not parse endian")]
    ParseEndian,
    #[error("could not parse bitness")]
    ParseBits,
    #[error("could not parse processor variant")]
    ParseVariant,
    #[error("could not parse architecture definition: incorrect format")]
    ParseFormat,
}

impl FromStr for ArchitectureDef {
    type Err = ArchDefParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.splitn(4, ':').collect::<Vec<_>>();
        if parts.len() != 4 {
            return Err(ArchDefParseError::ParseFormat)
        }

        let processor = parts[0];
        let endian = match parts[1] {
            "le" | "LE" => Endian::Little,
            "be" | "BE" => Endian::Big,
            _ => return Err(ArchDefParseError::ParseEndian),
        };
        let bits = parts[2].parse::<usize>()
            .map_err(|_| ArchDefParseError::ParseBits)?;
        let variant = parts[3];

        Ok(ArchitectureDef::new(processor, endian, bits, variant))
    }
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
