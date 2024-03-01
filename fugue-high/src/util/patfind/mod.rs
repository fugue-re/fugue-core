use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::{Display, Write};
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use fugue_ir::endian::Endian;

use nom::branch::alt;
use nom::bytes::complete::is_a;
use nom::character::complete::space0;
use nom::error::{Error, ErrorKind};
use nom::multi::fold_many1;
use nom::sequence::delimited;
use nom::{Err, Finish, IResult};

use regex::bytes::RegexBuilder;
pub use regex::bytes::{Match, Regex};

use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use static_init::dynamic;
use thiserror::Error;

#[dynamic(lazy)]
pub static ARM_LE_32: PatternSet = PatternSet::from_str(include_str!("./arm-le-32.yml")).unwrap();

#[derive(Debug, Error)]
pub enum PatternError {
    #[error("cannot parse pattern: {0:?}")]
    Parse(ErrorKind),
}

#[derive(Clone)]
pub struct Pattern {
    data: Vec<u8>,
    mask: Vec<u8>,
    norm: Regex,
    bits: u32,
}

const HTABLE: [u8; 16] = [
    0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66,
];

const VTABLE_02: [u8; 256] = [
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x1, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
];

const MTABLE_02: [u8; 256] = [
    0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1,
    0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1,
    0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x0, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1,
    0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1,
    0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1,
    0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1,
    0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1,
    0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1,
    0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1,
    0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1,
    0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1,
    0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1,
    0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1,
    0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1, 0x1,
];

const VTABLE_16: [u8; 256] = [
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8,
    0x9, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xa, 0xb, 0xc, 0xd, 0xe, 0xf, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0xa, 0xb, 0xc, 0xd, 0xe, 0xf, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
];

const MTABLE_16: [u8; 256] = [
    0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf,
    0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf,
    0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0x0, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf,
    0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf,
    0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf,
    0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf,
    0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf,
    0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf,
    0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf,
    0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf,
    0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf,
    0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf,
    0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf,
    0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf, 0xf,
];

impl Pattern {
    fn parse_hex(input: &str) -> IResult<&str, (u8, u8)> {
        let (input, pattern) = is_a(b"0123456789abcdefABCDEF.".as_ref())(input)?;
        if pattern.len() != 2 {
            return Err(Err::Error(Error::new(input, ErrorKind::Satisfy)));
        }

        let bytes = pattern.as_bytes();

        let v0 = VTABLE_16[bytes[0] as usize] << 4;
        let m0 = MTABLE_16[bytes[0] as usize] << 4;

        let v1 = VTABLE_16[bytes[1] as usize];
        let m1 = MTABLE_16[bytes[1] as usize];

        let v = v0 | v1;
        let m = m0 | m1;

        Ok((input, (v, m)))
    }

    fn parse_bin(input: &str) -> IResult<&str, (u8, u8)> {
        let (input, pattern) = is_a(b"01.".as_ref())(input)?;
        if pattern.len() != 8 {
            return Err(Err::Error(Error::new(input, ErrorKind::Satisfy)));
        }

        let bytes = pattern.as_bytes();

        let v0 = VTABLE_02[bytes[0] as usize] << 7;
        let m0 = MTABLE_02[bytes[0] as usize] << 7;

        let v1 = VTABLE_02[bytes[1] as usize] << 6;
        let m1 = MTABLE_02[bytes[1] as usize] << 6;

        let v2 = VTABLE_02[bytes[2] as usize] << 5;
        let m2 = MTABLE_02[bytes[2] as usize] << 5;

        let v3 = VTABLE_02[bytes[3] as usize] << 4;
        let m3 = MTABLE_02[bytes[3] as usize] << 4;

        let v4 = VTABLE_02[bytes[4] as usize] << 3;
        let m4 = MTABLE_02[bytes[4] as usize] << 3;

        let v5 = VTABLE_02[bytes[5] as usize] << 2;
        let m5 = MTABLE_02[bytes[5] as usize] << 2;

        let v6 = VTABLE_02[bytes[6] as usize] << 1;
        let m6 = MTABLE_02[bytes[6] as usize] << 1;

        let v7 = VTABLE_02[bytes[7] as usize];
        let m7 = MTABLE_02[bytes[7] as usize];

        let v = v0 | v1 | v2 | v3 | v4 | v5 | v6 | v7;
        let m = m0 | m1 | m2 | m3 | m4 | m5 | m6 | m7;

        Ok((input, (v, m)))
    }

