use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;
use std::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Div, DivAssign,
    Mul, MulAssign, Neg, Not, Rem, RemAssign, Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
};
use std::str::FromStr;

use fugue_bytes::Order;
use rug::integer::Order as BVOrder;
use rug::Integer as BigInt;

use crate::error::{ParseError, TryFromBitVecError};

mod mask_table;
use mask_table::lookup_mask;

pub const MAX_BITS: Option<u32> = Some(2048);

/// BitVec(value, mask, is_signed, number of bits)
#[derive(Debug, Clone, Hash, serde::Deserialize, serde::Serialize)]
pub struct BitVec(
    pub(crate) Box<BigInt>,
    pub(crate) u32, // [nbits|sign]
);

impl fmt::Display for BitVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", *self.0, self.bits())
    }
}

impl fmt::LowerHex for BitVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}:{}", *self.0, self.bits())
    }
}

impl fmt::UpperHex for BitVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#X}:{}", *self.0, self.bits())
    }
}

impl fmt::Binary for BitVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#b}:{}", *self.0, self.bits())
    }
}

impl FromStr for BitVec {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (cst, sz) = s.rsplit_once(':').ok_or(ParseError::InvalidFormat)?;

        let val = if let Some(cstv) = cst.strip_prefix("0x") {
            BigInt::from_str_radix(cstv, 16)
        } else {
            BigInt::from_str_radix(cst, 10)
        }
        .map_err(|_| ParseError::InvalidConst)?;

        let bits = usize::from_str(sz).map_err(|_| ParseError::InvalidSize)?;

        Ok(Self::from_bigint(val, bits))
    }
}

impl BitVec {
    #[inline(always)]
    fn pack_meta(sign: bool, bits: u32) -> u32 {
        (bits << 1) | (sign as u32)
    }

    fn set_bits(&mut self, bits: usize) {
        self.1 = ((bits as u32) << 1) | (self.1 & 1);
    }

    pub fn from_bigint(v: BigInt, bits: usize) -> Self {
        Self(Box::new(v), Self::pack_meta(false, bits as u32)).mask()
    }

    pub(crate) fn from_bigint_with(v: BigInt, mask: &'static BigInt) -> Self {
        let bits = mask.significant_bits();
        Self(Box::new(v & mask), Self::pack_meta(false, bits))
    }

    pub(crate) fn mask(mut self) -> Self {
        self.mask_assign();
        self
    }

    pub(crate) fn mask_assign(&mut self) {
        *self.0 &= lookup_mask(self.bits());
    }

    pub fn as_raw(&self) -> &BigInt {
        &*self.0
    }

    pub fn as_bigint(&self) -> Cow<BigInt> {
        Cow::Borrowed(self.as_raw())
    }

    pub fn into_bigint(self) -> BigInt {
        if self.is_negative() {
            -*self.abs().0
        } else {
            *self.0
        }
    }

    pub fn zero(bits: usize) -> Self {
        if bits == 0 {
            //|| bits % 8 != 0 {
            panic!("bits must be > 0")
            //panic!("bits must be multiple of 8 and > 0")
        }
        Self::from_bigint(BigInt::from(0), bits)
    }

    pub fn one(bits: usize) -> Self {
        if bits == 0 {
            //|| bits % 8 != 0 {
            panic!("bits must be > 0")
            //panic!("bits must be multiple of 8 and > 0")
        }
        Self::from_bigint(BigInt::from(1), bits)
    }

    pub fn count_ones(&self) -> u32 {
        self.0.count_ones().unwrap()
    }

    pub fn count_zeros(&self) -> u32 {
        self.bits() as u32 - self.count_ones()
    }

    pub fn leading_ones(&self) -> u32 {
        let mut lo = 0;
        let mut pos = self.bits() as u32;
        while pos > 0 && self.bit(pos - 1) == true {
            lo += 1;
            pos -= 1;
        }
        lo
    }

    pub fn leading_zeros(&self) -> u32 {
        let mut lz = 0;
        let mut pos = self.bits() as u32;
        while pos > 0 && self.bit(pos - 1) == false {
            lz += 1;
            pos -= 1;
        }
        lz
    }

    pub fn bits(&self) -> usize {
        (self.1 >> 1) as usize
    }

    pub fn signed(self) -> Self {
        Self(self.0, self.1 | 1)
    }

    pub fn signed_assign(&mut self) {
        self.1 |= 1;
    }

    pub fn is_zero(&self) -> bool {
        *self.0 == 0
    }

    pub fn is_one(&self) -> bool {
        *self.0 == 1
    }

    pub fn is_signed(&self) -> bool {
        (self.1 & 1) != 0
    }

    pub fn is_negative(&self) -> bool {
        self.is_signed() && self.msb()
    }

    pub fn unsigned(self) -> Self {
        Self(self.0, self.1 & !1)
    }

    pub fn unsigned_assign(&mut self) {
        self.1 &= !1;
    }

    pub fn is_unsigned(&self) -> bool {
        (self.1 & 1) == 0
    }

    pub fn bit(&self, index: u32) -> bool {
        self.0.get_bit(index)
    }

    pub fn set_bit(&mut self, index: u32) {
        self.0.set_bit(index, true);
    }

    pub fn leading_one(&self) -> Option<u32> {
        let lzs = self.leading_zeros();
        if lzs == self.bits() as u32 {
            None
        } else {
            Some(self.bits() as u32 - (1 + lzs))
        }
    }

    pub fn msb(&self) -> bool {
        self.0.get_bit(self.bits() as u32 - 1)
    }

    pub fn lsb(&self) -> bool {
        self.0.get_bit(0)
    }

    pub fn from_be_bytes(buf: &[u8]) -> Self {
        Self::from_bigint(BigInt::from_digits(&buf, BVOrder::MsfBe), buf.len() * 8)
    }

    pub fn from_le_bytes(buf: &[u8]) -> Self {
        Self::from_bigint(BigInt::from_digits(&buf, BVOrder::LsfLe), buf.len() * 8)
    }

    #[inline(always)]
    pub fn from_ne_bytes(buf: &[u8]) -> Self {
        if cfg!(target_endian = "big") {
            Self::from_be_bytes(buf)
        } else {
            Self::from_le_bytes(buf)
        }
    }

