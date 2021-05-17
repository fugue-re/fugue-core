use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Shl, Shr, Sub};

use num_bigint::BigInt;
use num_traits::{FromPrimitive, One, Zero};

#[derive(Debug, Clone, Hash)]
pub struct BitVec<const N: usize>(BigInt, BigInt, bool);

impl<const N: usize> fmt::Display for BitVec<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.0, N)
    }
}

impl<const N: usize> fmt::LowerHex for BitVec<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:x}:{}", self.0, N)
    }
}

impl<const N: usize> fmt::UpperHex for BitVec<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:X}:{}", self.0, N)
    }
}

impl<const N: usize> fmt::Binary for BitVec<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:b}:{}", self.0, N)
    }
}

impl<const N: usize> From<BigInt> for BitVec<N> {
    fn from(v: BigInt) -> Self {
        Self(v, Self::mask_value(), false).mask()
    }
}

impl<const N: usize> BitVec<N> {
    fn mask_value() -> BigInt {
        (BigInt::one() << N) - BigInt::one()
    }

    pub fn zero() -> Self {
        Self::from(BigInt::zero())
    }

    pub fn one() -> Self {
        Self::from(BigInt::one())
    }

    pub fn bits(&self) -> usize {
        N
    }

    pub fn signed(&self) -> Self {
        Self(self.0.clone(), self.1.clone(), true)
    }

    pub fn is_signed(&self) -> bool {
        self.2
    }

    pub fn is_negative(&self) -> bool {
        self.2 && self.msb()
    }

    pub fn unsigned(&self) -> Self {
        Self(self.0.clone(), self.1.clone(), false)
    }

    pub fn is_unsigned(&self) -> bool {
        !self.2
    }

    pub fn msb(&self) -> bool {
        self.0.bit(N as u64 - 1)
    }

    pub fn lsb(&self) -> bool {
        self.0.bit(0)
    }

    fn mask(self) -> Self {
        Self(self.0 & self.1, Self::mask_value(), false)
    }

    pub fn convert<const M: usize>(self) -> BitVec<{ M }> {
        if self.is_signed() {
            if M > N && self.0.bit(N as u64 - 1) { // negative; extension
                let mask = ((BigInt::one() << M) - BigInt::one()) ^ self.1;
                BitVec::<{ M }>::from(self.0 | mask)
            } else { // truncate
                BitVec::<{ M }>::from(self.0)
            }.signed()
        } else {
            BitVec::<{ M }>::from(self.0)
        }
    }
}

impl<const N: usize> BitVec<N> {
    pub fn abs(&self) -> BitVec<N> {
        if self.is_negative() {
            -self
        } else {
            self.clone()
        }
    }

    pub fn borrow(&self, rhs: &Self) -> bool {
        let min = if self.is_signed() || rhs.is_signed() {
            -(BigInt::one() << (N - 1))
        } else {
            BigInt::zero()
        };
        (&self.0 - &rhs.0) < min
    }

    pub fn carry(&self, rhs: &Self) -> bool {
        let max = if self.is_signed() || rhs.is_signed() {
            (BigInt::one() << (N - 1)) - BigInt::one()
        } else {
            (BigInt::one() << N) - BigInt::one()
        };
        (&self.0 + &rhs.0) > max
    }

    pub fn max_value(&self) -> Self {
        if self.is_signed() {
            Self::from((BigInt::one() << (N - 1)) - BigInt::one()).signed()
        } else {
            Self::from((BigInt::one() << N) - BigInt::one())
        }
    }

    pub fn min_value(&self) -> Self {
        if self.is_signed() {
            Self::from(-(BigInt::one() << (N - 1))).signed()
        } else {
            Self::zero()
        }
    }
}

impl<const N: usize> PartialEq<Self> for BitVec<N> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<const N: usize> Eq for BitVec<N> { }

impl<const N: usize> PartialOrd for BitVec<N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<const N: usize> Ord for BitVec<N> {
    fn cmp(&self, other: &Self) -> Ordering {
        let lneg = self.is_negative();
        let rneg = other.is_negative();

        if lneg || rneg {
            let lhs = if lneg { -(-self).0 } else { self.0.clone() };
            let rhs = if rneg { -(-other).0 } else { other.0.clone() };

            lhs.cmp(&rhs)
        } else {
            self.0.cmp(&other.0)
        }
    }
}

impl<const N: usize> Neg for BitVec<N> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        !self + Self::one()
    }
}