    fn parse_one(input: &str) -> IResult<&str, (u8, u8)> {
        alt((Self::parse_hex, Self::parse_bin))(input)
    }

    fn parse(input: &str) -> IResult<&str, Pattern> {
        let (input, (data, mask, regex)) = fold_many1(
            delimited(space0, Self::parse_one, space0),
            || (Vec::new(), Vec::new(), String::new()),
            |(mut acc_v, mut acc_m, mut acc_r), (v, m)| {
                acc_v.push(v);
                acc_m.push(m);
                if m != 0xff {
                    acc_r.push('.');
                } else {
                    acc_r.push_str("\\x");
                    acc_r.push(HTABLE[(v >> 4) as usize] as char);
                    acc_r.push(HTABLE[(v & 0xf) as usize] as char);
                }
                (acc_v, acc_m, acc_r)
            },
        )(input)?;

        let norm = RegexBuilder::new(regex.as_ref())
            .unicode(false)
            .dot_matches_new_line(true)
            .build()
            .map_err(|_| Err::Error(Error::new(input, ErrorKind::MapRes)))?;

        let bits = mask.iter().map(|b| b.count_ones()).sum::<u32>();

        Ok((
            input,
            Self {
                data,
                mask,
                norm,
                bits,
            },
        ))
    }

    pub fn is_match_at(&self, bytes: impl AsRef<[u8]>, offset: usize) -> bool {
        let bytes = bytes.as_ref();
        offset < bytes.len()
            && bytes.len() - offset >= self.data.len()
            && self
                .data
                .iter()
                .zip(bytes.iter().zip(self.mask.iter()))
                .all(|(v, (b, m))| *v == b & m)
    }

    pub fn is_match(&self, bytes: impl AsRef<[u8]>) -> bool {
        self.is_match_at(bytes, 0)
    }

    pub fn matches_start(&self, bytes: impl AsRef<[u8]>) -> bool {
        self.is_match(bytes)
    }

    pub fn matches_end(&self, bytes: impl AsRef<[u8]>) -> bool {
        let bytes = bytes.as_ref();
        if bytes.len() < self.data.len() {
            return false;
        }
        let (_, suffix) = bytes.split_at(bytes.len() - self.data.len());
        self.is_match(suffix)
    }

    pub fn normalised_matcher(&self) -> &Regex {
        &self.norm
    }

    pub fn find_iter<'a>(&'a self, bytes: &'a [u8]) -> impl Iterator<Item = Match<'a>> {
        self.normalised_matcher()
            .find_iter(bytes)
            .filter(|m| self.is_match(m.as_bytes()))
    }

    pub fn bits(&self) -> u32 {
        self.bits
    }
}

