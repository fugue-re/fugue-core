use std::str::FromStr;

use fugue_bytes::Endian;
use thiserror::Error;

use crate::runtime::Lifter;

pub struct LifterBuilder {
    processor: String,
    bits: Option<u32>,
    is_big: bool,
    variant: Option<String>,
}

#[derive(Debug, Error)]
pub enum LifterBuilderError {
    #[error("invalid language format")]
    ParseFormat,
    #[error("invalid language bits; must be: 8, 16, 32, or 64")]
    ParseBits,
    #[error("invalid endian; must be BE or LE")]
    ParseEndian,
    #[error("unsupported architecture")]
    Unsupported,
}

impl FromStr for LifterBuilder {
    type Err = LifterBuilderError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(':');

        let Some(processor) = parts.next().map(str::trim) else {
            return Err(LifterBuilderError::ParseFormat);
        };

        if processor.is_empty() {
            return Err(LifterBuilderError::ParseFormat);
        }

        let Some(endian) = parts.next().map(str::trim) else {
            return Err(LifterBuilderError::ParseFormat);
        };

        let is_big = match endian {
            "le" | "LE" => false,
            "be" | "BE" => true,
            _ => {
                return Err(LifterBuilderError::ParseEndian);
            }
        };

        let Some(bits) = parts.next().map(str::trim) else {
            return Err(LifterBuilderError::ParseFormat);
        };

        let bits = match bits.parse::<u32>() {
            Ok(bits) if [8, 16, 32, 64].contains(&bits) => bits,
            _ => {
                return Err(LifterBuilderError::ParseBits);
            }
        };

        let variant = parts
            .next()
            .map(str::trim)
            .and_then(|v| (!v.is_empty()).then_some(v))
            .map(ToOwned::to_owned);

        Ok(Self {
            processor: processor.to_owned(),
            is_big,
            bits: Some(bits),
            variant,
        })
    }
}

impl LifterBuilder {
    pub fn new(processor: impl Into<String>) -> Self {
        Self {
            processor: processor.into(),
            bits: None,
            is_big: false,
            variant: None,
        }
    }

    pub fn set_bits(&mut self, bits: u32) {
        self.bits = Some(bits);
    }

    pub fn bits(mut self, bits: u32) -> Self {
        self.set_bits(bits);
        self
    }

    pub fn big_endian(mut self) -> Self {
        self.is_big = true;
        self
    }

    pub fn little_endian(mut self) -> Self {
        self.is_big = false;
        self
    }

    pub fn set_endian(&mut self, endian: Endian) {
        self.is_big = endian.is_big();
    }

    pub fn endian(mut self, endian: Endian) -> Self {
        self.set_endian(endian);
        self
    }

    pub fn set_variant(&mut self, variant: impl Into<String>) {
        self.variant = Some(variant.into());
    }

    pub fn variant(mut self, variant: impl Into<String>) -> Self {
        self.set_variant(variant);
        self
    }

    pub fn build_str(language: impl AsRef<str>) -> Result<Lifter, LifterBuilderError> {
        language.as_ref().parse::<Self>()?.build()
    }

    pub fn build(&self) -> Result<Lifter, LifterBuilderError> {
        match (
            self.processor.as_ref(),
            self.is_big,
            self.bits,
            self.variant.as_deref(),
        ) {
            #[cfg(feature = "x86")]
            ("x86", false, Some(32), None | Some("default")) => {
                Ok(crate::x86::LifterFactory::new_default())
            }
            #[cfg(feature = "x86-64")]
            ("x86", false, Some(64), None | Some("default")) => {
                Ok(crate::x86_64::LifterFactory::new_default())
            }
            #[cfg(feature = "x86-64")]
            ("x86", false, Some(64), Some("compat32")) => {
                Ok(crate::x86_64::LifterFactory::new_compat32())
            }
            #[cfg(feature = "arm-be")]
            ("ARM", true, None | Some(32), variant) => match variant {
                None | Some("v8") => Ok(crate::arm::be::LifterFactory::new_v8()),
                Some("v8T") => Ok(crate::arm::be::LifterFactory::new_v8t()),
                _ => Err(LifterBuilderError::Unsupported),
            },
            #[cfg(feature = "arm-le")]
            ("ARM", false, None | Some(32), variant) => match variant {
                None | Some("v8") => Ok(crate::arm::le::LifterFactory::new_v8()),
                Some("v8T") => Ok(crate::arm::le::LifterFactory::new_v8t()),
                _ => Err(LifterBuilderError::Unsupported),
            },
            #[cfg(feature = "aarch64-be")]
            ("AARCH64", true, None | Some(64), None | Some("v8A")) => {
                Ok(crate::aarch64::be::LifterFactory::new_v8a())
            }
            #[cfg(feature = "aarch64-le")]
            ("AARCH64", false, None | Some(64), None | Some("v8A")) => {
                Ok(crate::aarch64::le::LifterFactory::new_v8a())
            }
            _ => Err(LifterBuilderError::Unsupported),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_builder_defaults() -> Result<(), LifterBuilderError> {
        let _ = LifterBuilder::new("x86").bits(32).build()?;
        let _ = LifterBuilder::new("x86").bits(64).build()?;

        let _ = LifterBuilder::new("ARM").little_endian().build()?;
        let _ = LifterBuilder::new("ARM")
            .bits(32)
            .big_endian()
            .variant("v8T")
            .build()?;

        let _ = LifterBuilder::new("AARCH64").little_endian();
        let _ = LifterBuilder::new("AARCH64").big_endian().bits(64);
        let _ = LifterBuilder::new("AARCH64")
            .little_endian()
            .bits(64)
            .variant("v8A");

        Ok(())
    }

    #[test]
    fn test_from_str_defaults() -> Result<(), LifterBuilderError> {
        let _ = "x86:LE:32".parse::<LifterBuilder>()?.build()?;
        let _ = "x86:LE:32:default".parse::<LifterBuilder>()?.build()?;

        assert!("x86:LE".parse::<LifterBuilder>().is_err());
        assert!("x86:BE:32:default"
            .parse::<LifterBuilder>()?
            .build()
            .is_err());

        let _ = "x86:LE:64:default".parse::<LifterBuilder>()?.build()?;

        let _ = "ARM:BE:32".parse::<LifterBuilder>()?.build()?;
        let _ = "ARM:BE:32:v8".parse::<LifterBuilder>()?.build()?;
        let _ = "ARM:BE:32:v8T".parse::<LifterBuilder>()?.build()?;

        let _ = "ARM:LE:32".parse::<LifterBuilder>()?.build()?;
        let _ = "ARM:LE:32:v8".parse::<LifterBuilder>()?.build()?;
        let _ = "ARM:LE:32:v8T".parse::<LifterBuilder>()?.build()?;

        let _ = "AARCH64:BE:64".parse::<LifterBuilder>()?.build()?;
        let _ = "AARCH64:BE:64:v8A".parse::<LifterBuilder>()?.build()?;

        let _ = "AARCH64:LE:64".parse::<LifterBuilder>()?.build()?;
        let _ = "AARCH64:LE:64:v8A".parse::<LifterBuilder>()?.build()?;

        Ok(())
    }
}
