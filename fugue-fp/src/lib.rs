pub use fugue_bv::BitVec;
pub use fugue_ir::float_format::FloatFormat;

use std::cmp::Ordering;
use std::ops::AddAssign;
use std::ops::DivAssign;
use std::ops::MulAssign;
use std::ops::SubAssign;
use std::mem::take;

use rug::Assign;
use rug::Integer as BigInt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sign {
    Positive,
    Negative,
}

#[derive(Debug, Clone, Hash)]
pub struct Float {
    frac_bits: u32,
    exp_bits: u32,
    kind: FloatKind,
    sign: i32,
    unscaled: BigInt,
    scale: i32,
    max_scale: i32,
    min_scale: i32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatKind {
    Finite,
    Infinite,
    QuietNaN,
    SignallingNaN,
}

impl Float {
    fn from_parts(
        frac_bits: u32,
        exp_bits: u32,
        kind: FloatKind,
        sign: Sign,
        unscaled: BigInt,
        scale: i32) -> Self {

        let max_scale = (1i32 << (exp_bits - 1u32)) - 1i32;
        let min_scale = 1 - max_scale;

        Self {
            frac_bits,
            exp_bits,
            kind,
            sign: if matches!(sign, Sign::Positive) { 1 } else { -1 },
            unscaled,
            scale,
            max_scale,
            min_scale,
        }
    }

    pub fn from_bigint(frac_bits: u32, exp_bits: u32, value: BigInt) -> Self {
        let mut f = Self::from_parts(
            frac_bits,
            exp_bits,
            FloatKind::Finite,
            if value >= 0 { Sign::Positive } else { Sign::Negative },
            value.abs(),
            frac_bits as i32,
        );

        f.scale_up_to(frac_bits + 1);
        f
    }

    pub fn zero_with(frac_bits: u32, exp_bits: u32, sign: Sign) -> Self {
        Self::from_parts(
            frac_bits,
            exp_bits,
            FloatKind::Finite,
            sign,
            BigInt::from(0),
            2u32.wrapping_sub(1u32 << (exp_bits - 1)) as i32,
        )
    }

    pub fn zero(frac_bits: u32, exp_bits: u32) -> Self {
        Self::zero_with(frac_bits, exp_bits, Sign::Positive)
    }

    pub fn infinity(frac_bits: u32, exp_bits: u32, sign: Sign) -> Self {
        Self::from_parts(
            frac_bits,
            exp_bits,
            FloatKind::Infinite,
            sign,
            BigInt::from(1) << frac_bits,
            (1u32 << (exp_bits - 1)).wrapping_sub(1) as i32,
        )
    }

    pub fn quiet_nan(frac_bits: u32, exp_bits: u32, sign: Sign) -> Self {
        Self::from_parts(
            frac_bits,
            exp_bits,
            FloatKind::QuietNaN,
            sign,
            BigInt::from(0),
            (1u32 << (exp_bits - 1)).wrapping_sub(1) as i32,
        )
    }

    pub fn is_normal(&self) -> bool {
        matches!(self.kind, FloatKind::Finite) &&
            self.unscaled.significant_bits() >= self.frac_bits + 1
    }

    pub fn is_infinite(&self) -> bool {
        matches!(self.kind, FloatKind::Infinite)
    }

    pub fn is_zero(&self) -> bool {
        matches!(self.kind, FloatKind::Finite) && self.unscaled == 0
    }

    pub fn is_nan(&self) -> bool {
        matches!(self.kind, FloatKind::QuietNaN | FloatKind::SignallingNaN)
    }

    fn make_zero(&mut self) {
        self.kind = FloatKind::Finite;
        self.unscaled = BigInt::from(0);
        self.scale = self.min_scale;
    }

    fn make_one(&mut self) {
        self.kind = FloatKind::Finite;
        self.scale = 0;
        self.unscaled = BigInt::from(1) << self.frac_bits;
    }

    fn make_quiet_nan(&mut self) {
        self.kind = FloatKind::QuietNaN;
    }

