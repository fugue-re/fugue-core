use std::sync::Arc;

use ahash::AHashMap as Map;

use crate::deserialise::{DeserialiseError, XmlExt};

pub type FloatFormats = Map<usize, Arc<FloatFormat>>;

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub struct FloatFormat {
    pub size: usize,
    pub sign_pos: u32,
    pub frac_pos: u32,
    pub frac_size: u32,
    pub exp_pos: u32,
    pub exp_max: i32,
    pub exp_size: u32,
    pub bias: i32,
    pub j_bit_implied: bool,
}

impl FloatFormat {
    pub const fn float2() -> Self {
        FloatFormat {
            size: 2,
            sign_pos: 15,
            frac_pos: 0,
            frac_size: 10,
            exp_pos: 10,
            exp_size: 5,
            exp_max: (1 << 5) - 1,
            bias: 15,
            j_bit_implied: true,
        }
    }

    pub const fn float4() -> Self {
        FloatFormat {
            size: 4,
            sign_pos: 31,
            frac_pos: 0,
            frac_size: 23,
            exp_pos: 23,
            exp_size: 8,
            exp_max: (1 << 8) - 1,
            bias: 127,
            j_bit_implied: true,
        }
    }

    pub const fn float8() -> Self {
        FloatFormat {
            size: 8,
            sign_pos: 63,
            frac_pos: 0,
            frac_size: 52,
            exp_pos: 52,
            exp_size: 11,
            exp_max: (1 << 11) - 1,
            bias: 1023,
            j_bit_implied: true,
        }
    }

    pub const fn float10() -> Self {
        FloatFormat {
            size: 10,
            sign_pos: 79,
            exp_pos: 64,
            exp_size: 15,
            exp_max: (1 << 15) - 1,
            frac_pos: 0,
            frac_size: 64,
            bias: 16383,
            j_bit_implied: true,
        }
    }

    pub const fn float16() -> Self {
        FloatFormat {
            size: 16,
            sign_pos: 127,
            exp_pos: 112,
            exp_size: 15,
            exp_max: (1 << 15) - 1,
            frac_pos: 0,
            frac_size: 112,
            bias: 16383,
            j_bit_implied: true,
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn bits(&self) -> usize {
        self.size * 8
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        let size = input.attribute_int("size")?;
        let sign_pos = input.attribute_int("signpos")?;
        let frac_pos = input.attribute_int("fracpos")?;
        let frac_size = input.attribute_int("fracsize")?;
        let exp_pos = input.attribute_int("exppos")?;
        let exp_size = input.attribute_int("expsize")?;
        let exp_max = (1i32 << exp_size) - 1;

        let bias = input.attribute_int("bias")?;
        let j_bit_implied = input.attribute_bool("jbitimpled")?;

        Ok(FloatFormat {
            size,
            sign_pos,
            frac_pos,
            frac_size,
            exp_pos,
            exp_size,
            exp_max,
            bias,
            j_bit_implied,
        })
    }
}
