use std::convert::TryFrom;
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum Format {
    Raw,
    PE,
    ELF,
    TE,
    MachO,
    Other,
}

impl Format {
    pub fn is_elf(&self) -> bool {
        *self == Self::ELF
    }

    pub fn is_pe(&self) -> bool {
        *self == Self::PE
    }

    pub fn is_raw(&self) -> bool {
        *self == Self::Raw
    }

    pub fn is_macho(&self) -> bool {
        *self == Self::MachO
    }

    pub fn is_te(&self) -> bool {
        *self == Self::TE
    }

    pub fn is_other(&self) -> bool {
        *self == Self::Other
    }
}

impl TryFrom<&str> for Format {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Error> {
        match value {
            "Raw" => Ok(Self::Raw),
            "PE" => Ok(Self::PE),
            "ELF" => Ok(Self::ELF),
            "Mach-O" => Ok(Self::MachO),
            "TE" => Ok(Self::TE),
            "Other" => Ok(Self::Other),
            _ => Err(Error::UnsupportedFormat(value.to_string())),
        }
    }
}

impl Into<&'static str> for Format {
    fn into(self) -> &'static str {
        match self {
            Self::Raw => "Raw",
            Self::PE => "PE",
            Self::ELF => "ELF",
            Self::MachO => "Mach-O",
            Self::Other => "Other",
            Self::TE => "TE",
         }
    }
}
