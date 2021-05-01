use crate::endian::Endian;
use crate::language::{Processor, Variant};

use super::Error;

pub trait XmlExt {
    fn attribute_endian(
        &self,
        name: &'static str,
    ) -> Result<Endian, Error>;

    fn attribute_processor(
        &self,
        name: &'static str,
    ) -> Result<Processor, Error>;

    fn attribute_variant(
        &self,
        name: &'static str,
    ) -> Result<Variant, Error>;

    fn attribute_string(
        &self,
        name: &'static str,
    ) -> Result<String, Error>;

    fn attribute_string_opt(
        &self,
        name: &'static str,
        default: &str,
    ) -> String;

    fn attribute_int<T: FromStrRadix>(
        &self,
        name: &'static str,
    ) -> Result<T, Error>;

    fn attribute_int_opt<T: FromStrRadix>(
        &self,
        name: &'static str,
        default: T,
    ) -> Result<T, Error>;

    fn attribute_bool(
        &self,
        name: &'static str,
    ) -> Result<bool, Error>;
}

pub trait FromStrRadix: Sized {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, Error>;
}

impl FromStrRadix for i8 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, Error> {
        Self::from_str_radix(s, radix).map_err(Error::ParseInteger)
    }
}

impl FromStrRadix for i16 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, Error> {
        Self::from_str_radix(s, radix).map_err(Error::ParseInteger)
    }
}

impl FromStrRadix for i32 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, Error> {
        Self::from_str_radix(s, radix).map_err(Error::ParseInteger)
    }
}

impl FromStrRadix for i64 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, Error> {
        Self::from_str_radix(s, radix).map_err(Error::ParseInteger)
    }
}

impl FromStrRadix for isize {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, Error> {
        Self::from_str_radix(s, radix).map_err(Error::ParseInteger)
    }
}

impl FromStrRadix for u8 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, Error> {
        Self::from_str_radix(s, radix).map_err(Error::ParseInteger)
    }
}

impl FromStrRadix for u16 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, Error> {
        Self::from_str_radix(s, radix).map_err(Error::ParseInteger)
    }
}

impl FromStrRadix for u32 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, Error> {
        Self::from_str_radix(s, radix).map_err(Error::ParseInteger)
    }
}

impl FromStrRadix for u64 {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, Error> {
        Self::from_str_radix(s, radix).map_err(Error::ParseInteger)
    }
}

impl FromStrRadix for usize {
    fn from_str_base(s: &str, radix: u32) -> Result<Self, Error> {
        Self::from_str_radix(s, radix).map_err(Error::ParseInteger)
    }
}

impl XmlExt for xml::Node<'_, '_> {
    fn attribute_endian(&self, name: &'static str) -> Result<Endian, Error> {
        let n = self.attribute(name)
            .map_err(|error| AttributeExpected { name })?;
        match n {
            "big" | "BIG" | "be" | "BE" => Ok(Endian::Big),
            "little" | "LITTLE" | "le" | "LE" => Ok(Endian::Little),
            _ => Err(Error::ParseEndian),
        }
    }

    fn attribute_processor(&self, name: &'static str) -> Result<Processor, Error> {
        let n = self.attribute(name)
            .ok_or_else(|| Error::AttributeExpected(name))?;
        Ok(n.into())
    }

    fn attribute_variant(&self, name: &'static str) -> Result<Variant, Error> {
        let n = self.attribute(name)
            .ok_or_else(|| Error::AttributeExpected(name))?;
        Ok(n.into())
    }

    fn attribute_string(
        &self,
        name: &'static str,
    ) -> Result<String, Error> {
        self.attribute(name)
            .map(String::from)
            .ok_or_else(|| Error::AttributeExpected(name))
    }

    fn attribute_string_opt(
        &self,
        name: &'static str,
        default: &str,
    ) -> String {
        self.attribute(name)
            .map(String::from)
            .unwrap_or_else(|| default.to_owned())
    }

    fn attribute_int<T: FromStrRadix>(
        &self,
        name: &'static str,
    ) -> Result<T, Error> {
        let s = self.attribute(name)
            .ok_or_else(|| Error::AttributeExpected(name))?;
        if s.starts_with("0x") || s.starts_with("0X") {
            T::from_str_base(&s[2..], 16)
        } else {
            T::from_str_base(s, 10)
        }
    }

    fn attribute_int_opt<T: FromStrRadix>(
        &self,
        name: &'static str,
        default: T,
    ) -> Result<T, Error> {
        if let Some(s) = self.attribute(name) {
            if s.starts_with("0x") || s.starts_with("0X") {
                T::from_str_base(&s[2..], 16)
            } else {
                T::from_str_base(s, 10)
            }
        } else {
            Ok(default)
        }
    }

    fn attribute_bool(
        &self,
        name: &'static str,
    ) -> Result<bool, Error> {
        self.attribute(name)
            .ok_or_else(|| Error::AttributeExpected(name))?
            .parse::<bool>()
            .map_err(Error::ParseBool)
    }
}