    pub fn to_be_bytes(&self, buf: &mut [u8]) {
        let size = self.bits() / 8 + if self.bits() % 8 == 0 { 0 } else { 1 };
        if buf.len() != size {
            panic!("invalid buf size {}; expected {}", buf.len(), size);
        }
        if self.is_negative() {
            buf.iter_mut().for_each(|v| *v = 0xffu8);
        } else {
            buf.iter_mut().for_each(|v| *v = 0u8);
        };

        self.0.write_digits(
            &mut buf[((self.bits() / 8) - self.0.significant_digits::<u8>())..],
            BVOrder::MsfBe,
        );
    }

    pub fn to_le_bytes(&self, buf: &mut [u8]) {
        let size = self.bits() / 8 + if self.bits() % 8 == 0 { 0 } else { 1 };
        if buf.len() != size {
            panic!("invalid buf size {}; expected {}", buf.len(), size);
        }
        if self.is_negative() {
            buf.iter_mut().for_each(|v| *v = 0xffu8);
        } else {
            buf.iter_mut().for_each(|v| *v = 0u8);
        }
        self.0.write_digits(
            &mut buf[..self.0.significant_digits::<u8>()],
            BVOrder::LsfLe,
        );
    }

    #[inline(always)]
    pub fn to_ne_bytes(&self, buf: &mut [u8]) {
        if cfg!(target_endian = "big") {
            self.to_be_bytes(buf)
        } else {
            self.to_le_bytes(buf)
        }
    }

    pub fn from_bytes<O: Order>(bytes: &[u8], signed: bool) -> BitVec {
        let v = if O::ENDIAN.is_big() {
            Self::from_be_bytes(bytes)
        } else {
            Self::from_le_bytes(bytes)
        };

        if signed {
            v.signed()
        } else {
            v
        }
    }

    pub fn into_bytes<O: Order>(self, bytes: &mut [u8]) {
        if O::ENDIAN.is_big() {
            self.to_be_bytes(bytes)
        } else {
            self.to_le_bytes(bytes)
        }
    }

    pub fn abs(&self) -> BitVec {
        if self.is_negative() {
            -self
        } else {
            self.clone()
        }
    }

    pub fn signed_borrow(&self, rhs: &Self) -> bool {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `signed_borrow` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }

        let lneg = self.msb();
        let rneg = rhs.msb();

        let min = -(BigInt::from(1) << (self.bits() - 1) as u32);

        match (lneg, rneg) {
            (false, false) => BigInt::from(&*self.0 - &*rhs.0) < min,
            (true, false) => BigInt::from(-&*(-self).0).sub(&*rhs.0) < min,
            (false, true) => (&*self.0).sub(BigInt::from(-&*(-rhs).0)) < min,
            (true, true) => (BigInt::from(-&*(-self).0) - BigInt::from(-&*(-rhs).0)) < min,
        }
    }

    #[deprecated = "use BitVec::signed_borrow"]
    pub fn borrow(&self, rhs: &Self) -> bool {
        self.signed_borrow(rhs)
    }

    pub fn carry(&self, rhs: &Self) -> bool {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `carry` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        if self.is_signed() || rhs.is_signed() {
            self.signed_carry(rhs)
        } else {
            let max = (BigInt::from(1) << self.bits() as u32) - BigInt::from(1);
            BigInt::from(&*self.0 + &*rhs.0) > max
        }
    }

    pub fn signed_carry(&self, rhs: &Self) -> bool {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `carry` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }

        let lneg = self.msb();
        let rneg = rhs.msb();

        let max = (BigInt::from(1) << (self.bits() - 1) as u32) - BigInt::from(1);

        match (lneg, rneg) {
            (false, false) => BigInt::from(&*self.0 + &*rhs.0) > max,
            (true, false) => BigInt::from(-&*(-self).0).add(&*rhs.0) > max,
            (false, true) => (&*self.0).add(BigInt::from(-&*(-rhs).0)) > max,
            (true, true) => (BigInt::from(-&*(-self).0) + BigInt::from(-&*(-rhs).0)) > max,
        }
    }

    pub fn rem_euclid(&self, rhs: &Self) -> Self {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `rem_euclid` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }

        let r = self.rem(rhs);

        if r.msb() {
            // less than 0
            r + if rhs.msb() { -rhs } else { rhs.clone() }
        } else {
            r
        }
    }

    pub fn lcm(&self, rhs: &Self) -> Self {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `lcm` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }

        Self::from_bigint(self.0.lcm_ref(&*rhs.0).into(), self.bits())
    }

    pub fn gcd(&self, rhs: &Self) -> Self {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `gcd` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }

        if self.is_zero() {
            return rhs.clone();
        }

        if rhs.is_zero() {
            return self.clone();
        }

        Self::from_bigint(self.0.gcd_ref(&*rhs.0).into(), self.bits())
    }

    pub fn gcd_ext(&self, rhs: &Self) -> (Self, Self, Self) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `gcd_ext` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }

        if self.is_zero() {
            return (rhs.clone(), Self::zero(self.bits()), Self::one(self.bits()));
        }

        if rhs.is_zero() {
            return (
                self.clone(),
                Self::one(self.bits()),
                Self::zero(self.bits()),
            );
        }

        let (g, a, b) = self.0.extended_gcd_ref(&*rhs.0).into();
        let mask = lookup_mask(self.bits());
        (
            Self::from_bigint_with(g, mask),
            Self::from_bigint_with(a, mask),
            Self::from_bigint_with(b, mask),
        )
    }

    pub fn max_value_with(bits: usize, signed: bool) -> Self {
        let mask = lookup_mask(bits);
        if signed {
            Self::from_bigint_with(
                (BigInt::from(1) << (bits - 1) as u32) - BigInt::from(1),
                mask,
            )
            .signed()
        } else {
            Self::from_bigint_with((BigInt::from(1) << bits as u32) - BigInt::from(1), mask)
        }
    }

    pub fn max_value(&self) -> Self {
        if self.is_signed() {
            Self::from_bigint(
                (BigInt::from(1) << (self.bits() - 1) as u32) - BigInt::from(1),
                self.bits(),
            )
            .signed()
        } else {
            Self::from_bigint(
                (BigInt::from(1) << self.bits() as u32) - BigInt::from(1),
                self.bits(),
            )
        }
    }

    pub fn min_value_with(bits: usize, signed: bool) -> Self {
        let mask = lookup_mask(bits);
        if signed {
            Self::from_bigint_with(-(BigInt::from(1) << (bits - 1) as u32), mask).signed()
        } else {
            Self::from_bigint_with(BigInt::from(0), mask)
        }
    }

    pub fn min_value(&self) -> Self {
        if self.is_signed() {
            Self::from_bigint(-(BigInt::from(1) << (self.bits() - 1) as u32), self.bits()).signed()
        } else {
            Self::from_bigint(BigInt::from(0), self.bits())
        }
    }

    pub fn signed_cast(&self, size: usize) -> Self {
        self.clone().signed().cast(size)
    }

    pub fn unsigned_cast(&self, size: usize) -> Self {
        self.clone().unsigned().cast(size)
    }

    pub fn signed_cast_assign(&mut self, size: usize) {
        self.signed_assign();
        self.cast_assign(size)
    }

    pub fn unsigned_cast_assign(&mut self, size: usize) {
        self.unsigned_assign();
        self.cast_assign(size)
    }

    pub fn cast(self, size: usize) -> Self {
        if self.is_signed() {
            if size > self.bits() && self.msb() {
                let mask = lookup_mask(size);
                let extm = BigInt::from(lookup_mask(self.bits()) ^ mask);
                Self::from_bigint_with(*self.0 | extm, mask)
            } else {
                Self::from_bigint(*self.0, size)
            }
            .signed()
        } else {
            Self::from_bigint(*self.0, size)
        }
    }

    pub fn cast_assign(&mut self, size: usize) {
        if self.is_signed() {
            if size > self.bits() && self.msb() {
                let mask = lookup_mask(size);
                let extm = BigInt::from(lookup_mask(self.bits()) ^ mask);
                *self.0 |= extm;
                self.set_bits(size);
                self.mask_assign();
                self.signed_assign();
            } else {
                self.1 = Self::pack_meta(true, size as _);
                self.mask_assign();
            }
        } else {
            self.1 = Self::pack_meta(false, size as _);
            self.mask_assign();
        }
    }
}

