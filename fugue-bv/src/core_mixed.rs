use fugue_bytes::Order;

use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;
use std::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Div, DivAssign,
    Mul, MulAssign, Neg, Not, Rem, RemAssign, Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
};
use std::str::FromStr;
use std::sync::Arc;

use rug::Integer as BigInt;

use crate::error::{ParseError, TryFromBitVecError};
use crate::{core_bigint, core_u128, core_u64};

pub const MAX_BITS: Option<u32> = None;

#[derive(Debug, Clone, Hash, serde::Deserialize, serde::Serialize)]
pub enum BitVec {
    N(core_u64::BitVec),
    B(core_u128::BitVec),
    U(core_bigint::BitVec),
}

#[inline(always)]
fn apply1<F, T, O>(f: F, t: T) -> O
where
    F: FnOnce(T) -> O,
{
    f(t)
}

fn apply2<F, T, O>(f: F, u: T, v: T) -> O
where
    F: FnOnce(T, T) -> O,
{
    f(u, v)
}

fn apply2_mut<F, T>(f: F, u: &mut T, v: &T)
where
    F: FnOnce(&mut T, &T),
{
    f(u, v)
}

macro_rules! bind {
    ($self:ident, $f:expr) => {{
        match $self {
            BitVec::N(m) => BitVec::N(apply1($f, m)),
            BitVec::B(m) => BitVec::B(apply1($f, m)),
            BitVec::U(m) => BitVec::U(apply1($f, m)),
        }
    }};
    (ref $self:ident, $f:expr) => {{
        match $self {
            BitVec::N(ref m) => Self::N(apply1($f, m)),
            BitVec::B(ref m) => Self::B(apply1($f, m)),
            BitVec::U(ref m) => Self::U(apply1($f, m)),
        }
    }};
    (ref mut $self:ident, $f:expr) => {{
        match $self {
            BitVec::N(ref mut m) => Self::N(apply1($f, m)),
            BitVec::B(ref mut m) => Self::B(apply1($f, m)),
            BitVec::U(ref mut m) => Self::U(apply1($f, m)),
        }
    }};
}

macro_rules! fold_map {
    ($self:ident, $f:expr) => {{
        match $self {
            BitVec::N(m) => apply1($f, m),
            BitVec::B(m) => apply1($f, m),
            BitVec::U(m) => apply1($f, m),
        }
    }};
    (ref $self:ident, $f:expr) => {{
        match $self {
            BitVec::N(ref m) => apply1($f, m),
            BitVec::B(ref m) => apply1($f, m),
            BitVec::U(ref m) => apply1($f, m),
        }
    }};
    (ref mut $self:ident, $f:expr) => {{
        match $self {
            BitVec::N(ref mut m) => apply1($f, m),
            BitVec::B(ref mut m) => apply1($f, m),
            BitVec::U(ref mut m) => apply1($f, m),
        }
    }};
}

macro_rules! bind2 {
    ($self:ident, $other:ident, $f:expr) => {{
        match ($self, $other) {
            (BitVec::N(m), BitVec::N(n)) => BitVec::N(apply2($f, m, n)),
            (BitVec::B(m), BitVec::B(n)) => BitVec::B(apply2($f, m, n)),
            (BitVec::U(m), BitVec::U(n)) => BitVec::U(apply2($f, m, n)),
            _ => panic!("cannot apply operation to operands with different bit sizes"),
        }
    }};
    (ref $self:ident, $other:ident, $f:expr) => {{
        match ($self, $other) {
            (BitVec::N(ref m), BitVec::N(ref n)) => BitVec::N(apply2($f, m, n)),
            (BitVec::B(ref m), BitVec::B(ref n)) => BitVec::B(apply2($f, m, n)),
            (BitVec::U(ref m), BitVec::U(ref n)) => BitVec::U(apply2($f, m, n)),
            _ => panic!("cannot apply operation to operands with different bit sizes"),
        }
    }};
    (ref mut $self:ident, $other:ident, $f:expr) => {{
        match ($self, $other) {
            (BitVec::N(ref mut m), BitVec::N(ref mut n)) => BitVec::N(apply2($f, m, n)),
            (BitVec::B(ref mut m), BitVec::B(ref mut n)) => BitVec::B(apply2($f, m, n)),
            (BitVec::U(ref mut m), BitVec::U(ref mut n)) => BitVec::U(apply2($f, m, n)),
            _ => panic!("cannot apply operation to operands with different bit sizes"),
        }
    }};
}