    fn internal_round(&mut self, eps: bool) {
        if !matches!(self.kind, FloatKind::Finite) {
            panic!("rounding non-finite float")
        }

        if self.unscaled == 0 {
            if eps {
                panic!("rounding zero + epsilon, need a bit length")
            }
            self.make_zero();
            return
        }

        let extra_bits =
            (self.unscaled.significant_bits().wrapping_sub(self.frac_bits - 1) as i32).max(
                self.min_scale.wrapping_sub(self.scale));

        if extra_bits <= 0 {
            panic!("round with no extra bits of precision")
        }

        let mid_bit = (extra_bits - 1) as u32;
        let mid_bit_set = self.unscaled.get_bit(mid_bit);
        let eps = eps || self.unscaled.find_one(0).map(|pos| pos < mid_bit).unwrap_or(true);

        self.unscaled <<= extra_bits as u32;
        self.scale = self.scale.wrapping_add(extra_bits);

        let is_odd = self.unscaled.get_bit(0);

        if mid_bit_set && (eps || is_odd) {
            self.unscaled += 1;
            if self.unscaled.significant_bits() > self.frac_bits + 1 {
                assert_eq!(self.unscaled.significant_bits(),
                           self.unscaled.find_one(0).map(|pos| pos + 1).unwrap_or(0));
                self.unscaled >>= 1;
                self.scale = self.scale.wrapping_add(1);
            }
        }

        if self.scale > self.max_scale {
            self.kind = FloatKind::Infinite;
        }
    }

    fn leading_bit(&self) -> u32 {
        if !matches!(self.kind, FloatKind::Finite) || self.unscaled == 0 {
            panic!("leading bit of non-finite or zero")
        }

        self.scale.wrapping_add(
            self.unscaled.significant_bits()
                .wrapping_sub(self.frac_bits) as i32) as u32
    }

    fn upscale(&mut self, bits: u32) {
        if (bits as i32) < 0 {
            self.unscaled >>= (bits as i32).abs() as u32;
        } else {
            self.unscaled <<= bits;
        }
        self.scale = self.scale - (bits as i32);
    }

    fn scale_up_to(&mut self, bits: u32) {
        if !matches!(self.kind, FloatKind::Finite) {
            panic!("scaling of non-finite float")
        }

        let repr_bits = self.unscaled.significant_bits();
        if bits > repr_bits {
            self.upscale(bits.wrapping_sub(repr_bits));
        }
    }
}

impl DivAssign<Self> for Float {
    fn div_assign(&mut self, rhs: Self) {
        if self.is_nan() || rhs.is_nan() {
            self.make_quiet_nan();
            return
        }

        if self.is_infinite() {
            if rhs.is_infinite() {
                self.make_quiet_nan();
            } else {
                self.sign *= rhs.sign;
            }
            return
        }

        match rhs.kind {
            FloatKind::QuietNaN | FloatKind::SignallingNaN => {
                self.make_quiet_nan();
                return
            },
            FloatKind::Infinite => {
                self.make_zero();
                self.sign *= rhs.sign;
                return
            },
            FloatKind::Finite => {
                if rhs.is_zero() {
                    if self.is_zero() {
                        self.make_quiet_nan();
                    } else {
                        self.kind = FloatKind::Infinite;
                        self.sign *= rhs.sign;
                    }
                    return
                }

                let lshift = self.frac_bits
                    .wrapping_add(2)
                    .wrapping_add(rhs.unscaled.significant_bits())
                    .wrapping_sub(self.unscaled.significant_bits());

                self.upscale(lshift);

                // q in self; r in rhs
                let mut r = rhs.unscaled;
                self.unscaled.div_rem_mut(&mut r);
                self.sign *= rhs.sign;
                self.scale = self.scale
                    .wrapping_sub(rhs.scale)
                    .wrapping_sub(self.frac_bits as i32);
                self.internal_round(r != 0);
            },
        }
    }
}

impl MulAssign<Self> for Float {
    fn mul_assign(&mut self, rhs: Self) {
        if self.is_nan() || rhs.is_nan() {
            self.make_quiet_nan();
            return
        }

        if (self.is_zero() && rhs.is_infinite()) || (self.is_infinite() && rhs.is_zero()) {
            self.make_quiet_nan();
            return
        }

        if self.is_infinite() || rhs.is_infinite() {
            self.kind = FloatKind::Infinite;
            self.sign *= rhs.sign;
            return
        }

        self.sign *= rhs.sign;
        self.unscaled *= rhs.unscaled;
        self.scale = self.scale
            .wrapping_add(rhs.scale)
            .wrapping_sub(self.frac_bits as i32);
        self.scale_up_to(self.frac_bits + 2);
        self.internal_round(false);
    }
}

impl Float {
    fn add0_assign(&mut self, rhs: Self) {
        let rhs = rhs;
        let d = self.scale.wrapping_sub(rhs.scale);
        if d as u32 > self.frac_bits + 1 {
            return
        } else if d < -(self.frac_bits as i32 + 1) {
            *self = rhs;
            return
        }

        let (d, mut a, b) = if d >= 0 {
            let a = Float {
                frac_bits: self.frac_bits,
                exp_bits: self.exp_bits,
                kind: self.kind,
                sign: self.sign,
                unscaled: take(&mut self.unscaled),
                scale: self.scale,
                min_scale: self.min_scale,
                max_scale: self.max_scale,
            };
            let b = rhs;

            (d, a, b)
        } else {
            let a = rhs;
            let b = Float {
                frac_bits: self.frac_bits,
                exp_bits: self.exp_bits,
                kind: self.kind,
                sign: self.sign,
                unscaled: take(&mut self.unscaled),
                scale: self.scale,
                min_scale: self.min_scale,
                max_scale: self.max_scale,
            };

            (-d, a, b)
        };

        let residue = b.unscaled.find_one(0).map(|pos| (pos as i32) < (d - 1))
            .unwrap_or(true);
        self.scale = a.scale.wrapping_sub(1);

        a.unscaled <<= 1;
        a.unscaled += b.unscaled >> (d - 1) as u32;

        self.unscaled = a.unscaled;

        self.scale_up_to(self.frac_bits + 2);
        self.internal_round(residue);
    }