impl PartialEq<Self> for BitVec {
    fn eq(&self, other: &Self) -> bool {
        self.bits() == other.bits() && self.0 == other.0
    }
}
impl Eq for BitVec {}

impl PartialOrd for BitVec {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for BitVec {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.bits() != other.bits() {
            panic!(
                "bit vector of size {} cannot be compared with bit vector of size {}",
                self.bits(),
                other.bits(),
            )
        }
        let lneg = self.is_negative();
        let rneg = other.is_negative();

        match (lneg, rneg) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => self.0.cmp(&other.0),
        }
    }
}

impl BitVec {
    pub fn signed_cmp(&self, other: &Self) -> Ordering {
        if self.bits() != other.bits() {
            panic!(
                "bit vector of size {} cannot be compared with bit vector of size {}",
                self.bits(),
                other.bits()
            )
        }
        let lneg = self.msb();
        let rneg = other.msb();

        match (lneg, rneg) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => self.0.cmp(&other.0),
        }
    }
}

impl Neg for BitVec {
    type Output = Self;

    fn neg(self) -> Self::Output {
        let mask = lookup_mask(self.bits());
        BitVec::from_bigint_with((*self.0 ^ mask) + 1, mask)
    }
}

impl<'a> Neg for &'a BitVec {
    type Output = BitVec;

    fn neg(self) -> Self::Output {
        let mask = lookup_mask(self.bits());
        BitVec::from_bigint_with(BigInt::from(&*self.0 ^ mask) + 1, mask)
    }
}

impl BitVec {
    pub fn neg_assign(&mut self) {
        *self.0 ^= lookup_mask(self.bits());
        *self.0 += 1;
        self.mask_assign();
    }
}

impl Not for BitVec {
    type Output = Self;

    fn not(self) -> Self::Output {
        let mask = lookup_mask(self.bits());
        Self::from_bigint_with(*self.0 ^ mask, mask)
    }
}

impl<'a> Not for &'a BitVec {
    type Output = BitVec;

    fn not(self) -> Self::Output {
        let mask = lookup_mask(self.bits());
        BitVec::from_bigint_with(BigInt::from(&*self.0 ^ mask), mask)
    }
}

impl BitVec {
    pub fn not_assign(&mut self) {
        *self.0 ^= lookup_mask(self.bits());
        self.mask_assign();
    }
}

impl Add for BitVec {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `+` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        let mask = lookup_mask(self.bits());
        Self::from_bigint_with(*self.0 + *rhs.0, mask)
    }
}

impl<'a> Add for &'a BitVec {
    type Output = BitVec;

    fn add(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `+` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        BitVec::from_bigint_with(BigInt::from(&*self.0 + &*rhs.0), lookup_mask(self.bits()))
    }
}

impl AddAssign for BitVec {
    fn add_assign(&mut self, rhs: Self) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `+` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        *self.0 += *rhs.0;
        self.mask_assign()
    }
}

impl AddAssign<&'_ BitVec> for BitVec {
    fn add_assign(&mut self, rhs: &BitVec) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `+` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        *self.0 += &*rhs.0;
        self.mask_assign()
    }
}

impl Div for BitVec {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `/` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        let lneg = self.is_negative();
        let rneg = rhs.is_negative();
        let size = lookup_mask(self.bits());

        match (lneg, rneg) {
            (false, false) => BitVec::from_bigint_with(*self.0 / *rhs.0, size),
            (true, false) => -BitVec::from_bigint_with(*(-self).0 / *rhs.0, size),
            (false, true) => -BitVec::from_bigint_with(*self.0 / *(-rhs).0, size),
            (true, true) => BitVec::from_bigint_with(*(-self).0 / *(-rhs).0, size),
        }
    }
}

impl<'a> Div for &'a BitVec {
    type Output = BitVec;

    fn div(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `/` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        let lneg = self.is_negative();
        let rneg = rhs.is_negative();

        match (lneg, rneg) {
            (false, false) => {
                BitVec::from_bigint_with(BigInt::from(&*self.0 / &*rhs.0), lookup_mask(self.bits()))
            }
            (true, false) => -BitVec::from_bigint_with(
                BigInt::from(&*(-self).0 / &*rhs.0),
                lookup_mask(self.bits()),
            ),
            (false, true) => -BitVec::from_bigint_with(
                BigInt::from(&*self.0 / &*(-rhs).0),
                lookup_mask(self.bits()),
            ),
            (true, true) => BitVec::from_bigint_with(
                BigInt::from(&*(-self).0 / &*(-rhs).0),
                lookup_mask(self.bits()),
            ),
        }
    }
}