macro_rules! fold_map2 {
    ($self:ident, $other:ident, $f:expr) => {{
        match ($self, $other) {
            (BitVec::N(m), BitVec::N(n)) => apply2($f, m, n),
            (BitVec::B(m), BitVec::B(n)) => apply2($f, m, n),
            (BitVec::U(m), BitVec::U(n)) => apply2($f, m, n),
            _ => panic!("cannot apply operation to operands with different bit sizes"),
        }
    }};
    (ref $self:ident, $other:ident, $f:expr) => {{
        match ($self, $other) {
            (BitVec::N(ref m), BitVec::N(ref n)) => apply2($f, m, n),
            (BitVec::B(ref m), BitVec::B(ref n)) => apply2($f, m, n),
            (BitVec::U(ref m), BitVec::U(ref n)) => apply2($f, m, n),
            _ => panic!("cannot apply operation to operands with different bit sizes"),
        }
    }};
    (ref mut $self:ident, ref $other:ident, $f:expr) => {{
        match ($self, $other) {
            (BitVec::N(ref mut m), BitVec::N(ref n)) => apply2_mut($f, m, n),
            (BitVec::B(ref mut m), BitVec::B(ref n)) => apply2_mut($f, m, n),
            (BitVec::U(ref mut m), BitVec::U(ref n)) => apply2_mut($f, m, n),
            _ => panic!("cannot apply operation to operands with different bit sizes"),
        }
    }};
    (ref mut $self:ident, ref mut $other:ident, $f:expr) => {{
        match ($self, $other) {
            (BitVec::N(ref mut m), BitVec::N(ref mut n)) => apply2($f, m, n),
            (BitVec::B(ref mut m), BitVec::B(ref mut n)) => apply2($f, m, n),
            (BitVec::U(ref mut m), BitVec::U(ref mut n)) => apply2($f, m, n),
            _ => panic!("cannot apply operation to operands with different bit sizes"),
        }
    }};
}

impl From<core_u64::BitVec> for BitVec {
    fn from(bv: core_u64::BitVec) -> Self {
        Self::N(bv)
    }
}

impl From<core_u128::BitVec> for BitVec {
    fn from(bv: core_u128::BitVec) -> Self {
        Self::B(bv)
    }
}

impl From<core_bigint::BitVec> for BitVec {
    fn from(bv: core_bigint::BitVec) -> Self {
        Self::U(bv)
    }
}

impl fmt::Display for BitVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fold_map!(self, |slf| write!(f, "{}:{}", slf.0, slf.bits()))
    }
}

impl fmt::LowerHex for BitVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fold_map!(self, |slf| write!(f, "{:#x}:{}", slf.0, slf.bits()))
    }
}

impl fmt::UpperHex for BitVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fold_map!(self, |slf| write!(f, "{:#X}:{}", slf.0, slf.bits()))
    }
}

impl fmt::Binary for BitVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fold_map!(self, |slf| write!(f, "{:#b}:{}", slf.0, slf.bits()))
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
    pub fn from_str_radix(s: &str, radix: u32) -> Result<Self, ParseError> {
        let (cst, sz) = s.rsplit_once(':').ok_or(ParseError::InvalidFormat)?;
        let val =
            BigInt::from_str_radix(cst, radix as i32).map_err(|_| ParseError::InvalidConst)?;

        let bits = usize::from_str(sz).map_err(|_| ParseError::InvalidSize)?;
        Ok(Self::from_bigint(val, bits))
    }
}

