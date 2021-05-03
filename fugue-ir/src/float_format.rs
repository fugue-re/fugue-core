use crate::bits;
use crate::deserialise::error::Error;
use crate::deserialise::parse::XmlExt;

use std::num::FpCategory;
use std::mem::size_of;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloatFormat {
    size: usize,
    sign_pos: u32,
    frac_pos: u32,
    frac_size: u32,
    exp_pos: u32,
    exp_max: i32,
    exp_size: u32,
    bias: i32,
    j_bit_implied: bool,
}

impl FloatFormat {
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

    pub fn from_parts(sign: bool, signif: u64, exp: i32) -> f64 {
        let signif = signif.checked_shr(1).unwrap_or(0);
        let precis = 8 * size_of::<u64>() as i32 - 1;
        let expchg = exp - precis + 1;

        let res = ldexp(signif as f64, expchg);
        if sign {
            res * -1.0f64
        } else {
            res
        }
    }

    pub fn into_parts(x: f64) -> (FpCategory, bool, u64, i32) {
        let kind = x.classify();
        let (mut norm, e) = frexp(x);
        norm = ldexp(norm, 8 * size_of::<u64>() as i32 - 1);

        let sign = x.is_sign_negative();
        let signif = (norm as u64).checked_shl(1).unwrap_or(0);
        let exp = e.wrapping_sub(1);

        (kind, sign, signif, exp)
    }

    pub fn fractional_code(&self, x: u64) -> u64 {
        let y = x.checked_shr(self.frac_pos).unwrap_or(0);
        let z = y.checked_shl(8 * size_of::<u64>() as u32 - self.frac_size).unwrap_or(0);
        z
    }

    pub fn with_fractional_code(&self, x: u64, code: u64) -> u64 {
        let y = code.checked_shr(8 * size_of::<u64>() as u32 - self.frac_size)
            .unwrap_or(0);
        let z = y.checked_shl(self.frac_pos).unwrap_or(0);
        x | z
    }

    pub fn sign(&self, x: u64) -> bool {
        let y = x.checked_shr(self.sign_pos).unwrap_or(0);
        (y & 1) != 0
    }

    pub fn with_sign(&self, x: u64, sign: bool) -> u64 {
        if !sign {
            x
        } else {
            x | 1u64.checked_shl(self.sign_pos).unwrap_or(0)
        }
    }

    pub fn exponent_code(&self, x: u64) -> i32 {
        let y = x.checked_shr(self.exp_pos).unwrap_or(0);
        let mask = 1u64.checked_shl(self.exp_size)
            .unwrap_or(0)
            .wrapping_sub(1);

        (y & mask) as i32
    }

    pub fn with_exponent_code(&self, x: u64, code: u64) -> u64 {
        x | code.checked_shl(self.exp_pos).unwrap_or(0)
    }

    pub fn zero_encoding(&self, sign: bool) -> u64 {
        let mut res = 0;
        res = self.with_fractional_code(res, 0);
        res = self.with_exponent_code(res, 0);
        self.with_sign(res, sign)
    }

    pub fn infinity_encoding(&self, sign: bool) -> u64 {
        let mut res = 0;
        res = self.with_fractional_code(res, 0);
        res = self.with_exponent_code(res, self.exp_max as u64);
        self.with_sign(res, sign)
    }

    pub fn nan_encoding(&self, sign: bool) -> u64 {
        let mut res = 0;
        let mask = 1u64.checked_shl(8 * size_of::<u64>() as u32 - 1)
            .unwrap_or(0);
        res = self.with_fractional_code(res, mask);
        res = self.with_exponent_code(res, self.exp_max as u64);
        self.with_sign(res, sign)
    }

    pub fn host_float(&self, encoding: u64) -> (FpCategory, f64) {
        let sign = self.sign(encoding);
        let frac = self.fractional_code(encoding);
        let exp = self.exponent_code(encoding);
        let mut normal = true;

        let kind = if exp == 0 {
            if frac == 0 {
                return (FpCategory::Zero, if sign { -0.0f64 } else { 0.0f64 })
            }
            normal = false;
            FpCategory::Subnormal
        } else if exp == self.exp_max {
            if frac == 0 {
                return (FpCategory::Infinite, if sign { f64::NEG_INFINITY } else { f64::INFINITY })
            } else {
                return (FpCategory::Nan, if sign { -f64::NAN } else { f64::NAN })
            }
        } else {
            FpCategory::Normal
        };

        let exp = exp.wrapping_sub(self.bias);

        if normal && self.j_bit_implied {
            let frac = frac.checked_shr(1).unwrap_or(0);
            let highbit = 1u64.checked_shl(8 * size_of::<u64>() as u32 - 1)
                .unwrap_or(0);
            let frac = frac | highbit;
            (kind, Self::from_parts(sign, frac, exp))
        } else {
            (kind, Self::from_parts(sign, frac, exp))
        }
    }