impl DivAssign for BitVec {
    fn div_assign(&mut self, mut rhs: Self) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `/` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }

        let lneg = self.is_negative();
        let rneg = rhs.is_negative();

        match (lneg, rneg) {
            (false, false) => {
                *self.0 /= *rhs.0;
                self.mask_assign()
            }
            (true, false) => {
                self.neg_assign();
                *self.0 /= *rhs.0;
                self.mask_assign();
                self.neg_assign();
            }
            (false, true) => {
                rhs.neg_assign();
                *self.0 /= *rhs.0;
                self.mask_assign();
                self.neg_assign();
            }
            (true, true) => {
                self.neg_assign();
                rhs.neg_assign();
                *self.0 /= *rhs.0;
                self.mask_assign();
            }
        }
    }
}

impl DivAssign<&'_ BitVec> for BitVec {
    fn div_assign(&mut self, rhs: &BitVec) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `/` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }

        let lneg = self.is_negative();
        let rneg = rhs.is_negative();

        match (lneg, rneg) {
            (false, false) => {
                *self.0 /= &*rhs.0;
                self.mask_assign()
            }
            (true, false) => {
                self.neg_assign();
                *self.0 /= &*rhs.0;
                self.mask_assign();
                self.neg_assign();
            }
            (false, true) => {
                let rhs = -rhs;
                *self.0 /= *rhs.0;
                self.mask_assign();
                self.neg_assign();
            }
            (true, true) => {
                self.neg_assign();
                let rhs = -rhs;
                *self.0 /= *rhs.0;
                self.mask_assign();
            }
        }
    }
}

impl BitVec {
    pub fn signed_div(&self, rhs: &Self) -> BitVec {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `/` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        let lneg = self.msb();
        let rneg = rhs.msb();

        match (lneg, rneg) {
            (false, false) => {
                BitVec::from_bigint_with(BigInt::from(&*self.0 / &*rhs.0), lookup_mask(self.bits()))
            }
            (true, false) => -BitVec::from_bigint_with(
                BigInt::from(&*(-self).0 / &*rhs.0),
                lookup_mask(self.bits()),
            ),
            (false, true) => -BitVec::from_bigint_with(
                BigInt::from(&*self.0 / &*(-rhs).0),
                lookup_mask(self.bits()),
            ),
            (true, true) => BitVec::from_bigint_with(
                BigInt::from(&*(-self).0 / &*(-rhs).0),
                lookup_mask(self.bits()),
            ),
        }
    }

    pub fn signed_div_assign(&mut self, rhs: &Self) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `/` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }

        let lneg = self.msb();
        let rneg = rhs.msb();

        match (lneg, rneg) {
            (false, false) => {
                *self.0 /= &*rhs.0;
                self.mask_assign()
            }
            (true, false) => {
                self.neg_assign();
                *self.0 /= &*rhs.0;
                self.mask_assign();
                self.neg_assign();
            }
            (false, true) => {
                let rhs = -rhs;
                *self.0 /= *rhs.0;
                self.mask_assign();
                self.neg_assign();
            }
            (true, true) => {
                let rhs = -rhs;
                self.neg_assign();
                *self.0 /= *rhs.0;
                self.mask_assign();
            }
        }
    }
}

impl Mul for BitVec {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `*` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        let mask = lookup_mask(self.bits());
        Self::from_bigint_with(*self.0 * *rhs.0, mask)
    }
}

impl<'a> Mul for &'a BitVec {
    type Output = BitVec;

    fn mul(self, rhs: Self) -> Self::Output {
        BitVec::from_bigint_with(BigInt::from(&*self.0 * &*rhs.0), lookup_mask(self.bits()))
    }
}

impl MulAssign for BitVec {
    fn mul_assign(&mut self, rhs: Self) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `*` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        *self.0 *= *rhs.0;
        self.mask_assign()
    }
}

impl MulAssign<&'_ BitVec> for BitVec {
    fn mul_assign(&mut self, rhs: &BitVec) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `*` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }

        *self.0 *= &*rhs.0;
        self.mask_assign()
    }
}

impl Rem for BitVec {
    type Output = Self;

    fn rem(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `%` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        let lneg = self.is_negative();
        let rneg = rhs.is_negative();
        let size = lookup_mask(self.bits());

        match (lneg, rneg) {
            (false, false) => BitVec::from_bigint_with(*self.0 % *rhs.0, size),
            (true, false) => -BitVec::from_bigint_with(*(-self).0 % *rhs.0, size),
            (false, true) => BitVec::from_bigint_with(*self.0 % *(-rhs).0, size),
            (true, true) => -BitVec::from_bigint_with(*(-self).0 % *(-rhs).0, size),
        }
    }
}

impl<'a> Rem for &'a BitVec {
    type Output = BitVec;

    fn rem(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `%` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        let lneg = self.is_negative();
        let rneg = rhs.is_negative();

        match (lneg, rneg) {
            (false, false) => {
                BitVec::from_bigint_with(BigInt::from(&*self.0 % &*rhs.0), lookup_mask(self.bits()))
            }
            (true, false) => {
                -BitVec::from_bigint_with(*(-self).0 % &*rhs.0, lookup_mask(self.bits()))
            }
            (false, true) => {
                BitVec::from_bigint_with(&*self.0 % *(-rhs).0, lookup_mask(self.bits()))
            }
            (true, true) => {
                -BitVec::from_bigint_with(*(-self).0 % *(-rhs).0, lookup_mask(self.bits()))
            }
        }
    }
}

impl RemAssign for BitVec {
    fn rem_assign(&mut self, mut rhs: Self) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `%` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }

        let lneg = self.is_negative();
        let rneg = rhs.is_negative();

        match (lneg, rneg) {
            (false, false) => {
                *self.0 %= *rhs.0;
                self.mask_assign()
            }
            (true, false) => {
                self.neg_assign();
                *self.0 %= *rhs.0;
                self.mask_assign();
                self.neg_assign();
            }
            (false, true) => {
                rhs.neg_assign();
                *self.0 %= *rhs.0;
                self.mask_assign();
                self.neg_assign();
            }
            (true, true) => {
                self.neg_assign();
                rhs.neg_assign();
                *self.0 %= *rhs.0;
                self.mask_assign();
            }
        }
    }
}