impl BitVec {
    pub fn from_bigint(v: BigInt, bits: usize) -> Self {
        if bits <= 64 {
            let v = core_bigint::BitVec::from_bigint(v, bits).to_u64().unwrap();
            Self::N(core_u64::BitVec::from_uint(v, bits))
        } else if bits <= 128 {
            let v = core_bigint::BitVec::from_bigint(v, bits).to_u128().unwrap();
            Self::B(core_u128::BitVec::from_uint(v, bits))
        } else {
            Self::U(core_bigint::BitVec::from_bigint(v, bits))
        }
    }

    #[allow(unused)]
    pub(crate) fn from_bigint_with(v: BigInt, mask: Arc<BigInt>) -> Self {
        let bits = mask.count_ones().unwrap() as usize;
        if bits <= 64 {
            let v = core_bigint::BitVec::from_bigint_with(v, mask)
                .to_u64()
                .unwrap();
            Self::N(core_u64::BitVec::from_uint(v, bits))
        } else if bits <= 128 {
            let v = core_bigint::BitVec::from_bigint_with(v, mask)
                .to_u128()
                .unwrap();
            Self::B(core_u128::BitVec::from_uint(v, bits))
        } else {
            Self::U(core_bigint::BitVec::from_bigint_with(v, mask))
        }
    }

    pub fn as_bigint(&self) -> Cow<BigInt> {
        match self {
            Self::N(ref bv) => Cow::Owned(BigInt::from(bv.0)),
            Self::B(ref bv) => Cow::Owned(BigInt::from(bv.0)),
            Self::U(ref bv) => Cow::Borrowed(bv.as_raw()),
        }
    }

    pub fn zero(bits: usize) -> Self {
        if bits <= 64 {
            Self::N(core_u64::BitVec::zero(bits))
        } else if bits <= 128 {
            Self::B(core_u128::BitVec::zero(bits))
        } else {
            Self::U(core_bigint::BitVec::zero(bits))
        }
    }

    pub fn one(bits: usize) -> Self {
        if bits <= 64 {
            Self::N(core_u64::BitVec::one(bits))
        } else if bits <= 128 {
            Self::B(core_u128::BitVec::one(bits))
        } else {
            Self::U(core_bigint::BitVec::one(bits))
        }
    }

    pub fn count_ones(&self) -> u32 {
        fold_map!(self, |slf| slf.count_ones())
    }

    pub fn count_zeros(&self) -> u32 {
        fold_map!(self, |slf| slf.count_zeros())
    }

    pub fn leading_ones(&self) -> u32 {
        fold_map!(self, |slf| slf.leading_ones())
    }

    pub fn leading_zeros(&self) -> u32 {
        fold_map!(self, |slf| slf.leading_zeros())
    }

    pub fn bits(&self) -> usize {
        fold_map!(self, |slf| slf.bits())
    }

    pub fn signed(self) -> Self {
        bind!(self, |slf| slf.signed())
    }

    pub fn signed_assign(&mut self) {
        fold_map!(ref mut self, |slf| slf.signed_assign());
    }

    pub fn is_zero(&self) -> bool {
        fold_map!(self, |slf| slf.is_zero())
    }

    pub fn is_one(&self) -> bool {
        fold_map!(self, |slf| slf.is_one())
    }

    pub fn is_signed(&self) -> bool {
        fold_map!(self, |slf| slf.is_signed())
    }

    pub fn is_negative(&self) -> bool {
        fold_map!(self, |slf| slf.is_negative())
    }

    pub fn unsigned(self) -> Self {
        bind!(self, |slf| slf.unsigned())
    }

    pub fn unsigned_assign(&mut self) {
        fold_map!(ref mut self, |slf| slf.unsigned_assign());
    }

    pub fn is_unsigned(&self) -> bool {
        fold_map!(self, |slf| slf.is_unsigned())
    }

    pub fn bit(&self, index: u32) -> bool {
        fold_map!(self, |slf| slf.bit(index))
    }

    pub fn set_bit(&mut self, index: u32) {
        fold_map!(self, |slf| slf.set_bit(index))
    }