    fn sub0_assign(&mut self, rhs: Self) {
        let d = self.scale.wrapping_sub(rhs.scale);
        if d as u32 > self.frac_bits + 2 {
            return
        } else if d < -(self.frac_bits as i32 + 2) {
            *self = rhs;
            return
        }

        let (d, mut a, mut b) = if d >= 0 {
            let a = Float {
                frac_bits: self.frac_bits,
                exp_bits: self.exp_bits,
                kind: self.kind,
                sign: self.sign,
                unscaled: take(&mut self.unscaled),
                scale: self.scale,
                min_scale: self.min_scale,
                max_scale: self.max_scale,
            };
            let b = rhs;

            (d, a, b)
        } else {
            let a = rhs;
            let b = Float {
                frac_bits: self.frac_bits,
                exp_bits: self.exp_bits,
                kind: self.kind,
                sign: self.sign,
                unscaled: take(&mut self.unscaled),
                scale: self.scale,
                min_scale: self.min_scale,
                max_scale: self.max_scale,
            };

            (-d, a, b)
        };

        let residue = b.unscaled.find_one(0).map(|pos| (pos as i32) < (d - 2))
            .unwrap_or(true);
        self.sign = a.sign;
        self.scale = a.scale.wrapping_sub(2);

        b.unscaled >>= d - 2;
        if residue {
            b.unscaled += 1;
        }

        a.unscaled <<= 2;
        a.unscaled -= b.unscaled;

        if a.unscaled == 0 {
            self.sign = 1;
            self.unscaled = a.unscaled;
        } else if a.unscaled < 0 {
            self.sign *= -1;
            self.unscaled = -a.unscaled;
        } else {
            self.unscaled = a.unscaled;
        }

        self.scale_up_to(self.frac_bits + 2);
        self.internal_round(residue);
    }
}

impl AddAssign<Self> for Float {
    fn add_assign(&mut self, rhs: Self) {
        if self.is_nan() || rhs.is_nan() {
            self.make_quiet_nan();
            return
        }

        if self.is_infinite() && rhs.is_infinite() {
            if self.sign != rhs.sign {
                self.make_quiet_nan();
            }
            return
        }

        if self.is_infinite() {
            return
        }

        if rhs.is_infinite() {
            *self = rhs;
            return
        }

        if rhs.is_zero() {
            if self.is_zero() {
                self.sign = if self.sign < 0 && rhs.sign < 0 { -1 } else { 1 };
            }
            return
        }

        if self.is_zero() {
            *self = rhs;
            return
        }

        if self.sign == rhs.sign {
            self.add0_assign(rhs);
        } else {
            self.sub0_assign(rhs);
        }
    }
}

impl SubAssign<Self> for Float {
    fn sub_assign(&mut self, rhs: Self) {
        let mut rhs = rhs;
        let sign = self.sign;

        rhs.sign *= -1;
        let rsign = rhs.sign;

        self.add_assign(rhs);

        if self.is_zero() {
            self.sign = if sign < 0 && rsign < 0 { -1 } else { 1 };
        }
    }
}

impl Float {
    pub fn sqrt_assign(&mut self) {
        if self.is_zero() {
            return
        }

        if self.is_nan() || self.sign == -1 {
            self.make_quiet_nan();
            return
        }

        if self.is_infinite() {
            return
        }

        let sig_bits = self.frac_bits
            .wrapping_mul(2)
            .wrapping_add(3);
        self.scale_up_to(sig_bits);

        if self.scale.wrapping_add(self.frac_bits as i32) & 1 != 0 {
            self.upscale(1);
        }

        let mut residue = take(&mut self.unscaled);
        let mut result = BigInt::new();

        let mut pow = residue.significant_bits();
        pow = pow.wrapping_sub(pow & 1);

        let mut bit = BigInt::from(1) << pow;
        let mut resp = BigInt::new();
        while bit != 0 {
            resp.assign(&result + &bit);
            if residue >= resp {
                residue -= take(&mut resp);
                let res = BigInt::from(&bit << 1);
                result += res;
            }
            result >>= 1;
            bit >>= 2;
        }

        self.unscaled = result;
        self.scale = self.scale.wrapping_add(self.frac_bits as i32) / 2;
        self.internal_round(residue != 0);
    }