impl RemAssign<&'_ BitVec> for BitVec {
    fn rem_assign(&mut self, rhs: &BitVec) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `%` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }

        let lneg = self.is_negative();
        let rneg = rhs.is_negative();

        match (lneg, rneg) {
            (false, false) => {
                *self.0 %= &*rhs.0;
                self.mask_assign()
            }
            (true, false) => {
                self.neg_assign();
                *self.0 %= &*rhs.0;
                self.mask_assign();
                self.neg_assign();
            }
            (false, true) => {
                let rhs = -rhs;
                *self.0 %= *rhs.0;
                self.mask_assign();
                self.neg_assign();
            }
            (true, true) => {
                let rhs = -rhs;
                self.neg_assign();
                *self.0 %= *rhs.0;
                self.mask_assign();
            }
        }
    }
}

impl BitVec {
    pub fn signed_rem(&self, rhs: &Self) -> BitVec {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `%` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        let lneg = self.msb();
        let rneg = rhs.msb();

        match (lneg, rneg) {
            (false, false) => {
                BitVec::from_bigint_with(BigInt::from(&*self.0 % &*rhs.0), lookup_mask(self.bits()))
            }
            (true, false) => {
                -BitVec::from_bigint_with(*(-self).0 % &*rhs.0, lookup_mask(self.bits()))
            }
            (false, true) => {
                BitVec::from_bigint_with(&*self.0 % *(-rhs).0, lookup_mask(self.bits()))
            }
            (true, true) => {
                -BitVec::from_bigint_with(*(-self).0 % *(-rhs).0, lookup_mask(self.bits()))
            }
        }
    }

    pub fn signed_rem_assign(&mut self, rhs: &Self) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `%` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }

        let lneg = self.msb();
        let rneg = rhs.msb();

        match (lneg, rneg) {
            (false, false) => {
                *self.0 %= &*rhs.0;
                self.mask_assign()
            }
            (true, false) => {
                self.neg_assign();
                *self.0 %= &*rhs.0;
                self.mask_assign();
                self.neg_assign();
            }
            (false, true) => {
                let rhs = -rhs;
                *self.0 %= *rhs.0;
                self.mask_assign();
                self.neg_assign();
            }
            (true, true) => {
                let rhs = -rhs;
                self.neg_assign();
                *self.0 %= *rhs.0;
                self.mask_assign();
            }
        }
    }
}

impl Sub for BitVec {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `-` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        let mask = lookup_mask(self.bits());
        Self::from_bigint_with(*self.0 - *rhs.0, mask)
    }
}

impl<'a> Sub for &'a BitVec {
    type Output = BitVec;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `-` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        BitVec::from_bigint_with(BigInt::from(&*self.0 - &*rhs.0), lookup_mask(self.bits()))
    }
}

impl SubAssign for BitVec {
    fn sub_assign(&mut self, rhs: Self) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `-` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        *self.0 -= *rhs.0;
        self.mask_assign()
    }
}

impl SubAssign<&'_ BitVec> for BitVec {
    fn sub_assign(&mut self, rhs: &BitVec) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `-` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        *self.0 -= &*rhs.0;
        self.mask_assign()
    }
}

impl BitAnd for BitVec {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `&` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        let mask = lookup_mask(self.bits());
        Self::from_bigint_with(*self.0 & *rhs.0, mask)
    }
}

impl<'a> BitAnd for &'a BitVec {
    type Output = BitVec;

    fn bitand(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `&` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        BitVec::from_bigint_with(BigInt::from(&*self.0 & &*rhs.0), lookup_mask(self.bits()))
    }
}

impl BitAndAssign for BitVec {
    fn bitand_assign(&mut self, rhs: Self) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `&` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        *self.0 &= *rhs.0;
        self.mask_assign()
    }
}

impl BitAndAssign<&'_ BitVec> for BitVec {
    fn bitand_assign(&mut self, rhs: &BitVec) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `&` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        *self.0 &= &*rhs.0;
        self.mask_assign()
    }
}

impl BitOr for BitVec {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `|` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        let mask = lookup_mask(self.bits());
        Self::from_bigint_with(*self.0 | *rhs.0, mask)
    }
}

impl<'a> BitOr for &'a BitVec {
    type Output = BitVec;

    fn bitor(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `|` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        BitVec::from_bigint_with(BigInt::from(&*self.0 | &*rhs.0), lookup_mask(self.bits()))
    }
}

impl BitOrAssign for BitVec {
    fn bitor_assign(&mut self, rhs: Self) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `|` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        *self.0 |= *rhs.0;
        self.mask_assign()
    }
}

impl BitOrAssign<&'_ BitVec> for BitVec {
    fn bitor_assign(&mut self, rhs: &BitVec) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `|` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        *self.0 |= &*rhs.0;
        self.mask_assign()
    }
}

impl BitXor for BitVec {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `^` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        let mask = lookup_mask(self.bits());
        Self::from_bigint_with(*self.0 ^ *rhs.0, mask)
    }
}

impl<'a> BitXor for &'a BitVec {
    type Output = BitVec;

    fn bitxor(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `^` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        BitVec::from_bigint_with(BigInt::from(&*self.0 ^ &*rhs.0), lookup_mask(self.bits()))
    }
}

impl BitXorAssign for BitVec {
    fn bitxor_assign(&mut self, rhs: Self) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `^` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        *self.0 ^= *rhs.0;
        self.mask_assign()
    }
}

impl BitXorAssign<&'_ BitVec> for BitVec {
    fn bitxor_assign(&mut self, rhs: &BitVec) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `^` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        *self.0 ^= &*rhs.0;
        self.mask_assign()
    }
}

impl Shl<u32> for BitVec {
    type Output = Self;

    fn shl(self, rhs: u32) -> Self::Output {
        let mask = lookup_mask(self.bits());
        Self::from_bigint_with(*self.0 << rhs, mask)
    }
}

impl<'a> Shl<u32> for &'a BitVec {
    type Output = BitVec;

    fn shl(self, rhs: u32) -> Self::Output {
        BitVec::from_bigint_with(BigInt::from(&*self.0 << rhs), lookup_mask(self.bits()))
    }
}

impl ShlAssign<u32> for BitVec {
    fn shl_assign(&mut self, rhs: u32) {
        *self.0 <<= rhs;
        self.mask_assign()
    }
}

impl Shl for BitVec {
    type Output = Self;