impl<'a, const N: usize> Neg for &'a BitVec<N> {
    type Output = BitVec<N>;

    fn neg(self) -> Self::Output {
        !self + BitVec::one()
    }
}

impl<const N: usize> Add for BitVec<N> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::from(self.0 + rhs.0)
    }
}

impl<'a, const N: usize> Add for &'a BitVec<N> {
    type Output = BitVec<N>;

    fn add(self, rhs: Self) -> Self::Output {
        BitVec::from(&self.0 + &rhs.0)
    }
}

impl<const N: usize> Div for BitVec<N> {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        if self.is_signed() || rhs.is_signed() {
            let lmsb = self.msb();
            let rmsb = rhs.msb();

            match (lmsb, rmsb) {
                (false, false) => BitVec::from(self.0 / rhs.0),
                (true, false) => -BitVec::from((-self).0 / rhs.0),
                (false, true) => -BitVec::from(self.0 / (-rhs).0),
                (true, true) => BitVec::from((-self).0 / (-rhs).0),
            }
        } else {
            BitVec::from(self.0 / rhs.0)
        }
    }
}

impl<'a, const N: usize> Div for &'a BitVec<N> {
    type Output = BitVec<N>;

    fn div(self, rhs: Self) -> Self::Output {
        if self.is_signed() || rhs.is_signed() {
            let lmsb = self.msb();
            let rmsb = rhs.msb();

            match (lmsb, rmsb) {
                (false, false) => BitVec::from(&self.0 / &rhs.0),
                (true, false) => -BitVec::from((-self).0 / &rhs.0),
                (false, true) => -BitVec::from(&self.0 / (-rhs).0),
                (true, true) => BitVec::from((-self).0 / (-rhs).0),
            }
        } else {
            BitVec::from(&self.0 / &rhs.0)
        }
    }
}

impl<const N: usize> BitVec<N> {
    pub fn rem_euclid(&self, rhs: &Self) -> Self {
        let r = self.rem(rhs);

        if r.msb() { // less than 0
            r + if rhs.msb() { -rhs } else { rhs.clone() }
        } else {
            r
        }
    }
}

impl<const N: usize> Rem for BitVec<N> {
    type Output = Self;

    fn rem(self, rhs: Self) -> Self::Output {
        if self.is_signed() || rhs.is_signed() {
            let lmsb = self.msb();
            let rmsb = rhs.msb();

            match (lmsb, rmsb) {
                (false, false) => Self::from(self.0 % rhs.0),
                (true, false) =>  -Self::from((-self).0 % rhs.0),
                (false, true) => Self::from(self.0 % (-rhs).0),
                (true, true) => -Self::from((-self).0 % (-rhs).0),
            }
        } else {
            Self::from(self.0 % rhs.0)
        }
    }
}

impl<'a, const N: usize> Rem for &'a BitVec<N> {
    type Output = BitVec<N>;

    fn rem(self, rhs: Self) -> Self::Output {
        if self.is_signed() || rhs.is_signed() {
            let lmsb = self.msb();
            let rmsb = rhs.msb();

            match (lmsb, rmsb) {
                (false, false) => BitVec::from(&self.0 % &rhs.0),
                (true, false) =>  -BitVec::from((-self).0 % &rhs.0),
                (false, true) => BitVec::from(&self.0 % (-rhs).0),
                (true, true) => -BitVec::from((-self).0 % (-rhs).0),
            }
        } else {
            BitVec::from(&self.0 % &rhs.0)
        }
    }
}

impl<const N: usize> Mul for BitVec<N> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::from(self.0 * rhs.0)
    }
}

impl<'a, const N: usize> Mul for &'a BitVec<N> {
    type Output = BitVec<N>;

    fn mul(self, rhs: Self) -> Self::Output {
        BitVec::from(&self.0 * &rhs.0)
    }
}

impl<const N: usize> Shl<u32> for BitVec<N> {
    type Output = Self;

    fn shl(self, rhs: u32) -> Self::Output {
        Self::from(self.0 << rhs)
    }
}

impl<'a, const N: usize> Shl<u32> for &'a BitVec<N> {
    type Output = BitVec<N>;

    fn shl(self, rhs: u32) -> Self::Output {
        BitVec::from(&self.0 << rhs)
    }
}

impl<const N: usize> Shr<u32> for BitVec<N> {
    type Output = Self;

