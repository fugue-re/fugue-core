use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use crate::error::Error;
use crate::schema;

use fugue_ir::endian::Endian;

#[derive(Debug, Clone)]
pub struct Architecture {
    name: String,
    endian: Endian,
    bits: usize,
    variant: String,
}

impl PartialEq for Architecture {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.endian == other.endian
            && self.bits == other.bits
            && self.variant == other.variant
    }
}

impl Eq for Architecture {}

impl PartialOrd for Architecture {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        (self.name(), self.endian(), self.bits(), self.variant()).partial_cmp(&(
            other.name(),
            other.endian(),
            other.bits(),
            other.variant(),
        ))
    }
}

impl Ord for Architecture {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Hash for Architecture {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.endian.hash(state);
        self.bits.hash(state);
        self.variant.hash(state);
    }
}

impl Architecture {
    pub fn is_little_endian(&self) -> bool {
        self.endian.is_little()
    }

    pub fn is_big_endian(&self) -> bool {
        self.endian.is_big()
    }

    pub fn endian(&self) -> Endian {
        self.endian
    }

    pub fn bits(&self) -> usize {
        self.bits
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn variant(&self) -> &str {
        &self.variant
    }

    pub(crate) fn from_reader(reader: schema::architecture::Reader) -> Result<Self, Error> {
        let name = reader.get_name().map_err(Error::Deserialisation)?.to_string();
        let endian = Endian::from(if reader.get_endian() { Endian::Big } else { Endian::Little });
        let bits = reader.get_bits() as usize;
        let variant = reader.get_variant().map_err(Error::Deserialisation)?.to_string();
        Ok(Self {
            name,
            endian,
            bits,
            variant,
        })
    }

    pub(crate) fn to_builder(
        &self,
        builder: &mut schema::architecture::Builder,
    ) -> Result<(), Error> {
        builder.set_name(self.name());
        builder.set_endian(self.is_big_endian());
        builder.set_bits(self.bits() as u32);
        builder.set_variant(self.variant());
        Ok(())
    }
}