    fn shl(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `<<` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        if *rhs.0 >= self.bits() {
            Self::zero(self.bits())
        } else {
            if let Some(rhs) = rhs.0.to_u32() {
                let mask = lookup_mask(self.bits());
                Self::from_bigint_with(*self.0 << rhs, mask)
            } else {
                Self::zero(self.bits())
            }
        }
    }
}

impl<'a> Shl for &'a BitVec {
    type Output = BitVec;

    fn shl(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `<<` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        if *rhs.0 >= self.bits() {
            BitVec::zero(self.bits())
        } else {
            if let Some(rhs) = rhs.0.to_u32() {
                BitVec::from_bigint_with(BigInt::from(&*self.0 << rhs), lookup_mask(self.bits()))
            } else {
                BitVec::zero(self.bits())
            }
        }
    }
}

impl ShlAssign for BitVec {
    fn shl_assign(&mut self, rhs: Self) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `<<` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        if *rhs.0 >= self.bits() {
            *self.0 = BigInt::from(0u32);
        } else {
            if let Some(rhs) = rhs.0.to_u32() {
                self.shl_assign(rhs);
            } else {
                *self.0 = BigInt::from(0);
            }
        }
    }
}

impl ShlAssign<&'_ BitVec> for BitVec {
    fn shl_assign(&mut self, rhs: &BitVec) {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `<<` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        if *rhs.0 >= self.bits() {
            *self.0 = BigInt::from(0u32);
        } else {
            if let Some(rhs) = rhs.0.to_u32() {
                self.shl_assign(rhs);
            } else {
                *self.0 = BigInt::from(0);
            }
        }
    }
}

impl Shr<u32> for BitVec {
    type Output = Self;

    fn shr(self, rhs: u32) -> Self::Output {
        let size = self.bits();
        if rhs as usize >= size {
            if self.is_signed() {
                -Self::one(size)
            } else {
                Self::zero(size)
            }
        } else if self.is_negative() {
            // perform ASR
            let smask = lookup_mask(self.bits());
            let mask =
                smask ^ ((BigInt::from(1) << (size - rhs as usize) as u32) - BigInt::from(1));
            Self::from_bigint_with((*self.0 >> rhs) | mask, smask)
        } else {
            let mask = lookup_mask(self.bits());
            Self::from_bigint_with(*self.0 >> rhs, mask)
        }
    }
}

impl<'a> Shr<u32> for &'a BitVec {
    type Output = BitVec;

    fn shr(self, rhs: u32) -> Self::Output {
        let size = self.bits();
        if rhs as usize >= size {
            if self.is_signed() {
                -BitVec::one(size)
            } else {
                BitVec::zero(size)
            }
        } else if self.is_negative() {
            // perform ASR
            let smask = lookup_mask(self.bits());
            let mask =
                smask ^ ((BigInt::from(1) << (size - rhs as usize) as u32) - BigInt::from(1));
            BitVec::from_bigint_with(BigInt::from(&*self.0 >> rhs) | mask, smask)
        } else {
            BitVec::from_bigint_with(BigInt::from(&*self.0 >> rhs), lookup_mask(self.bits()))
        }
    }
}

impl ShrAssign<u32> for BitVec {
    fn shr_assign(&mut self, rhs: u32) {
        let size = self.bits();
        if rhs as usize >= size {
            if self.is_signed() {
                (*self.0).clone_from(&*lookup_mask(self.bits()));
            } else {
                *self.0 = BigInt::from(0u32);
            }
        } else if self.is_negative() {
            // perform ASR
            let smask = lookup_mask(self.bits());
            let mask =
                smask ^ ((BigInt::from(1u32) << (size - rhs as usize) as u32) - BigInt::from(1u32));
            *self.0 >>= rhs;
            *self.0 |= mask;
            self.mask_assign();
        } else {
            *self.0 >>= rhs;
            self.mask_assign();
        }
    }
}

impl Shr for BitVec {
    type Output = Self;

    fn shr(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `>>` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        if *rhs.0 >= self.bits() {
            if self.is_signed() {
                -Self::one(self.bits())
            } else {
                Self::zero(self.bits())
            }
        } else if self.is_negative() {
            // perform ASR
            if let Some(rhs) = rhs.0.to_u32() {
                let smask = lookup_mask(self.bits());
                let mask = smask
                    ^ ((BigInt::from(1) << (self.bits() - rhs as usize) as u32) - BigInt::from(1));
                Self::from_bigint_with((*self.0 >> rhs) | mask, smask)
            } else {
                -Self::one(self.bits())
            }
        } else {
            if let Some(rhs) = rhs.0.to_u32() {
                let mask = lookup_mask(self.bits());
                Self::from_bigint_with(*self.0 >> rhs, mask)
            } else {
                Self::zero(self.bits())
            }
        }
    }
}

impl<'a> Shr for &'a BitVec {
    type Output = BitVec;

    fn shr(self, rhs: Self) -> Self::Output {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `>>` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        if *rhs.0 >= self.bits() {
            if self.is_signed() {
                -BitVec::one(self.bits())
            } else {
                BitVec::zero(self.bits())
            }
        } else if self.is_negative() {
            // perform ASR
            if let Some(rhs) = rhs.0.to_u32() {
                let smask = lookup_mask(self.bits());
                let mask = smask
                    ^ ((BigInt::from(1) << (self.bits() - rhs as usize) as u32) - BigInt::from(1));
                BitVec::from_bigint_with(BigInt::from(&*self.0 >> rhs) | mask, smask)
            } else {
                -BitVec::one(self.bits())
            }
        } else {
            if let Some(rhs) = rhs.0.to_u32() {
                BitVec::from_bigint_with(BigInt::from(&*self.0 >> rhs), lookup_mask(self.bits()))
            } else {
                BitVec::zero(self.bits())
            }
        }
    }
}

impl ShrAssign for BitVec {
    fn shr_assign(&mut self, rhs: BitVec) {
        let size = self.bits();
        if *rhs.0 >= size as u64 {
            if self.is_signed() {
                (*self.0).clone_from(&*lookup_mask(self.bits()));
                self.mask_assign();
            } else {
                *self.0 = BigInt::from(0u32);
            }
        } else if let Some(rhs) = rhs.0.to_u32() {
            self.shr_assign(rhs);
        } else {
            (*self.0).clone_from(&*lookup_mask(self.bits()));
            self.mask_assign();
        }
    }
}