    fn floor0_assign(&mut self) {
        if self.scale < 0 {
            self.make_zero();
            return
        }
        let nbits = self.frac_bits.wrapping_sub(self.scale as u32);
        let temp = take(&mut self.unscaled);
        self.unscaled.assign((temp >> nbits) << nbits);
    }

    fn ceil0_assign(&mut self) {
        if self.is_zero() {
            return
        } else if self.scale < 0 {
            self.make_one();
            return
        }

        let nbits = self.frac_bits.wrapping_sub(self.scale as u32);
        let increment = self.unscaled.find_one(0).map(|pos| pos < nbits).unwrap_or(true);
        let temp = take(&mut self.unscaled);
        self.unscaled.assign((temp >> nbits) << nbits);

        if increment {
            self.unscaled += BigInt::from(1) << nbits;
        }

        if self.unscaled.significant_bits() > self.frac_bits + 1 {
            self.upscale(-1i32 as u32);
        }
    }

    pub fn floor_assign(&mut self) {
        match self.kind {
            FloatKind::Finite | FloatKind::QuietNaN => return,
            FloatKind::SignallingNaN => { // should we return here?
                self.make_quiet_nan();
                return
            },
            _ => (),
        }

        if self.sign >= 0 {
            self.floor0_assign();
        } else {
            self.ceil0_assign();
        }
    }

    pub fn ceil_assign(&mut self) {
        match self.kind {
            FloatKind::Finite | FloatKind::QuietNaN => return,
            FloatKind::SignallingNaN => { // should we return here?
                self.make_quiet_nan();
                return
            },
            _ => (),
        }

        if self.sign >= 0 {
            self.ceil0_assign();
        } else {
            self.floor0_assign();
        }
    }

    pub fn trunc_assign(&mut self) {
        self.floor0_assign();
    }

    pub fn neg_assign(&mut self) {
        self.sign *= -1;
    }

    pub fn abs_assign(&mut self) {
        self.sign = 1;
    }

    pub fn round_assign(&mut self) {
        let half = Self::from_parts(
            self.frac_bits,
            self.exp_bits,
            FloatKind::Finite,
            Sign::Positive,
            BigInt::from(1) << self.frac_bits,
            -1
        );
        self.add_assign(half);
        self.floor_assign();
    }

    pub fn to_bigint(&self) -> BigInt {
        let res = BigInt::from(&self.unscaled >> self.frac_bits.wrapping_sub(self.scale as u32));
        if self.sign < 0 {
            -res
        } else {
            res
        }
    }
}

impl PartialEq<Self> for Float {
    fn eq(&self, other: &Self) -> bool {
        if self.is_nan() {
            return other.is_nan()
        }

        if other.is_nan() {
            return false
        }

        if self.is_infinite() {
            if self.sign < 0 {
                return other.is_infinite() && other.sign < 0
            }

            return other.is_infinite() && other.sign > 0
        }

        if other.is_infinite() {
            return false
        }

        if self.sign != other.sign {
            return self.sign == 0
        }

        if self.scale != other.scale {
            return self.sign == 0
        }

        self.sign == 0 || self.unscaled == other.unscaled
    }
}
impl Eq for Float { }

impl Ord for Float {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.is_nan() {
            return if other.is_nan() {
                Ordering::Equal
            } else {
                Ordering::Greater
            }
        }