    pub fn leading_one(&self) -> Option<u32> {
        fold_map!(self, |slf| slf.leading_one())
    }

    pub fn msb(&self) -> bool {
        fold_map!(self, |slf| slf.msb())
    }

    pub fn lsb(&self) -> bool {
        fold_map!(self, |slf| slf.lsb())
    }

    pub fn from_be_bytes(buf: &[u8]) -> Self {
        if buf.len() <= 8 {
            Self::N(core_u64::BitVec::from_be_bytes(buf))
        } else if buf.len() <= 16 {
            Self::B(core_u128::BitVec::from_be_bytes(buf))
        } else {
            Self::U(core_bigint::BitVec::from_be_bytes(buf))
        }
    }

    pub fn from_le_bytes(buf: &[u8]) -> Self {
        if buf.len() <= 8 {
            Self::N(core_u64::BitVec::from_le_bytes(buf))
        } else if buf.len() <= 16 {
            Self::B(core_u128::BitVec::from_le_bytes(buf))
        } else {
            Self::U(core_bigint::BitVec::from_le_bytes(buf))
        }
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
        fold_map!(self, |slf| slf.to_be_bytes(buf))
    }

    pub fn to_le_bytes(&self, buf: &mut [u8]) {
        fold_map!(self, |slf| slf.to_le_bytes(buf))
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

    pub fn incr(&self, value: u64) -> Self {
        self + &BitVec::from_u64(value, self.bits())
    }

    pub fn succ(&self) -> Self {
        self.incr(1)
    }

    pub fn decr(&self, value: u64) -> Self {
        self - &BitVec::from_u64(value, self.bits())
    }

    pub fn pred(&self) -> Self {
        self.decr(1)
    }

    pub fn abs(&self) -> BitVec {
        if self.is_negative() {
            -self
        } else {
            self.clone()
        }
    }

    pub fn lcm(&self, rhs: &Self) -> Self {
        bind2!(self, rhs, |slf, rhs| slf.lcm(rhs))
    }

    pub fn gcd(&self, rhs: &Self) -> Self {
        bind2!(self, rhs, |slf, rhs| slf.gcd(rhs))
    }

    pub fn gcd_ext(&self, rhs: &Self) -> (Self, Self, Self) {
        fold_map2!(self, rhs, |slf, rhs| {
            let (g, a, b) = slf.gcd_ext(rhs);
            (g.into(), a.into(), b.into())
        })
    }

    pub fn signed_borrow(&self, rhs: &Self) -> bool {
        fold_map2!(self, rhs, |slf, rhs| slf.signed_borrow(rhs))
    }

    pub fn carry(&self, rhs: &Self) -> bool {
        fold_map2!(self, rhs, |slf, rhs| slf.carry(rhs))
    }

    pub fn signed_carry(&self, rhs: &Self) -> bool {
        fold_map2!(self, rhs, |slf, rhs| slf.signed_carry(rhs))
    }

    pub fn rem_euclid(&self, rhs: &Self) -> Self {
        bind2!(self, rhs, |slf, rhs| slf.rem_euclid(rhs))
    }

    pub fn max_value_with(bits: usize, signed: bool) -> Self {
        if bits <= 64 {
            Self::N(core_u64::BitVec::max_value_with(bits, signed))
        } else if bits <= 128 {
            Self::B(core_u128::BitVec::max_value_with(bits, signed))
        } else {
            Self::U(core_bigint::BitVec::max_value_with(bits, signed))
        }
    }

    pub fn max_value(&self) -> Self {
        bind!(self, |slf| slf.max_value())
    }

    pub fn min_value_with(bits: usize, signed: bool) -> Self {
        if bits <= 64 {
            Self::N(core_u64::BitVec::min_value_with(bits, signed))
        } else if bits <= 128 {
            Self::B(core_u128::BitVec::min_value_with(bits, signed))
        } else {
            Self::U(core_bigint::BitVec::min_value_with(bits, signed))
        }
    }

    pub fn min_value(&self) -> Self {
        bind!(self, |slf| slf.min_value())
    }

    pub fn signed_cast(&self, size: usize) -> Self {
        self.clone().signed().cast(size)
    }

    pub fn unsigned_cast(&self, size: usize) -> Self {
        self.clone().unsigned().cast(size)
    }

    pub fn signed_cast_assign(&mut self, bits: usize) {
        fold_map!(ref mut self, |slf| slf.signed_cast_assign(bits));
    }

    pub fn unsigned_cast_assign(&mut self, bits: usize) {
        fold_map!(ref mut self, |slf| slf.unsigned_cast_assign(bits));
    }

    pub fn cast(self, size: usize) -> Self {
        let signed = self.is_signed();
        match self {
            Self::N(bv) => {
                if size <= 64 {
                    Self::N(bv.cast(size))
                } else if size <= 128 {
                    let v = core_u128::BitVec::from_u64(bv.0, bv.bits());
                    Self::B(if signed {
                        v.signed().cast(size)
                    } else {
                        v.cast(size)
                    })
                } else {
                    let v = core_bigint::BitVec::from_u64(bv.0, bv.bits());
                    Self::U(if signed {
                        v.signed().cast(size)
                    } else {
                        v.cast(size)
                    })
                }
            }
            Self::B(bv) => {
                if size <= 64 {
                    let v = core_u64::BitVec::from_u64(bv.0 as u64, 64);
                    Self::N(if signed {
                        v.cast(size).signed()
                    } else {
                        v.cast(size)
                    })
                } else if size <= 128 {
                    Self::B(bv.cast(size))
                } else {
                    let v = core_bigint::BitVec::from_u128(bv.0, bv.bits());
                    Self::U(if signed {
                        v.signed().cast(size)
                    } else {
                        v.cast(size)
                    })
                }
            }
            Self::U(bv) => {
                if size <= 64 {
                    let v = core_u64::BitVec::from_u64(bv.cast(64).to_u64().unwrap(), 64);
                    Self::N(if signed {
                        v.cast(size).signed()
                    } else {
                        v.cast(size)
                    })
                } else if size <= 128 {
                    let v = core_u128::BitVec::from_u128(bv.cast(128).to_u128().unwrap(), 128);
                    Self::B(if signed {
                        v.cast(size).signed()
                    } else {
                        v.cast(size)
                    })
                } else {
                    Self::U(bv.cast(size))
                }
            }
        }
    }

    pub fn cast_assign(&mut self, size: usize) {
        let signed = self.is_signed();
        match self {
            Self::N(ref mut bv) => {
                if size <= 64 {
                    bv.cast_assign(size);
                } else if size <= 128 {
                    let mut v = core_u128::BitVec::from_u64(bv.0, bv.bits());
                    if signed {
                        v.signed_cast_assign(size);
                    } else {
                        v.cast_assign(size);
                    }
                    *self = Self::B(v);
                } else {
                    let mut v = core_bigint::BitVec::from_u64(bv.0, bv.bits());
                    if signed {
                        v.signed_cast_assign(size);
                    } else {
                        v.cast_assign(size);
                    }
                    *self = Self::U(v);
                }
            }
            Self::B(ref mut bv) => {
                if size <= 64 {
                    let mut v = core_u64::BitVec::from_u64(bv.0 as u64, 64);
                    if signed {
                        v.signed_cast_assign(size);
                    } else {
                        v.cast_assign(size);
                    }
                    *self = Self::N(v);
                } else if size <= 128 {
                    bv.cast_assign(size);
                } else {
                    let mut v = core_bigint::BitVec::from_u128(bv.0, bv.bits());
                    if signed {
                        v.signed_cast_assign(size);
                    } else {
                        v.cast_assign(size);
                    }
                    *self = Self::U(v);
                }
            }
            Self::U(ref mut bv) => {
                if size <= 64 {
                    bv.cast_assign(64);
                    let mut v = core_u64::BitVec::from_u64(bv.to_u64().unwrap(), 64);
                    if signed {
                        v.signed_cast_assign(size);
                    } else {
                        v.cast_assign(size);
                    }
                    *self = Self::N(v);
                } else if size <= 128 {
                    bv.cast_assign(128);
                    let mut v = core_u128::BitVec::from_u128(bv.to_u128().unwrap(), 128);
                    if signed {
                        v.signed_cast_assign(size);
                    } else {
                        v.cast_assign(size);
                    }
                    *self = Self::B(v);
                } else {
                    bv.cast_assign(size);
                }
            }
        }
    }
}

impl PartialEq<Self> for BitVec {
    fn eq(&self, other: &Self) -> bool {
        self.bits() == other.bits() && fold_map2!(self, other, |slf, other| slf.eq(other))
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
        fold_map2!(self, other, |slf, other| slf.cmp(other))
    }
}

impl BitVec {
    pub fn signed_cmp(&self, other: &Self) -> Ordering {
        fold_map2!(self, other, |slf, other| slf.signed_cmp(other))
    }
}

impl Neg for BitVec {
    type Output = Self;