impl Display for Pattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, (v, m)) in self.data.iter().zip(self.mask.iter()).enumerate() {
            if i != 0 {
                f.write_char(' ')?;
            }
            match m {
                0x00 => f.write_str("..")?,
                0x0f => {
                    f.write_char('.')?;
                    f.write_char(HTABLE[(v & 0xf) as usize] as char)?;
                }
                0xf0 => {
                    f.write_char(HTABLE[(v >> 4) as usize] as char)?;
                    f.write_char('.')?;
                }
                0xff => {
                    f.write_char(HTABLE[(v >> 4) as usize] as char)?;
                    f.write_char(HTABLE[(v & 0xf) as usize] as char)?;
                }
                _ => {
                    for i in (0..8).rev() {
                        if m >> i & 1 == 0 {
                            f.write_char('.')?;
                        } else {
                            f.write_char(HTABLE[(v >> i & 1) as usize] as char)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl FromStr for Pattern {
    type Err = PatternError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (input, pattern) = Pattern::parse(s)
            .finish()
            .map_err(|k| PatternError::Parse(k.code))?;

        if !input.is_empty() {
            return Err(PatternError::Parse(ErrorKind::Complete));
        }

        Ok(pattern)
    }
}

impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Cow::<str>::deserialize(deserializer)?;
        Self::from_str(&*s).map_err(<D::Error as serde::de::Error>::custom)
    }
}

impl Serialize for Pattern {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

#[derive(Debug, Error)]
pub enum PatternSetError {
    #[error("cannot parse patterns: {0}")]
    Parse(serde_yaml::Error),
    #[error("cannot parse patterns from `{0}`: {1}")]
    ParseFile(PathBuf, serde_yaml::Error),
    #[error("cannot parse patterns from `{0}`: {1}")]
    ReadFile(PathBuf, io::Error),
}

#[derive(Clone)]
pub struct PatternSet {
    architecture: PatternArch,
    groups: Vec<PatternGroup>,
    patterns: Vec<PatternsWithContext>,
}

pub struct PatternSetMatchIter<'a>(
    Box<dyn Iterator<Item = (Range<usize>, &'a PatternContext)> + 'a>,
);

impl<'a> Iterator for PatternSetMatchIter<'a> {
    type Item = (Range<usize>, &'a PatternContext);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl PatternSet {
    pub fn from_str(input: impl AsRef<str>) -> Result<PatternSet, PatternSetError> {
        serde_yaml::from_str(input.as_ref()).map_err(PatternSetError::Parse)
    }

    pub fn from_reader(reader: impl Read) -> Result<PatternSet, PatternSetError> {
        serde_yaml::from_reader(reader).map_err(PatternSetError::Parse)
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<PatternSet, PatternSetError> {
        let path = path.as_ref();
        let file = BufReader::new(
            File::open(path).map_err(|e| PatternSetError::ReadFile(path.to_owned(), e))?,
        );
        serde_yaml::from_reader(file).map_err(|e| PatternSetError::ParseFile(path.to_owned(), e))
    }

    pub fn architecture(&self) -> &PatternArch {
        &self.architecture
    }

    pub fn matches<'a>(&'a self, bytes: &'a [u8]) -> PatternSetMatchIter<'a> {
        PatternSetMatchIter(Box::new(
            self.groups
                .iter()
                .flat_map(|group| group.matches(bytes))
                .chain(
                    self.patterns
                        .iter()
                        .flat_map(|pattern| pattern.matches(bytes)),
                ),
        ))
    }
}

impl<'de> Deserialize<'de> for PatternSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let ps = PatternSetT::deserialize(deserializer)?;

        Ok(Self {
            architecture: ps.architecture.into_owned(),
            groups: ps.patterns.groups.into_owned(),
            patterns: ps.patterns.patterns.into_owned(),
        })
    }
}

impl Serialize for PatternSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let t = PatternSetT {
            architecture: Cow::Borrowed(&self.architecture),
            patterns: PatternOrGroupSeq {
                groups: Cow::Borrowed(&self.groups),
                patterns: Cow::Borrowed(&self.patterns),
            },
        };

        t.serialize(serializer)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum PatternOrGroup<'a> {
    Pattern {
        pattern: Cow<'a, PatternsWithContext>,
    },
    PatternGroup {
        #[serde(rename = "pattern-group")]
        pattern_group: Cow<'a, PatternGroup>,
    },
}

#[derive(Deserialize, Serialize)]
struct PatternSetT<'a> {
    architecture: Cow<'a, PatternArch>,
    #[serde(bound(deserialize = "PatternOrGroupSeq<'a>: Deserialize<'de>"))]
    patterns: PatternOrGroupSeq<'a>,
}

#[derive(Clone)]
struct PatternOrGroupSeq<'a> {
    groups: Cow<'a, [PatternGroup]>,
    patterns: Cow<'a, [PatternsWithContext]>,
}

