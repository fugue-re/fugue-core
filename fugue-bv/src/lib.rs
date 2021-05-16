use std::fmt;
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Not, Rem, Shl, Shr, Sub};

use num_bigint::BigInt;
use num_traits::{FromPrimitive, Num, One, Zero};

pub trait Sort:
    Zero
    + One
    + Num
    + FromPrimitive
    + BitAnd<Self, Output = Self>
    + BitOr<Self, Output = Self>
    + BitXor<Self, Output = Self>
    + Shl<usize, Output = Self>
    + Shl<u32, Output = Self>
    + Shr<u32, Output = Self>
    + Add<Self, Output = Self>
    + Sub<Self, Output = Self>
    + Mul<Self, Output = Self>
    + Div<Self, Output = Self>
    + Rem<Self, Output = Self>
{
    const IS_SIGNED: bool;

    fn sign(&self) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
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

    pub fn signed(self) -> Self {
        Self(self.0, self.1, true)
    }

    fn mask(self) -> Self {
        Self(self.0 & self.1, Self::mask_value(), false)
    }

    pub fn convert<const M: usize>(self) -> BitVec<{ M }> {
        BitVec::<{ M }>::from(self.0)
    }
}

impl<const N: usize> BitVec<N> {
    pub fn max_value() -> Self {
        Self::from((BigInt::one() << N) - BigInt::one())
    }

    pub fn min_value() -> Self {
        Self::zero()
    }
}

impl<const N: usize> Add for BitVec<N> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::from(self.0 + rhs.0)
    }
}

impl<const N: usize> Div for BitVec<N> {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self::from(self.0 / rhs.0)
    }
}

impl<const N: usize> Rem for BitVec<N> {
    type Output = Self;

    fn rem(self, rhs: Self) -> Self::Output {
        Self::from(self.0 % rhs.0)
    }
}

impl<const N: usize> Mul for BitVec<N> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::from(self.0 * rhs.0)
    }
}

impl<const N: usize> Shl<u32> for BitVec<N> {
    type Output = Self;

    fn shl(self, rhs: u32) -> Self::Output {
        Self::from(self.0 << rhs)
    }
}

impl<const N: usize> Shr<u32> for BitVec<N> {
    type Output = Self;

    fn shr(self, rhs: u32) -> Self::Output {
        Self::from(self.0 >> rhs)
    }
}

impl<const N: usize> BitAnd for BitVec<N> {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::from(self.0 & rhs.0)
    }
}

impl<const N: usize> BitOr for BitVec<N> {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::from(self.0 | rhs.0)
    }
}

impl<const N: usize> BitXor for BitVec<N> {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        Self::from(self.0 ^ rhs.0)
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
        Self::max_value() ^ self
    }
}

impl<const N: usize> Sub for BitVec<N> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        if rhs > self {
            Self::max_value() - Self::from(rhs.0 - self.0 - BigInt::one())
        } else {
            Self::from(self.0 - rhs.0)
        }
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

        println!("{:x}", v3);

        assert_eq!(v3 - v4, BitVec::<24>::from_u32(0xffffff).unwrap());
    }
}