    fn neg(self) -> Self::Output {
        bind!(self, |slf| slf.neg())
    }
}

impl BitVec {
    pub fn neg_assign(&mut self) {
        fold_map!(ref mut self, |slf| slf.neg_assign())
    }
}

impl<'a> Neg for &'a BitVec {
    type Output = BitVec;

    fn neg(self) -> Self::Output {
        bind!(self, |slf| slf.neg())
    }
}

impl Not for BitVec {
    type Output = Self;

    fn not(self) -> Self::Output {
        bind!(self, |slf| slf.not())
    }
}

impl<'a> Not for &'a BitVec {
    type Output = BitVec;

    fn not(self) -> Self::Output {
        bind!(self, |slf| slf.not())
    }
}

impl BitVec {
    pub fn not_assign(&mut self) {
        fold_map!(ref mut self, |slf| slf.not_assign())
    }
}

impl Add for BitVec {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.add(rhs))
    }
}

impl<'a> Add for &'a BitVec {
    type Output = BitVec;

    fn add(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.add(rhs))
    }
}

impl AddAssign for BitVec {
    fn add_assign(&mut self, rhs: Self) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.add_assign(rhs))
    }
}

impl AddAssign<&'_ BitVec> for BitVec {
    fn add_assign(&mut self, rhs: &BitVec) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.add_assign(rhs))
    }
}

