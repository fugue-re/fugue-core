use crate::error::Error;
use crate::schema;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Segment {
    name: String,
    address: u64,
    length: usize,
    alignment: usize,
    address_size: usize,
    code: bool,
    data: bool,
    external: bool,
    executable: bool,
    readable: bool,
    writable: bool,
    content: Vec<u8>,
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
        &self.content
    }

    pub(crate) fn from_reader(reader: schema::segment::Reader) -> Result<Self, Error> {
        Ok(Self {
            name: reader.get_name().map_err(Error::Deserialisation)?.to_string(),
            address: reader.get_address(),
            length: reader.get_length() as usize,
            alignment: reader.get_alignment() as usize,
            address_size: reader.get_address_size() as usize,
            code: reader.get_code(),
            data: reader.get_data(),
            external: reader.get_external(),
            executable: reader.get_executable(),
            readable: reader.get_readable(),
            writable: reader.get_writable(),
            content: reader.get_content().map_err(Error::Deserialisation)?.to_vec(),
        })
    }

    pub(crate) fn to_builder(&self, builder: &mut schema::segment::Builder) -> Result<(), Error> {
        builder.set_name(self.name());
        builder.set_address(self.address());
        builder.set_length(self.len() as u32);
        builder.set_alignment(self.alignment() as u32);
        builder.set_address_size(self.address_size() as u32);
        builder.set_code(self.is_code());
        builder.set_data(self.is_data());
        builder.set_external(self.is_external());
        builder.set_executable(self.is_executable());
        builder.set_readable(self.is_readable());
        builder.set_writable(self.is_writable());
        let content = builder.reborrow().init_content(self.content.len() as u32);
        content.copy_from_slice(&self.content);
        Ok(())
    }
}
