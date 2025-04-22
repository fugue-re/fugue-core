use std::num::ParseIntError;
use std::path::PathBuf;
use std::str::{ParseBoolError, Utf8Error};

use fugue_bytes::Endian;
use fugue_ghidra_marshal::MarshalError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DeserialiseError {
    #[error("attribute `{0}` expected")]
    AttributeExpected(&'static str),
    #[error(transparent)]
    Decoder(#[from] MarshalError),
    #[error("cannot deserialise dependency `{}`: {}", path.display(), error)]
    DeserialiseDepends {
        path: PathBuf,
        error: Box<crate::language::LanguageError>,
    },
    #[error("unexpected element `{0}`")]
    ElementUnexpected(u32),
    #[error("invariant not satisfied: {0}")]
    Invariant(&'static str),
    #[error("could not parse boolean: {0}")]
    ParseBool(#[from] ParseBoolError),
    #[error("could not parse integer: {0}")]
    ParseInteger(#[from] ParseIntError),
    #[error("could not parse endian")]
    ParseEndian,
    #[error("unexpected tag `{0}`")]
    TagUnexpected(String),
    #[error("expected UTF-8 encoded input: {0}")]
    Utf8Expected(#[from] Utf8Error),
    #[error(transparent)]
    Xml(#[from] xml::Error),
}

pub trait XmlExt {
    fn attribute_endian(&self, name: &'static str) -> Result<Endian, DeserialiseError>;

    fn attribute_processor(&self, name: &'static str) -> Result<String, DeserialiseError> {
        self.attribute_string(name)
    }

    fn attribute_variant(&self, name: &'static str) -> Result<String, DeserialiseError> {
        self.attribute_string(name)
    }

    fn attribute_string(&self, name: &'static str) -> Result<String, DeserialiseError>;

    fn attribute_string_or(
        &self,
        name1: &'static str,
        name2: &'static str,
    ) -> Result<String, DeserialiseError>;

    fn attribute_string_opt(&self, name: &'static str, default: &str) -> String;

    fn attribute_int<T: FromStrRadix>(&self, name: &'static str) -> Result<T, DeserialiseError>;

    fn attribute_int_or<T: FromStrRadix>(
        &self,
        name1: &'static str,
        name2: &'static str,
    ) -> Result<T, DeserialiseError>;

    fn attribute_line_number<T: Default + FromStrRadix>(
        &self,
        name: &'static str,
    ) -> Result<(T, T), DeserialiseError>;

    fn attribute_int_opt<T: FromStrRadix>(
        &self,
        name: &'static str,
        default: T,
    ) -> Result<T, DeserialiseError>;

    fn attribute_bool(&self, name: &'static str) -> Result<bool, DeserialiseError>;
}

#[inline(always)]
fn parse_int_radix<T: FromStrRadix>(s: &str) -> Result<T, DeserialiseError> {
    let b = s.as_bytes();
    if b.len() > 2 && b[0] == b'0' && (b[1] == b'X' || b[1] == b'x') {
        T::from_str_base(&s[2..], 16)
    } else {
        T::from_str_base(s, 10)
    }
}

pub trait FromStrRadix: Sized {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, DeserialiseError>;
}

impl FromStrRadix for i8 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, DeserialiseError> {
        Self::from_str_radix(s, radix).map_err(DeserialiseError::ParseInteger)
    }
}

impl FromStrRadix for i16 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, DeserialiseError> {
        Self::from_str_radix(s, radix).map_err(DeserialiseError::ParseInteger)
    }
}

impl FromStrRadix for i32 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, DeserialiseError> {
        Self::from_str_radix(s, radix).map_err(DeserialiseError::ParseInteger)
    }
}

impl FromStrRadix for i64 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, DeserialiseError> {
        Self::from_str_radix(s, radix).map_err(DeserialiseError::ParseInteger)
    }
}

impl FromStrRadix for isize {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, DeserialiseError> {
        Self::from_str_radix(s, radix).map_err(DeserialiseError::ParseInteger)
    }
}

impl FromStrRadix for u8 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, DeserialiseError> {
        Self::from_str_radix(s, radix).map_err(DeserialiseError::ParseInteger)
    }
}

impl FromStrRadix for u16 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, DeserialiseError> {
        Self::from_str_radix(s, radix).map_err(DeserialiseError::ParseInteger)
    }
}

impl FromStrRadix for u32 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, DeserialiseError> {
        Self::from_str_radix(s, radix).map_err(DeserialiseError::ParseInteger)
    }
}

impl FromStrRadix for u64 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, DeserialiseError> {
        Self::from_str_radix(s, radix).map_err(DeserialiseError::ParseInteger)
    }
}

impl FromStrRadix for usize {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, DeserialiseError> {
        Self::from_str_radix(s, radix).map_err(DeserialiseError::ParseInteger)
    }
}

impl XmlExt for xml::Node<'_, '_> {
    fn attribute_endian(&self, name: &'static str) -> Result<Endian, DeserialiseError> {
        let n = self
            .attribute(name)
            .ok_or_else(|| DeserialiseError::AttributeExpected(name))?;
        match n {
            "big" | "BIG" | "be" | "BE" => Ok(Endian::Big),
            "little" | "LITTLE" | "le" | "LE" => Ok(Endian::Little),
            _ => Err(DeserialiseError::ParseEndian),
        }
    }

    fn attribute_string(&self, name: &'static str) -> Result<String, DeserialiseError> {
        self.attribute(name)
            .map(String::from)
            .ok_or_else(|| DeserialiseError::AttributeExpected(name))
    }

    fn attribute_string_or(
        &self,
        name1: &'static str,
        name2: &'static str,
    ) -> Result<String, DeserialiseError> {
        self.attribute(name1)
            .or_else(|| self.attribute(name2))
            .map(String::from)
            .ok_or_else(|| DeserialiseError::AttributeExpected(name1))
    }

    fn attribute_string_opt(&self, name: &'static str, default: &str) -> String {
        self.attribute(name)
            .map(String::from)
            .unwrap_or_else(|| default.to_owned())
    }

    fn attribute_line_number<T: Default + FromStrRadix>(
        &self,
        name: &'static str,
    ) -> Result<(T, T), DeserialiseError> {
        let s = self
            .attribute(name)
            .ok_or_else(|| DeserialiseError::AttributeExpected(name))?;

        let b = s.as_bytes();
        if let Some(pos) = b.iter().position(|v| *v == b':') {
            // Two part index:line
            let index = parse_int_radix(&s[..pos])?;
            let line = parse_int_radix(&s[pos + 1..])?;
            Ok((index, line))
        } else {
            // One part 0:line
            let index = T::default();
            let line = parse_int_radix(s)?;
            Ok((index, line))
        }
    }

    fn attribute_int<T: FromStrRadix>(&self, name: &'static str) -> Result<T, DeserialiseError> {
        let s = self
            .attribute(name)
            .ok_or_else(|| DeserialiseError::AttributeExpected(name))?;
        parse_int_radix(s)
    }

    fn attribute_int_or<T: FromStrRadix>(
        &self,
        name1: &'static str,
        name2: &'static str,
    ) -> Result<T, DeserialiseError> {
        let s = self
            .attribute(name1)
            .or_else(|| self.attribute(name2))
            .ok_or_else(|| DeserialiseError::AttributeExpected(name1))?;
        parse_int_radix(s)
    }

    fn attribute_int_opt<T: FromStrRadix>(
        &self,
        name: &'static str,
        default: T,
    ) -> Result<T, DeserialiseError> {
        if let Some(s) = self.attribute(name) {
            parse_int_radix(s)
        } else {
            Ok(default)
        }
    }

    fn attribute_bool(&self, name: &'static str) -> Result<bool, DeserialiseError> {
        self.attribute(name)
            .ok_or_else(|| DeserialiseError::AttributeExpected(name))?
            .parse::<bool>()
            .map_err(DeserialiseError::ParseBool)
    }
}