impl Div for BitVec {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.div(rhs))
    }
}

impl<'a> Div for &'a BitVec {
    type Output = BitVec;

    fn div(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.div(rhs))
    }
}

impl DivAssign for BitVec {
    fn div_assign(&mut self, rhs: Self) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.div_assign(rhs))
    }
}

impl DivAssign<&'_ BitVec> for BitVec {
    fn div_assign(&mut self, rhs: &BitVec) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.div_assign(rhs))
    }
}

impl BitVec {
    pub fn signed_div(&self, rhs: &Self) -> BitVec {
        bind2!(self, rhs, |slf, rhs| slf.signed_div(rhs))
    }

    pub fn signed_div_assign(&mut self, rhs: &Self) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.signed_div_assign(rhs))
    }
}

impl Mul for BitVec {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.mul(rhs))
    }
}

impl<'a> Mul for &'a BitVec {
    type Output = BitVec;

    fn mul(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.mul(rhs))
    }
}

impl MulAssign for BitVec {
    fn mul_assign(&mut self, rhs: Self) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.mul_assign(rhs))
    }
}

impl MulAssign<&'_ BitVec> for BitVec {
    fn mul_assign(&mut self, rhs: &BitVec) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.mul_assign(rhs))
    }
}

impl Rem for BitVec {
    type Output = Self;

    fn rem(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.rem(rhs))
    }
}

impl<'a> Rem for &'a BitVec {
    type Output = BitVec;

    fn rem(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.rem(rhs))
    }
}

impl RemAssign for BitVec {
    fn rem_assign(&mut self, rhs: Self) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.rem_assign(rhs))
    }
}