impl<'de> Deserialize<'de> for PatternOrGroupSeq<'_> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visit;

        impl<'de> Visitor<'de> for Visit {
            type Value = PatternOrGroupSeq<'static>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct PatternOrGroupSeq")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut groups = Vec::new();
                let mut patterns = Vec::new();

                #[derive(Deserialize)]
                #[serde(untagged)]
                enum PatternOrGroupOwned {
                    Pattern {
                        pattern: PatternsWithContext,
                    },
                    PatternGroup {
                        #[serde(rename = "pattern-group")]
                        pattern_group: PatternGroup,
                    },
                }

                while let Some(pg) = seq.next_element::<PatternOrGroupOwned>()? {
                    match pg {
                        PatternOrGroupOwned::Pattern { pattern: p } => {
                            patterns.push(p);
                        }
                        PatternOrGroupOwned::PatternGroup { pattern_group: g } => {
                            groups.push(g);
                        }
                    }
                }

                Ok(PatternOrGroupSeq {
                    groups: Cow::Owned(groups),
                    patterns: Cow::Owned(patterns),
                })
            }
        }

        deserializer.deserialize_seq(Visit)
    }
}

impl Serialize for PatternOrGroupSeq<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser = serializer.serialize_seq(Some(self.patterns.len() + self.groups.len()))?;

        for g in self.groups.iter() {
            ser.serialize_element(&PatternOrGroup::PatternGroup {
                pattern_group: Cow::Borrowed(g),
            })?;
        }

        for p in self.patterns.iter() {
            ser.serialize_element(&PatternOrGroup::Pattern {
                pattern: Cow::Borrowed(p),
            })?;
        }

        ser.end()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatternArch {
    processor: String,
    endian: Endian,
    bits: Option<u32>,
    variant: Option<String>,
    convention: Option<String>,
}

impl PatternArch {
    pub fn new(processor: impl Into<String>, endian: Endian) -> Self {
        Self::new_with(processor, endian, None, None, None)
    }

    pub fn new_with(
        processor: impl Into<String>,
        endian: Endian,
        bits: impl Into<Option<u32>>,
        variant: impl Into<Option<String>>,
        convention: impl Into<Option<String>>,
    ) -> Self {
        Self {
            processor: processor.into(),
            endian,
            bits: bits.into(),
            variant: variant.into(),
            convention: convention.into(),
        }
    }

    pub fn processor(&self) -> &str {
        &self.processor
    }

    pub fn endian(&self) -> Endian {
        self.endian
    }

    pub fn bits(&self) -> Option<u32> {
        self.bits
    }

    pub fn variant(&self) -> Option<&str> {
        self.variant.as_deref()
    }

    pub fn convention(&self) -> Option<&str> {
        self.convention.as_deref()
    }
}