impl ShrAssign<&'_ BitVec> for BitVec {
    fn shr_assign(&mut self, rhs: &BitVec) {
        let size = self.bits();
        if *rhs.0 >= size as u64 {
            if self.is_signed() {
                (*self.0).clone_from(&*lookup_mask(self.bits()));
                self.mask_assign();
            } else {
                *self.0 = BigInt::from(0u32);
            }
        } else if let Some(rhs) = rhs.0.to_u32() {
            self.shr_assign(rhs);
        } else {
            (*self.0).clone_from(&*lookup_mask(self.bits()));
            self.mask_assign();
        }
    }
}

impl BitVec {
    pub fn signed_shr(&self, rhs: &Self) -> BitVec {
        if self.bits() != rhs.bits() {
            panic!(
                "cannot use `>>` with bit vector of size {} and bit vector of size {}",
                self.bits(),
                rhs.bits()
            )
        }
        if *rhs.0 >= self.bits() {
            -BitVec::one(self.bits())
        } else if self.msb() {
            // perform ASR
            if let Some(rhs) = rhs.0.to_u32() {
                let smask = lookup_mask(self.bits());
                let mask = smask
                    ^ ((BigInt::from(1) << (self.bits() - rhs as usize) as u32) - BigInt::from(1));
                BitVec::from_bigint_with(BigInt::from(&*self.0 >> rhs) | mask, smask)
            } else {
                -BitVec::one(self.bits())
            }
        } else {
            if let Some(rhs) = rhs.0.to_u32() {
                BitVec::from_bigint_with(BigInt::from(&*self.0 >> rhs), lookup_mask(self.bits()))
            } else {
                BitVec::zero(self.bits())
            }
        }
    }

    pub fn signed_shr_assign(&mut self, rhs: &Self) {
        self.signed_assign();
        self.signed_shr(rhs);
    }
}

macro_rules! impl_from_for {
    ($t:ident) => {
        impl From<$t> for BitVec {
            fn from(t: $t) -> Self {
                let bits = ::std::mem::size_of::<$t>() * 8;
                BitVec::from_bigint(BigInt::from(t), bits)
            }
        }
    };
}

macro_rules! impls_from_for {
    ($($tname:ident),*) => {
        $(
            impl_from_for!($tname);
        )*
    };
}

macro_rules! impl_to_u_for {
    ($t:tt) => {
        impl BitVec {
            ::paste::paste! {
                pub fn [< to_u $t >](&self) -> Option<[< u $t >]> {
                    self.0.[< to_u $t >]()
                }
            }
        }

        ::paste::paste! {
            impl ::std::convert::TryFrom<&'_ BitVec> for [< u $t >] {
                type Error = TryFromBitVecError;

                fn try_from(bv: &BitVec) -> Result<[< u $t >], TryFromBitVecError> {
                    bv.[< to_u $t >]().ok_or(TryFromBitVecError)
                }
            }
        }

        ::paste::paste! {
            impl ::std::convert::TryFrom<BitVec> for [< u $t >] {
                type Error = TryFromBitVecError;

                fn try_from(bv: BitVec) -> Result<[< u $t >], TryFromBitVecError> {
                    bv.[< to_u $t >]().ok_or(TryFromBitVecError)
                }
            }
        }
    };
}

macro_rules! impl_to_i_for {
    ($t:tt) => {
        impl BitVec {
            ::paste::paste! {
                pub fn [< to_i $t >](&self) -> Option<[< i $t >]> {
                    self.0.[< to_u $t >]().map(|v| v as [< i $t >])
                }
            }
        }

        ::paste::paste! {
            impl ::std::convert::TryFrom<&'_ BitVec> for [< i $t >] {
                type Error = TryFromBitVecError;

                fn try_from(bv: &BitVec) -> Result<[< i $t >], TryFromBitVecError> {
                    bv.[< to_i $t >]().ok_or(TryFromBitVecError)
                }
            }
        }

        ::paste::paste! {
            impl ::std::convert::TryFrom<BitVec> for [< i $t >] {
                type Error = TryFromBitVecError;

                fn try_from(bv: BitVec) -> Result<[< i $t >], TryFromBitVecError> {
                    bv.[< to_i $t >]().ok_or(TryFromBitVecError)
                }
            }
        }
    };
}

macro_rules! impl_from_t_for {
    ($t:ident) => {
        impl BitVec {
            ::paste::paste! {
                pub fn [< from_ $t >](t: $t, bits: usize) -> Self {
                    if bits == 0 { // || bits % 8 != 0 {
                        panic!("bits must be > 0")
                        // panic!("bits must be multiple of 8 and > 0")
                    }
                    BitVec::from_bigint(BigInt::from(t), bits)
                }
            }
        }
    };
}

macro_rules! impls_to_u_for {
    ($($tname:tt),*) => {
        $(
            impl_to_u_for!($tname);
        )*
    };
}

macro_rules! impls_to_i_for {
    ($($tname:tt),*) => {
        $(
            impl_to_i_for!($tname);
        )*
    };
}

macro_rules! impls_from_t_for {
    ($($tname:ident),*) => {
        $(
            impl_from_t_for!($tname);
        )*
    };
}

impls_from_for! { i8, i16, i32, i64, i128, isize }
impls_from_for! { u8, u16, u32, u64, u128, usize }
impls_from_t_for! { i8, i16, i32, i64, i128, isize }
impls_from_t_for! { u8, u16, u32, u64, u128, usize }