impl RemAssign<&'_ BitVec> for BitVec {
    fn rem_assign(&mut self, rhs: &BitVec) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.rem_assign(rhs))
    }
}

impl BitVec {
    pub fn signed_rem(&self, rhs: &Self) -> BitVec {
        bind2!(self, rhs, |slf, rhs| slf.signed_rem(rhs))
    }

    pub fn signed_rem_assign(&mut self, rhs: &Self) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.signed_rem_assign(rhs))
    }
}

impl Sub for BitVec {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.sub(rhs))
    }
}

impl<'a> Sub for &'a BitVec {
    type Output = BitVec;

    fn sub(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.sub(rhs))
    }
}

impl SubAssign for BitVec {
    fn sub_assign(&mut self, rhs: Self) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.sub_assign(rhs))
    }
}

impl SubAssign<&'_ BitVec> for BitVec {
    fn sub_assign(&mut self, rhs: &BitVec) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.sub_assign(rhs))
    }
}

impl BitAnd for BitVec {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.bitand(rhs))
    }
}

impl<'a> BitAnd for &'a BitVec {
    type Output = BitVec;

    fn bitand(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.bitand(rhs))
    }
}

impl BitAndAssign for BitVec {
    fn bitand_assign(&mut self, rhs: Self) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.bitand_assign(rhs))
    }
}

impl BitAndAssign<&'_ BitVec> for BitVec {
    fn bitand_assign(&mut self, rhs: &BitVec) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.bitand_assign(rhs))
    }
}

impl BitOr for BitVec {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.bitor(rhs))
    }
}

impl<'a> BitOr for &'a BitVec {
    type Output = BitVec;

    fn bitor(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.bitor(rhs))
    }
}

impl BitOrAssign for BitVec {
    fn bitor_assign(&mut self, rhs: Self) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.bitor_assign(rhs))
    }
}

impl BitOrAssign<&'_ BitVec> for BitVec {
    fn bitor_assign(&mut self, rhs: &BitVec) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.bitor_assign(rhs))
    }
}

impl BitXor for BitVec {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.bitxor(rhs))
    }
}

impl<'a> BitXor for &'a BitVec {
    type Output = BitVec;

    fn bitxor(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.bitxor(rhs))
    }
}

impl BitXorAssign for BitVec {
    fn bitxor_assign(&mut self, rhs: Self) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.bitxor_assign(rhs))
    }
}

impl BitXorAssign<&'_ BitVec> for BitVec {
    fn bitxor_assign(&mut self, rhs: &BitVec) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.bitxor_assign(rhs))
    }
}

impl Shl<u32> for BitVec {
    type Output = Self;

    fn shl(self, rhs: u32) -> Self::Output {
        bind!(self, |slf| slf.shl(rhs))
    }
}

impl<'a> Shl<u32> for &'a BitVec {
    type Output = BitVec;

    fn shl(self, rhs: u32) -> Self::Output {
        bind!(self, |slf| slf.shl(rhs))
    }
}

impl ShlAssign<u32> for BitVec {
    fn shl_assign(&mut self, rhs: u32) {
        fold_map!(ref mut self, |slf| slf.shl_assign(rhs))
    }
}

impl Shl for BitVec {
    type Output = Self;

    fn shl(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.shl(rhs))
    }
}

impl<'a> Shl for &'a BitVec {
    type Output = BitVec;

    fn shl(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.shl(rhs))
    }
}

impl ShlAssign for BitVec {
    fn shl_assign(&mut self, rhs: Self) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.shl_assign(rhs))
    }
}

impl ShlAssign<&'_ BitVec> for BitVec {
    fn shl_assign(&mut self, rhs: &BitVec) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.shl_assign(rhs))
    }
}

impl Shr<u32> for BitVec {
    type Output = Self;

    fn shr(self, rhs: u32) -> Self::Output {
        bind!(self, |slf| slf.shr(rhs))
    }
}

impl<'a> Shr<u32> for &'a BitVec {
    type Output = BitVec;