impl<'de> Deserialize<'de> for PatternArch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let parts = s.splitn(5, ':').collect::<Vec<_>>();
        if parts.len() != 5 {
            return Err(<D::Error as serde::de::Error>::custom(
                "architecture format is invalid",
            ));
        }

        let processor = if parts[0] == "*" || parts[0].is_empty() {
            return Err(<D::Error as serde::de::Error>::custom(
                "invalid architecture processor (should be non-empty string and not '*')",
            ));
        } else {
            parts[0].to_owned()
        };

        let endian = match parts[1] {
            "le" | "LE" => Endian::Little,
            "be" | "BE" => Endian::Big,
            _ => {
                return Err(<D::Error as serde::de::Error>::custom(
                    "invalid architecture endian (should be LE or BE)",
                ))
            }
        };

        let bits = if parts[2] == "*" {
            None
        } else {
            match parts[2].parse::<u32>() {
                Ok(bits) => Some(bits),
                Err(_) => {
                    return Err(<D::Error as serde::de::Error>::custom(
                        "invalid architecture bits (should be numeric or '*')",
                    ))
                }
            }
        };

        let variant = if parts[3] == "*" {
            None
        } else if parts[3].is_empty() {
            return Err(<D::Error as serde::de::Error>::custom(
                "invalid architecture variant (should be non-empty string or '*')",
            ));
        } else {
            Some(parts[3].to_owned())
        };

        let convention = if parts[4] == "*" {
            None
        } else if parts[4].is_empty() {
            return Err(<D::Error as serde::de::Error>::custom(
                "invalid architecture convention (should be non-empty string or '*')",
            ));
        } else {
            Some(parts[4].to_owned())
        };

        Ok(PatternArch {
            processor,
            endian,
            bits,
            variant,
            convention,
        })
    }
}

impl Serialize for PatternArch {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let endian = match self.endian {
            Endian::Big => "BE",
            Endian::Little => "LE",
        };

        let bits = self
            .bits
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "*".to_owned());

        let variant = self.variant.as_deref().unwrap_or("*");
        let convention = self.convention.as_deref().unwrap_or("*");

        let parts = format!(
            "{}:{}:{}:{}:{}",
            self.processor, endian, bits, variant, convention
        );

        serializer.serialize_str(&parts)
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct PatternGroup {
    #[serde(rename = "total-bits")]
    total_bits: u32,
    #[serde(rename = "post-bits")]
    post_bits: u32,
    #[serde(rename = "post")]
    post_patterns: PatternsWithContext,
    #[serde(rename = "pre")]
    pre_patterns: Vec<Pattern>,
}