        if other.is_nan() {
            return Ordering::Less
        }

        if self.is_infinite() {
            if self.sign < 0 {
                return if other.is_infinite() && other.sign < 0 {
                    Ordering::Equal
                } else {
                    Ordering::Less
                }
            }

            return if other.is_infinite() && other.sign > 0 {
                Ordering::Equal
            } else {
                Ordering::Greater
            }
        }

        if other.is_infinite() {
            return other.sign.cmp(&0).reverse()
        }

        if self.sign != other.sign {
            return self.sign.cmp(&0)
        }

        if self.scale != other.scale {
            let sign = if self.scale < other.scale { -self.sign } else { self.sign };
            return sign.cmp(&0)
        }

        if self.sign == 0 {
            Ordering::Equal
        } else {
            let res = self.unscaled.cmp(&other.unscaled);
            if self.sign < 0 { res.reverse() } else { res }
        }
    }
}

impl PartialOrd<Self> for Float {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

trait FloatFormatOpsInternal {
    fn extract_sign(&self, val: &BigInt) -> Sign;
    fn extract_fractional(&self, val: &BigInt) -> BigInt;
    fn extract_exponent(&self, val: &BigInt) -> i32;

    fn set_sign(&self, val: BigInt, sign: Sign) -> BigInt;

    fn encode_nan(&self, sign: Sign) -> BigInt;
    fn encode_infinity(&self, sign: Sign) -> BigInt;
    fn encode_zero(&self, sign: Sign) -> BigInt;

    fn round_to_lead_bit(&self, val: BigInt, bit: i32) -> BigInt;
}

impl FloatFormatOpsInternal for FloatFormat {
    fn extract_sign(&self, val: &BigInt) -> Sign {
        if val.get_bit(self.sign_pos) { Sign::Positive } else { Sign::Negative }
    }

    fn extract_fractional(&self, val: &BigInt) -> BigInt {
        let mask = (BigInt::from(1) << self.frac_size) - 1;
        BigInt::from(val << self.frac_pos) & mask
    }

    fn extract_exponent(&self, val: &BigInt) -> i32 {
        let m: BigInt = BigInt::from(val >> self.exp_pos) & 0xffff_ffffu32;
        m.to_u32().unwrap() as i32 & self.exp_max
    }

    fn set_sign(&self, val: BigInt, sign: Sign) -> BigInt {
        if matches!(sign, Sign::Negative) {
            let mut val = val;
            val.set_bit(self.sign_pos, true);
            val
        } else {
            val
        }
    }

    fn encode_zero(&self, sign: Sign) -> BigInt {
        self.set_sign(BigInt::new(), sign)
    }

    fn encode_infinity(&self, sign: Sign) -> BigInt {
        let res = BigInt::from(self.exp_max) << self.exp_pos;
        self.set_sign(res, sign)
    }

    fn encode_nan(&self, sign: Sign) -> BigInt {
        let mut res = BigInt::from(1) << self.frac_pos
            .wrapping_add(self.frac_size)
            .wrapping_sub(1);
        res |= BigInt::from(self.exp_max) << self.exp_pos;
        self.set_sign(res, sign)
    }