    fn shr(self, rhs: u32) -> Self::Output {
        bind!(self, |slf| slf.shr(rhs))
    }
}

impl ShrAssign<u32> for BitVec {
    fn shr_assign(&mut self, rhs: u32) {
        fold_map!(ref mut self, |slf| slf.shr_assign(rhs))
    }
}

impl Shr for BitVec {
    type Output = Self;

    fn shr(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.shr(rhs))
    }
}

impl<'a> Shr for &'a BitVec {
    type Output = BitVec;

    fn shr(self, rhs: Self) -> Self::Output {
        bind2!(self, rhs, |slf, rhs| slf.shr(rhs))
    }
}

impl ShrAssign for BitVec {
    fn shr_assign(&mut self, rhs: Self) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.shr_assign(rhs))
    }
}

impl ShrAssign<&'_ BitVec> for BitVec {
    fn shr_assign(&mut self, rhs: &BitVec) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.shr_assign(rhs))
    }
}

impl BitVec {
    pub fn signed_shr(&self, rhs: &Self) -> BitVec {
        bind2!(self, rhs, |slf, rhs| slf.signed_shr(rhs))
    }

    pub fn signed_shr_assign(&mut self, rhs: &Self) {
        fold_map2!(ref mut self, ref rhs, |slf, rhs| slf.signed_shr_assign(rhs))
    }
}

macro_rules! impl_from_for {
    ($t:ident) => {
        impl From<$t> for BitVec {
            fn from(t: $t) -> Self {
                let bits = ::std::mem::size_of::<$t>() * 8;
                if bits <= 64 {
                    Self::N(core_u64::BitVec::from(t))
                } else if bits <= 128 {
                    Self::B(core_u128::BitVec::from(t))
                } else {
                    Self::U(core_bigint::BitVec::from(t))
                }
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
                    fold_map!(self, |slf| slf.[< to_u $t >]())
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
                    fold_map!(self, |slf| slf.[< to_i $t >]())
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
                    if bits <= 64 {
                        Self::N(core_u64::BitVec::[< from_ $t >](t, bits))
                    } else if bits <= 128 {
                        Self::B(core_u128::BitVec::[< from_ $t >](t, bits))
                    } else {
                        Self::U(core_bigint::BitVec::[< from_ $t >](t, bits))
                    }
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

        let v5 = BitVec::from_u32(0x0, 120);
        let v6 = BitVec::from_u32(0x1, 120);

        assert_eq!(v5 - v6, -BitVec::from_i32(0x1, 120))
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
    fn test_1bit() {
        let v0 = BitVec::zero(1);
        assert_eq!(v0.bits(), 1);

        let v1 = v0.max_value();
        assert_eq!(v1, BitVec::from_u64(0b1, 1));

        let v2 = BitVec::one(1);
        assert_eq!(v2, BitVec::from_u64(0b1, 1));

        assert_eq!(v1.sub(v2.clone()), BitVec::from_u64(0b0, 1));
        assert_eq!(v0.sub(v2), BitVec::from_u64(0b1, 1));
    }

    #[test]
    fn test_3bit() {
        let v0 = BitVec::zero(3);
        assert_eq!(v0.bits(), 3);

        let v1 = v0.max_value();
        assert_eq!(v1, BitVec::from_u64(0b111, 3));

        let v2 = BitVec::one(3);
        assert_eq!(v2, BitVec::from_u64(0b1, 3));

        assert_eq!(v1.sub(v2.clone()), BitVec::from_u64(0b110, 3));
        assert_eq!(v0.sub(v2), BitVec::from_u64(0b111, 3));

        assert_eq!(
            BitVec::from_u64(5, 3).mul(BitVec::from_u64(3, 3).neg()),
            BitVec::from_u64(1, 3)
        );
    }

    #[test]
    fn test_parse() -> Result<(), ParseError> {
        assert_eq!("0x100:129".parse::<BitVec>()?.bits(), 129);
        assert_eq!("0x100:0".parse::<BitVec>()?.bits(), 0);
        Ok(())
    }
}