impl PatternGroup {
    pub fn matches<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> impl Iterator<Item = (Range<usize>, &'a PatternContext)> + 'a {
        let bytes = bytes.as_ref();

        self.post_patterns.patterns.iter().flat_map(|pattern| {
            pattern
                .normalised_matcher()
                .find_iter(bytes)
                .filter_map(|m| {
                    // impossible to match pre-pattern in this case
                    if m.start() == 0 {
                        return None;
                    }

                    let pre_bytes = &bytes[..m.start()];
                    let post_bits = pattern.bits();

                    if pattern.matches_start(m.as_bytes())
                        && self.pre_patterns.iter().any(|pre_pattern| {
                            // matches pre-pattern, and also matches at least total bits between
                            // pre- and post-patterns
                            pre_pattern.matches_end(pre_bytes)
                                && pre_pattern.bits() + post_bits >= self.total_bits
                        })
                    {
                        Some((m.range(), &self.post_patterns.context))
                    } else {
                        None
                    }
                })
        })
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct PatternsWithContext {
    patterns: Vec<Pattern>,
    #[serde(default)]
    context: PatternContext,
}

impl PatternsWithContext {
    pub fn matches<'a>(
        &'a self,
        bytes: &'a [u8],
    ) -> impl Iterator<Item = (Range<usize>, &'a PatternContext)> + 'a {
        let bytes = bytes.as_ref();

        self.patterns.iter().flat_map(|pattern| {
            pattern
                .normalised_matcher()
                .find_iter(bytes)
                .filter_map(|m| {
                    if pattern.is_match(m.as_bytes()) {
                        Some((m.range(), &self.context))
                    } else {
                        None
                    }
                })
        })
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct PatternContextItem<'a> {
    name: Cow<'a, String>,
    value: u32,
}

#[derive(Clone, Default)]
#[repr(transparent)]
pub struct PatternContext(BTreeMap<String, u32>);

impl PatternContext {
    pub fn variables(&self) -> impl Iterator<Item = (&str, u32)> {
        self.0.iter().map(|(name, value)| (&**name, *value))
    }
}

impl<'de> Deserialize<'de> for PatternContext {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visit;

        impl<'de> Visitor<'de> for Visit {
            type Value = PatternContext;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct PatternContext")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut context = BTreeMap::new();

                #[derive(Deserialize)]
                struct PatternContextItemOwned {
                    name: String,
                    value: u32,
                }

                while let Some(PatternContextItemOwned { name, value }) = seq.next_element()? {
                    context.insert(name, value);
                }

                Ok(PatternContext(context))
            }
        }

        deserializer.deserialize_seq(Visit)
    }
}

impl Serialize for PatternContext {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for (name, &value) in self.0.iter() {
            seq.serialize_element(&PatternContextItem {
                name: Cow::Borrowed(name),
                value,
            })?;
        }
        seq.end()
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;

    const PAT_GROUP: &'static str = r#"
post:
  context:
  - name: TMode
    value: 1
  patterns:
  - .. b5 1....... b0
  - .. b5 00...... 1c
  - .. b5 .. 46
  - .. b5 .. 01.01...
  - .. b5 .. 68
  - .. b5 .. 01.01... 10...... b0
  - 1....... b5 .. af
  - 100..... b0 .0 b5
  - 00...... 1c .0 b5
  - .. 01.01... .0 b5
  - .. 68 .0 b5
  - 2d e9 .. 0.
  - 4d f8 04 ed
post-bits: 16
pre:
  - '.......0 bd'
  - '.......0 bd 00 00'
  - '.......0 bd 00 bf'
  - '.......0 bd c0 46'
  - ff ff
  - c0 46
  - 70 47
  - 70 47 00 00
  - 70 47 c0 46
  - 70 47 00 bf
  - 000..... b0 .0 bd
  - 00 bf
  - af f3 00 80
  - bd e8 .. 0.
  - 46 f7
  - 5d f8 0....... fb
  - 5d f8 04 fb
  - bd e8 .. 100.....
total-bits: 32
"#;

    const PAT_WITH_CTX: &'static str = r#"
context:
- name: TMode
  value: 1
patterns:
- .. b5 1....... b0
- .. b5 00...... 1c
- .. b5 .. 46
- .. b5 .. 01.01...
- .. b5 .. 68
- .. b5 .. 01.01... 10...... b0
- 1....... b5 .. af
- 100..... b0 .0 b5
- 00...... 1c .0 b5
- .. 01.01... .0 b5
- .. 68 .0 b5
- 2d e9 .. 0.
- 4d f8 04 ed
"#;

    const PATS_WITH_CTX: &'static str = r#"
- pattern:
    context:
    - name: TMode
      value: 1
    patterns:
    - .. b5 1....... b0
    - .. b5 00...... 1c
    - .. b5 .. 46
    - .. b5 .. 01.01...
    - .. b5 .. 68
    - .. b5 .. 01.01... 10...... b0
    - 1....... b5 .. af
    - 100..... b0 .0 b5
    - 00...... 1c .0 b5
    - .. 01.01... .0 b5
    - .. 68 .0 b5
    - 2d e9 .. 0.
    - 4d f8 04 ed
- pattern-group:
    post:
      context:
      - name: TMode
        value: 1
      patterns:
      - .. b5 1....... b0
      - .. b5 00...... 1c
      - .. b5 .. 46
      - .. b5 .. 01.01...
      - .. b5 .. 68
      - .. b5 .. 01.01... 10...... b0
      - 1....... b5 .. af
      - 100..... b0 .0 b5
      - 00...... 1c .0 b5
      - .. 01.01... .0 b5
      - .. 68 .0 b5
      - 2d e9 .. 0.
      - 4d f8 04 ed
    post-bits: 16
    pre:
      - '.......0 bd'
      - '.......0 bd 00 00'
      - '.......0 bd 00 bf'
      - '.......0 bd c0 46'
      - ff ff
      - c0 46
      - 70 47
      - 70 47 00 00
      - 70 47 c0 46
      - 70 47 00 bf
      - 000..... b0 .0 bd
      - 00 bf
      - af f3 00 80
      - bd e8 .. 0.
      - 46 f7
      - 5d f8 0....... fb
      - 5d f8 04 fb
      - bd e8 .. 100.....
    total-bits: 32
"#;

    const PAT: &'static str = r#"
architecture: ARM:LE:32:*:*
patterns:
- pattern-group:
    post:
      context:
      - name: TMode
        value: 1
      patterns:
      - .. b5 1....... b0
      - .. b5 00...... 1c
      - .. b5 .. 46
      - .. b5 .. 01.01...
      - .. b5 .. 68
      - .. b5 .. 01.01... 10...... b0
      - 1....... b5 .. af
      - 100..... b0 .0 b5
      - 00...... 1c .0 b5
      - .. 01.01... .0 b5
      - .. 68 .0 b5
      - 2d e9 .. 0.
      - 4d f8 04 ed
    post-bits: 16
    pre:
      - '.......0 bd'
      - '.......0 bd 00 00'
      - '.......0 bd 00 bf'
      - '.......0 bd c0 46'
      - ff ff
      - c0 46
      - 70 47
      - 70 47 00 00
      - 70 47 c0 46
      - 70 47 00 bf
      - 000..... b0 .0 bd
      - 00 bf
      - af f3 00 80
      - bd e8 .. 0.
      - 46 f7
      - 5d f8 0....... fb
      - 5d f8 04 fb
      - bd e8 .. 100.....
    total-bits: 32
- pattern:
    context:
    - name: TMode
      value: 0
    patterns:
    - .. 0. 8f e2 .. 0. 8c e2 .. 0. bc e5
- pattern:
    context:
    - name: TMode
      value: 1
    patterns:
    - 03 b4 01 48 01 90 01 bd
- pattern:
    context:
    - name: TMode
      value: 1
    patterns:
    - 10 b5
"#;

    #[test]
    fn test_yaml_parse() -> anyhow::Result<()> {
        let _ = serde_yaml::from_str::<PatternGroup>(PAT_GROUP)?;
        let _ = serde_yaml::from_str::<PatternsWithContext>(PAT_WITH_CTX)?;
        let _ = serde_yaml::from_str::<PatternOrGroupSeq>(PATS_WITH_CTX)?;

        let _ = serde_yaml::from_reader::<_, PatternSet>(Cursor::new(PAT))?;
        let v = serde_yaml::from_str::<PatternSet>(PAT)?;
        let _ = serde_yaml::to_string(&v)?;

        Ok(())
    }

    #[test]
    fn test_pattern_parse() -> anyhow::Result<()> {
        let (_, pattern) = Pattern::parse("  .f 0. 000.011. ")?;

        assert_eq!(pattern.data, b"\x0f\x00\x06");
        assert_eq!(pattern.mask, b"\x0f\xf0\xee");

        // anything will match the regex
        assert!(pattern.norm.is_match(b"\x1f\x01\xff"));
        assert!(pattern.norm.is_match(b"\x10\x11\x00"));

        // exact matches
        assert!(pattern.is_match(b"\x1f\x01\x17"));
        assert!(pattern.is_match(b"\x0f\x00\x06"));
        assert!(!pattern.is_match(b"\x0f\x00\x08"));

        Ok(())
    }

    #[test]
    fn test_pattern_bundled_arm_le_32() -> anyhow::Result<()> {
        let i = b"\x0d\xc0\xa0\xe1\x00\x00\x2d\xe9\x00";
        let m = ARM_LE_32.matches(i);

        assert_eq!(m.count(), 2);

        Ok(())
    }
}