    fn shr(self, rhs: u32) -> Self::Output {
        if rhs as usize >= N {
            Self::zero()
        } else if self.is_signed() { // perform ASR
            let mask = self.1 ^ ((BigInt::one() << (N - rhs as usize)) - BigInt::one());
            Self::from((self.0 >> rhs) ^ mask)
        } else {
            Self::from(self.0 >> rhs)
        }
    }
}

impl<'a, const N: usize> Shr<u32> for &'a BitVec<N> {
    type Output = BitVec<N>;

    fn shr(self, rhs: u32) -> Self::Output {
        if rhs as usize >= N {
            BitVec::zero()
        } else if self.is_signed() { // perform ASR
            let mask = &self.1 ^ ((BigInt::one() << (N - rhs as usize)) - BigInt::one());
            BitVec::from((&self.0 >> rhs) ^ mask)
        } else {
            BitVec::from(&self.0 >> rhs)
        }
    }
}

impl<const N: usize> BitAnd for BitVec<N> {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::from(self.0 & rhs.0)
    }
}

impl<'a, const N: usize> BitAnd for &'a BitVec<N> {
    type Output = BitVec<N>;

    fn bitand(self, rhs: Self) -> Self::Output {
        BitVec::from(&self.0 & &rhs.0)
    }
}

impl<const N: usize> BitOr for BitVec<N> {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::from(self.0 | rhs.0)
    }
}

impl<'a, const N: usize> BitOr for &'a BitVec<N> {
    type Output = BitVec<N>;

    fn bitor(self, rhs: Self) -> Self::Output {
        BitVec::from(&self.0 | &rhs.0)
    }
}

impl<const N: usize> BitXor for BitVec<N> {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        Self::from(self.0 ^ rhs.0)
    }
}

impl<'a, const N: usize> BitXor for &'a BitVec<N> {
    type Output = BitVec<N>;

    fn bitxor(self, rhs: Self) -> Self::Output {
        BitVec::from(&self.0 ^ &rhs.0)
    }
}

impl<const N: usize> Zero for BitVec<N> {
    fn zero() -> Self {
        Self::from(BigInt::zero())
    }

    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl<const N: usize> One for BitVec<N> {
    fn one() -> Self {
        Self::from(BigInt::one())
    }
}

impl<const N: usize> Not for BitVec<N> {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::from(self.0 ^ self.1)
    }
}

impl<'a, const N: usize> Not for &'a BitVec<N> {
    type Output = BitVec<N>;

    fn not(self) -> Self::Output {
        BitVec::from(&self.0 ^ &self.1)
    }
}

impl<const N: usize> Sub for BitVec<N> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::from(self.0 - rhs.0)
    }
}

impl<'a, const N: usize> Sub for &'a BitVec<N> {
    type Output = BitVec<N>;

    fn sub(self, rhs: Self) -> Self::Output {
        BitVec::from(&self.0 - &rhs.0)
    }
}

impl<const N: usize> FromPrimitive for BitVec<N> {
    fn from_i64(n: i64) -> Option<Self> {
        BigInt::from_i64(n).map(Self::from)
    }

