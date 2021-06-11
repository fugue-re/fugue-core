use crate::error::Error;
use crate::schema;

pub use fugue_arch::ArchitectureDef;
pub use fugue_bytes::endian::Endian;

pub(crate) fn from_reader(reader: schema::architecture::Reader) -> Result<ArchitectureDef, Error> {
    let name = reader.get_processor().map_err(Error::Deserialisation)?.to_string();
    let endian = Endian::from(if reader.get_endian() { Endian::Big } else { Endian::Little });
    let bits = reader.get_bits() as usize;
    let variant = reader.get_variant().map_err(Error::Deserialisation)?.to_string();
    Ok(ArchitectureDef::new(name, endian, bits, variant))
}

pub(crate) fn to_builder(arch: &ArchitectureDef, builder: &mut schema::architecture::Builder) -> Result<(), Error> {
    builder.set_processor(arch.processor());
    builder.set_endian(arch.is_big());
    builder.set_bits(arch.bits() as u32);
    builder.set_variant(arch.variant());
    Ok(())
}