    pub fn encoding(&self, host: f64) -> u64 {
        let (kind, sign, signif, exp) = Self::into_parts(host);
        match kind {
            FpCategory::Zero => return self.zero_encoding(sign),
            FpCategory::Infinite => return self.infinity_encoding(sign),
            FpCategory::Nan => return self.nan_encoding(sign),
            _ => {
                let exp = exp.wrapping_add(self.bias);
                if exp < 0 {
                    return self.zero_encoding(sign)
                } else if exp > self.exp_max {
                    return self.infinity_encoding(sign)
                }

                let signif = if self.j_bit_implied && exp != 0 {
                    signif.checked_shl(1).unwrap_or(0)
                } else {
                    signif
                };

                let mut res = 0;
                res = self.with_fractional_code(res, signif);
                res = self.with_exponent_code(res, exp as u64);
                self.with_sign(res, sign)
            }
        }
    }

    pub fn convert_encoding(&self, other: &FloatFormat, encoding: u64) -> u64 {
        let sign = self.sign(encoding);
        let frac = self.fractional_code(encoding);
        let exp = self.exponent_code(encoding);

        let exp = if exp == other.exp_max {
            self.exp_max
        } else {
            let exp = exp.wrapping_sub(other.bias);
            let exp = exp.wrapping_add(self.bias);
            if exp < 0 {
                return self.zero_encoding(sign)
            } else if exp > self.exp_max {
                return self.infinity_encoding(sign)
            }
            exp
        };

        let frac = if self.j_bit_implied && !other.j_bit_implied {
            frac.checked_shl(1).unwrap_or(0)
        } else if other.j_bit_implied && !self.j_bit_implied {
            let frac = frac.checked_shl(1).unwrap_or(0);
            let highbit = 1u64.checked_shl(8 * size_of::<u64>() as u32 - 1)
                .unwrap_or(0);
            frac | highbit
        } else {
            frac
        };

        let mut res = 0;
        res = self.with_fractional_code(res, frac);
        res = self.with_exponent_code(res, exp as u64);
        self.with_sign(res, sign)
    }

    pub fn op_equal(&self, a: u64, b: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        let (_, b1) = self.host_float(b);
        if a1 == b1 { 1 } else { 0 }
    }

    pub fn op_not_equal(&self, a: u64, b: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        let (_, b1) = self.host_float(b);
        if a1 != b1 { 1 } else { 0 }
    }

    pub fn op_less(&self, a: u64, b: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        let (_, b1) = self.host_float(b);
        if a1 < b1 { 1 } else { 0 }
    }

    pub fn op_less_equal(&self, a: u64, b: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        let (_, b1) = self.host_float(b);
        if a1 <= b1 { 1 } else { 0 }
    }

    pub fn op_add(&self, a: u64, b: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        let (_, b1) = self.host_float(b);
        self.encoding(a1 + b1)
    }

    pub fn op_sub(&self, a: u64, b: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        let (_, b1) = self.host_float(b);
        self.encoding(a1 - b1)
    }

    pub fn op_mult(&self, a: u64, b: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        let (_, b1) = self.host_float(b);
        self.encoding(a1 * b1)
    }

    pub fn op_div(&self, a: u64, b: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        let (_, b1) = self.host_float(b);
        self.encoding(a1 * b1)
    }

    pub fn op_is_nan(&self, a: u64) -> u64 {
        let (k, _) = self.host_float(a);
        if k == FpCategory::Nan { 1 } else { 0 }
    }

    pub fn op_neg(&self, a: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        self.encoding(-a1)
    }

    pub fn op_abs(&self, a: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        self.encoding(a1.abs())
    }

    pub fn op_sqrt(&self, a: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        self.encoding(a1.sqrt())
    }

    pub fn op_float_of_int(&self, a: u64, size: usize) -> u64 {
        self.encoding(bits::sign_extend(a as i64, size) as f64)
    }

    pub fn op_float_of_float(&self, other: &FloatFormat, a: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        other.encoding(a1)
    }

    pub fn op_truncate(&self, a: u64, size: usize) -> u64 {
        let (_, a1) = self.host_float(a);
        let res = a1 as i64 as u64;
        res & bits::calculate_mask(size)
    }

    pub fn op_ceiling(&self, a: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        self.encoding(a1.ceil())
    }

    pub fn op_floor(&self, a: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        self.encoding(a1.floor())
    }

    pub fn op_round(&self, a: u64) -> u64 {
        let (_, a1) = self.host_float(a);
        self.encoding(a1.round())
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, Error> {
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

fn frexp(x: f64) -> (f64, i32) {
    extern "C" {
        fn frexp(x: f64, exp: *mut i32) -> f64;
    }
    let mut exp = 0;
    let r = unsafe {
        frexp(x, &mut exp as *mut i32)
    };
    (r, exp)
}

fn ldexp(x: f64, exp: i32) -> f64 {
    extern "C" {
        fn ldexp(x: f64, exp: i32) -> f64;
    }
    unsafe {
        ldexp(x, exp)
    }
}