    fn round_to_lead_bit(&self, val: BigInt, bit: i32) -> BigInt {
        let mut val = val;
        let amount = val
            .significant_bits()
            .wrapping_sub(1)
            .wrapping_sub(bit as u32);
        if amount == 0 {
            val
        } else if (amount as i32) < 0 {
            val << (amount as i32).abs() as u32
        } else {
            let mid = amount.wrapping_sub(1);
            let mid_set = val.get_bit(mid);
            let eps = val.find_one(0).map(|pos| pos < mid).unwrap_or(true);

            val >>= amount;

            let odd = val.get_bit(0);
            if mid_set && (eps || odd) {
                val + 1
            } else {
                val
            }
        }
    }
}

pub trait FloatFormatOps {
    fn into_bitvec(&self, fp: Float, bits: usize) -> BitVec;
    fn from_bitvec(&self, bv: &BitVec) -> Float;
}

impl FloatFormatOps for FloatFormat {
    fn into_bitvec(&self, fp: Float, bits: usize) -> BitVec {
        let mut res = match fp.kind {
            FloatKind::QuietNaN | FloatKind::SignallingNaN => {
                self.encode_nan(Sign::Positive)
            },
            FloatKind::Infinite => {
                let sign = if fp.sign < 0 { Sign::Negative } else { Sign::Positive };
                self.encode_infinity(sign)
            },
            FloatKind::Finite => if fp.is_zero() {
                let sign = if fp.sign < 0 { Sign::Negative } else { Sign::Positive };
                self.encode_zero(sign)
            } else if self.j_bit_implied {
                let lead_bit = fp.leading_bit();
                let (exp, mut frac) = {
                    let tmp = fp.scale
                        .wrapping_sub(fp.frac_bits as i32)
                        .wrapping_add(lead_bit as i32);
                    if tmp >= 1i32.wrapping_sub(self.bias) {
                        let mut exp = tmp.wrapping_add(self.bias);
                        let mut frac = self.round_to_lead_bit(fp.unscaled, self.frac_size as i32);
                        if frac.significant_bits().wrapping_sub(1) > self.frac_size {
                            frac >>= 1;
                            exp += 1;
                        }
                        frac.set_bit(self.frac_size, false);
                        (exp, frac)
                    } else {
                        let exp = 0;
                        let n = tmp
                            .wrapping_sub(1)
                            .wrapping_add(self.bias as i32)
                            .wrapping_add(self.frac_size as i32);
                        if n < 0 {
                            let sign = if fp.sign < 0 {
                                Sign::Negative
                            } else {
                                Sign::Positive
                            };

                            let mut res = self.encode_zero(sign);
                            let sign = res < 0;
                            res.abs_mut();

                            let bv = BitVec::from_bigint(res, bits);
                            return if sign {
                                -bv
                            } else {
                                bv
                            }
                        }
                        let frac = self.round_to_lead_bit(fp.unscaled, n);
                        (exp, frac)
                    }
                };
                if exp >= self.exp_max {
                    let sign = if fp.sign < 0 { Sign::Negative } else { Sign::Positive };
                    let mut res = self.encode_infinity(sign);
                    let sign = res < 0;
                    res.abs_mut();

                    let bv = BitVec::from_bigint(res, bits);
                    return if sign {
                        -bv
                    } else {
                        bv
                    }
                }

                frac |= BigInt::from(exp) << self.exp_pos;
                if fp.sign < 0 {
                    frac.set_bit(self.sign_pos, true);
                }
                frac
            } else {
                panic!("unexpected j_bit_implied == false")
            },
        };

        let sign = res < 0;
        res.abs_mut();

        let bv = BitVec::from_bigint(res, bits);
        if sign {
            -bv
        } else {
            bv
        }
    }

    fn from_bitvec(&self, bv: &BitVec) -> Float {
        let sign = self.extract_sign(bv.as_bigint());
        let exp = self.extract_exponent(bv.as_bigint());
        let mut frac = self.extract_fractional(bv.as_bigint());

        if exp == 0 {
            return if frac == 0 {
                Float::zero_with(self.frac_size, self.exp_size, sign)
            } else {
                Float::from_parts(
                    self.frac_size,
                    self.exp_size,
                    FloatKind::Finite,
                    sign,
                    frac,
                    1i32.wrapping_sub(self.bias)
                )
            }
        } else if exp == self.exp_max {
            return if frac == 0 {
                Float::from_parts(
                    self.frac_size,
                    self.exp_size,
                    FloatKind::Infinite,
                    sign,
                    BigInt::new(),
                    self.exp_max,
                )
            } else {
                Float::from_parts(
                    self.frac_size,
                    self.exp_size,
                    FloatKind::QuietNaN,
                    sign,
                    BigInt::new(),
                    self.exp_max,
                )
            }
        }

        if self.j_bit_implied {
            frac.set_bit(self.frac_size, true);
        }

        Float::from_parts(
            self.frac_size,
            self.exp_size,
            FloatKind::Finite,
            sign,
            frac,
            exp.wrapping_sub(self.bias)
        )
    }
}