impls_to_i_for! { 8, 16, 32, 64, 128, size }
impls_to_u_for! { 8, 16, 32, 64, 128, size }

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_wrapped_add() {
        let v1 = BitVec::from(0xff00u16);
        let v2 = BitVec::from(0x0100u16);

        assert_eq!(v1 + v2, BitVec::zero(16));

        let v3 = BitVec::from_u32(0xffff00, 24);
        let v4 = BitVec::from_u32(0x000100, 24);

        assert_eq!(v3 + v4, BitVec::zero(24));

        let v5 = BitVec::from_i32(-1, 24);
        let v6 = BitVec::from_i32(1, 24);

        assert_eq!(v5 + v6, BitVec::zero(24));
    }

    #[test]
    fn test_wrapped_sub() {
        let v1 = BitVec::from(0xfffeu16);
        let v2 = BitVec::from(0xffffu16);

        assert_eq!(v1 - v2, BitVec::from(0xffffu16));

        let v3 = BitVec::from_u32(0xfffffe, 24);
        let v4 = BitVec::from_u32(0xffffff, 24);

        assert_eq!(v3 - v4, BitVec::from_u32(0xffffff, 24));
    }

    #[test]
    fn test_signed_shift_right() {
        let v1 = BitVec::from(0xffffu16);
        assert_eq!(v1 >> 4, BitVec::from(0x0fffu16));

        let v2 = BitVec::from(0xffffu16);
        assert_eq!(v2.signed() >> 4, BitVec::from(0xffffu16));

        let v3 = BitVec::from(0x8000u16);
        assert_eq!(v3.signed() >> 4, BitVec::from(0xf800u16));

        let v4 = BitVec::from(0x05deu16);
        assert_eq!(v4.signed() >> 1u32, BitVec::from(0x2efu16));
    }

    #[test]
    fn test_signed_rem() {
        let v1 = BitVec::from(-100i64);
        let v2 = BitVec::from(-27i64);

        assert_eq!(v1.signed().rem(v2.signed()), BitVec::from(-19i64));

        let v3 = BitVec::from(-100i64);
        let v4 = BitVec::from(27i64);

        assert_eq!(v3.signed().rem(v4), BitVec::from(-19i64));

        let v5 = BitVec::from(100i64);
        let v6 = BitVec::from(-27i64);

        assert_eq!(v5.rem(v6.signed()), BitVec::from(19i64));

        let v7 = BitVec::from(100i64);
        let v8 = BitVec::from(27i64);

        assert_eq!(v7.signed().rem(v8), BitVec::from(19i64));
    }

    #[test]
    fn test_signed_rem_euclid() {
        let v1 = BitVec::from(-100i64);
        let v2 = BitVec::from(-27i64);

        assert_eq!(v1.signed().rem_euclid(&v2.signed()), BitVec::from(8i64));

        let v3 = BitVec::from(-100i64);
        let v4 = BitVec::from(27i64);

        assert_eq!(v3.signed().rem_euclid(&v4), BitVec::from(8i64));

        let v5 = BitVec::from(100i64);
        let v6 = BitVec::from(-27i64);

        assert_eq!(v5.rem_euclid(&v6.signed()), BitVec::from(19i64));

        let v7 = BitVec::from(100i64);
        let v8 = BitVec::from(27i64);

        assert_eq!(v7.signed().rem_euclid(&v8), BitVec::from(19i64));

        let v1 = BitVec::from(7i64);
        let v2 = BitVec::from(4i64);

        assert_eq!(v1.signed().rem_euclid(&v2.signed()), BitVec::from(3i64));

        let v3 = BitVec::from(-7i64);
        let v4 = BitVec::from(4i64);

        assert_eq!(v3.signed().rem_euclid(&v4), BitVec::from(1i64));

        let v5 = BitVec::from(7i64);
        let v6 = BitVec::from(-4i64);

        assert_eq!(v5.rem_euclid(&v6.signed()), BitVec::from(3i64));

        let v7 = BitVec::from(-7i64);
        let v8 = BitVec::from(-4i64);

        assert_eq!(v7.signed().rem_euclid(&v8.signed()), BitVec::from(1i64));
    }

    #[test]
    fn test_abs() {
        let v1 = BitVec::from(0x8000_0000u32).signed();
        assert_eq!(v1.abs(), BitVec::from(0x8000_0000u32));

        let v2 = BitVec::from(0x8000_0001u32).signed();
        assert_eq!(v2.abs(), BitVec::from(0x7fff_ffffu32));
    }

    #[test]
    fn test_compare() {
        let v1 = BitVec::from(0x8000_0000u32);
        let v2 = BitVec::from(0x8000_0001u32);
        let v3 = BitVec::from(0xffff_ffffu32).signed();

        assert_eq!(v1 < v2, true);
        assert_eq!(v1 < v3, false);
        assert_eq!(v3 < v1, true);
        assert_eq!(v3 < v2, true);
        assert_eq!(v1.clone().signed() == v1, true);
    }

    #[test]
    fn test_byte_convert() {
        let v1 = BitVec::from_be_bytes(&[0xff, 0xff]);
        let v2 = BitVec::from_be_bytes(&[0x80, 0x00]);
        let v3 = BitVec::from_be_bytes(&[0x7f, 0xff]);

        assert_eq!(v1, BitVec::from(0xffffu16));
        assert_eq!(v2, BitVec::from(0x8000u16));
        assert_eq!(v3, BitVec::from(0x7fffu16));

        let mut buf = [0u8; 2];

        v1.to_be_bytes(&mut buf);
        assert_eq!(&buf, &[0xff, 0xff]);

        v2.to_be_bytes(&mut buf);
        assert_eq!(&buf, &[0x80, 0x00]);

        v3.to_be_bytes(&mut buf);
        assert_eq!(&buf, &[0x7f, 0xff]);

        v1.to_le_bytes(&mut buf);
        assert_eq!(&buf, &[0xff, 0xff]);

        v2.to_le_bytes(&mut buf);
        assert_eq!(&buf, &[0x00, 0x80]);

        v3.to_le_bytes(&mut buf);
        assert_eq!(&buf, &[0xff, 0x7f]);
    }

    #[test]
    fn test_signed_borrow() {
        let v1 = BitVec::from(0x8000u16);
        let v2 = BitVec::from(0x1u16);

        assert_eq!(v1.signed_borrow(&v2), true);

        let v3 = BitVec::from(0x8001u16);
        let v4 = BitVec::from(0x1u16);

        assert_eq!(v3.signed_borrow(&v4), false);
    }

    #[test]
    fn test_count_bits() {
        let v0 = BitVec::from_u32(0xff, 16);
        assert_eq!(8, v0.count_ones());
        assert_eq!(8, v0.count_zeros());
    }

    #[test]
    fn test_3bit() {
        let v0 = BitVec::zero(3);
        assert_eq!(v0.bits(), 3);

        let v1 = v0.max_value();
        assert_eq!(v1, BitVec::from_u64(0b111, 3));

        let v2 = BitVec::one(3);
        assert_eq!(v2, BitVec::from_u64(0b1, 3));

        assert_eq!(v1.sub(v2), BitVec::from_u64(0b110, 3))
    }

    #[test]
    fn test_size() {
        assert_eq!(
            std::mem::size_of::<BitVec>(),
            std::mem::size_of::<crate::core_u64::BitVec>()
        );
        assert_eq!(std::mem::size_of::<BitVec>(), 16);
    }
}
