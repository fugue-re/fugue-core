use crate::error::Error;
use crate::schema;

use fugue_bytes::Endian;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Segment {
    name: String,
    address: u64,
    length: usize,
    alignment: usize,
    address_size: usize,
    endian: Endian,
    bits: usize,
    code: bool,
    data: bool,
    external: bool,
    executable: bool,
    readable: bool,
    writable: bool,
    bytes: Vec<u8>,
}

impl Segment {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn address(&self) -> u64 {
        self.address
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn alignment(&self) -> usize {
        self.alignment
    }

    pub fn address_size(&self) -> usize {
        self.address_size
    }

    pub fn endian(&self) -> Endian {
        self.endian
    }

    pub fn bits(&self) -> usize {
        self.bits
    }

    pub fn is_code(&self) -> bool {
        self.code
    }

    pub fn is_data(&self) -> bool {
        self.data
    }

    pub fn is_external(&self) -> bool {
        self.external
    }

    pub fn is_executable(&self) -> bool {
        self.executable
    }

    pub fn is_readable(&self) -> bool {
        self.readable
    }

    pub fn is_writable(&self) -> bool {
        self.writable
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn from_reader(reader: &schema::Segment) -> Result<Self, Error> {
        Ok(Self {
            name: reader.name().ok_or(Error::DeserialiseField("name"))?.to_string(),
            address: reader.address(),
            length: reader.size_() as usize,
            alignment: reader.alignment_() as usize,
            endian: if reader.endian() { Endian::Big } else { Endian::Little },
            bits: reader.bits() as usize,
            address_size: reader.address_size() as usize,
            code: reader.code(),
            data: reader.data(),
            external: reader.external(),
            executable: reader.executable(),
            readable: reader.readable(),
            writable: reader.writable(),
            bytes: reader.bytes().ok_or(Error::DeserialiseField("bytes"))?.to_vec(),
        })
    }

    pub(crate) fn to_builder<'a: 'b, 'b>(
        &self,
        builder: &'b mut flatbuffers::FlatBufferBuilder<'a>
    ) -> Result<flatbuffers::WIPOffset<schema::Segment<'a>>, Error> {
        let name = builder.create_string(self.name());
        let bytes = builder.create_vector_direct(&self.bytes);

        let mut sbuilder = schema::SegmentBuilder::new(builder);

        sbuilder.add_name(name);
        sbuilder.add_address(self.address());
        sbuilder.add_size_(self.len() as u32);
        sbuilder.add_alignment_(self.alignment() as u32);
        sbuilder.add_address_size(self.address_size() as u32);
        sbuilder.add_endian(self.endian.is_big());
        sbuilder.add_bits(self.bits as u32);
        sbuilder.add_code(self.is_code());
        sbuilder.add_data(self.is_data());
        sbuilder.add_external(self.is_external());
        sbuilder.add_executable(self.is_executable());
        sbuilder.add_readable(self.is_readable());
        sbuilder.add_writable(self.is_writable());
        sbuilder.add_bytes(bytes);

        Ok(sbuilder.finish())
    }
}
