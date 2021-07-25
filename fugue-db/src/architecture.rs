use crate::error::Error;
use crate::schema;

pub use fugue_arch::ArchitectureDef;
pub use fugue_bytes::endian::Endian;

pub(crate) fn from_reader(reader: &schema::Architecture) -> Result<ArchitectureDef, Error> {
    let name = reader.processor().ok_or(Error::DeserialiseField("processor"))?.to_string();
    let endian = Endian::from(if reader.endian() { Endian::Big } else { Endian::Little });
    let bits = reader.bits() as usize;
    let variant = reader.variant().ok_or(Error::DeserialiseField("variant"))?.to_string();
    Ok(ArchitectureDef::new(name, endian, bits, variant))
}

pub(crate) fn to_builder<'a: 'b, 'b>(
    arch: &ArchitectureDef,
    builder: &'b mut flatbuffers::FlatBufferBuilder<'a>
) -> Result<flatbuffers::WIPOffset<schema::Architecture<'a>>, Error> {

    let processor = builder.create_string(arch.processor());
    let variant = builder.create_string(arch.variant());

    let mut abuilder = schema::ArchitectureBuilder::new(builder);

    abuilder.add_processor(processor);
    abuilder.add_endian(arch.is_big());
    abuilder.add_bits(arch.bits() as u32);
    abuilder.add_variant(variant);

    Ok(abuilder.finish())
}