    fn from_u64(n: u64) -> Option<Self> {
        BigInt::from_u64(n).map(Self::from)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_wrapped_add() {
        let v1 = BitVec::<16>::from_u16(0xff00).unwrap();
        let v2 = BitVec::<16>::from_u16(0x0100).unwrap();

        assert_eq!(v1 + v2, BitVec::<16>::zero());

        let v3 = BitVec::<24>::from_u32(0xffff00).unwrap();
        let v4 = BitVec::<24>::from_u32(0x000100).unwrap();

        assert_eq!(v3 + v4, BitVec::<24>::zero());
    }

    #[test]
    fn test_wrapped_sub() {
        let v1 = BitVec::<16>::from_u16(0xfffe).unwrap();
        let v2 = BitVec::<16>::from_u16(0xffff).unwrap();

        assert_eq!(v1 - v2, BitVec::<16>::from_u16(0xffff).unwrap());

        let v3 = BitVec::<24>::from_u32(0xfffffe).unwrap();
        let v4 = BitVec::<24>::from_u32(0xffffff).unwrap();

        assert_eq!(v3 - v4, BitVec::<24>::from_u32(0xffffff).unwrap());
    }

    #[test]
    fn test_signed_shift_right() {
        let v1 = BitVec::<16>::from_u16(0xffff).unwrap();
        assert_eq!(v1 >> 4, BitVec::<16>::from_u16(0x0fff).unwrap());

        let v2 = BitVec::<16>::from_u16(0xffff).unwrap();
        assert_eq!(v2.signed() >> 4, BitVec::<16>::from_u16(0xffff).unwrap());

        let v3 = BitVec::<16>::from_u16(0x8000).unwrap();
        assert_eq!(v3.signed() >> 4, BitVec::<16>::from_u16(0xf800).unwrap());
    }

    #[test]
    fn test_signed_rem() {
        let v1 = BitVec::<64>::from_i64(-100).unwrap();
        let v2 = BitVec::<64>::from_i64(-27).unwrap();

        assert_eq!(v1.signed().rem(v2.signed()), BitVec::<64>::from_i64(-19).unwrap());

        let v3 = BitVec::<64>::from_i64(-100).unwrap();
        let v4 = BitVec::<64>::from_i64(27).unwrap();

        assert_eq!(v3.signed().rem(v4), BitVec::<64>::from_i64(-19).unwrap());

        let v5 = BitVec::<64>::from_i64(100).unwrap();
        let v6 = BitVec::<64>::from_i64(-27).unwrap();

        assert_eq!(v5.rem(v6.signed()), BitVec::<64>::from_i64(19).unwrap());

        let v7 = BitVec::<64>::from_i64(100).unwrap();
        let v8 = BitVec::<64>::from_i64(27).unwrap();

        assert_eq!(v7.signed().rem(v8), BitVec::<64>::from_i64(19).unwrap());
    }

    #[test]
    fn test_signed_rem_euclid() {
        let v1 = BitVec::<64>::from_i64(-100).unwrap();
        let v2 = BitVec::<64>::from_i64(-27).unwrap();

        assert_eq!(v1.signed().rem_euclid(&v2.signed()), BitVec::<64>::from_i64(8).unwrap());

        let v3 = BitVec::<64>::from_i64(-100).unwrap();
        let v4 = BitVec::<64>::from_i64(27).unwrap();

        assert_eq!(v3.signed().rem_euclid(&v4), BitVec::<64>::from_i64(8).unwrap());

        let v5 = BitVec::<64>::from_i64(100).unwrap();
        let v6 = BitVec::<64>::from_i64(-27).unwrap();

        assert_eq!(v5.rem_euclid(&v6.signed()), BitVec::<64>::from_i64(19).unwrap());

        let v7 = BitVec::<64>::from_i64(100).unwrap();
        let v8 = BitVec::<64>::from_i64(27).unwrap();

        assert_eq!(v7.signed().rem_euclid(&v8), BitVec::<64>::from_i64(19).unwrap());

        let v1 = BitVec::<64>::from_i64(7).unwrap();
        let v2 = BitVec::<64>::from_i64(4).unwrap();

        assert_eq!(v1.signed().rem_euclid(&v2.signed()), BitVec::<64>::from_i64(3).unwrap());

        let v3 = BitVec::<64>::from_i64(-7).unwrap();
        let v4 = BitVec::<64>::from_i64(4).unwrap();

        assert_eq!(v3.signed().rem_euclid(&v4), BitVec::<64>::from_i64(1).unwrap());

        let v5 = BitVec::<64>::from_i64(7).unwrap();
        let v6 = BitVec::<64>::from_i64(-4).unwrap();

        assert_eq!(v5.rem_euclid(&v6.signed()), BitVec::<64>::from_i64(3).unwrap());

        let v7 = BitVec::<64>::from_i64(-7).unwrap();
        let v8 = BitVec::<64>::from_i64(-4).unwrap();

        assert_eq!(v7.signed().rem_euclid(&v8.signed()), BitVec::<64>::from_i64(1).unwrap());
    }

    #[test]
    fn test_abs() {
        let v1 = BitVec::<32>::from_u32(0x8000_0000).unwrap().signed();
        assert_eq!(v1.abs(), BitVec::from_u32(0x8000_0000).unwrap());

        let v2 = BitVec::<32>::from_u32(0x8000_0001).unwrap().signed();
        assert_eq!(v2.abs(), BitVec::from_u32(0x7fff_ffff).unwrap());
    }

    #[test]
    fn test_ord() {
        let v1 = BitVec::<32>::from_u32(0x8000_0000).unwrap();
        let v2 = BitVec::<32>::from_u32(0x8000_0001).unwrap();
        let v3 = BitVec::<32>::from_u32(0xffff_ffff).unwrap();

        assert_eq!(v1 < v2, true);
        assert_eq!(v1 < v3.signed(), false);
        assert_eq!(v3.signed() < v1, true);
        assert_eq!(v3.signed() < v2, true);
    }
}
