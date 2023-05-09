use std::convert::TryInto;
use std::path::Path;

use crate::error::Error;
use crate::format::Format;
use crate::schema;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Metadata {
    input_path: String,
    input_md5: [u8; 16],
    input_sha256: [u8; 32],
    input_format: Format,
    file_size: u32,
    exporter: String,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            input_path: Default::default(),
            input_md5: Default::default(),
            input_sha256: Default::default(),
            input_format: Format::Raw,
            file_size: Default::default(),
            exporter: Default::default(),
        }
    }
}

impl Metadata {
    pub fn input_path(&self) -> &Path {
        self.input_path.as_ref()
    }

    pub fn input_md5(&self) -> [u8; 16] {
        self.input_md5.clone()
    }

    pub fn input_sha256(&self) -> [u8; 32] {
        self.input_sha256.clone()
    }

    pub fn input_format(&self) -> Format {
        self.input_format
    }

    pub fn file_size(&self) -> u32 {
        self.file_size
    }

    pub fn exporter(&self) -> &str {
        &self.exporter
    }

    pub(crate) fn from_reader(reader: schema::Metadata) -> Result<Self, Error> {
        let mut input_md5 = [0u8; 16];
        let mut input_sha256 = [0u8; 32];
        input_md5.copy_from_slice(reader.input_md5().ok_or(Error::DeserialiseField("input_md5"))?.bytes());
        input_sha256.copy_from_slice(reader.input_sha256().ok_or(Error::DeserialiseField("input_sha256"))?.bytes());

        Ok(Self {
            input_path: reader.input_path().ok_or(Error::DeserialiseField("input_path"))?.to_string(),
            input_format: reader.input_format().ok_or(Error::DeserialiseField("input_format"))?.try_into()?,
            input_md5,
            input_sha256,
            file_size: reader.input_size(),
            exporter: reader.exporter().ok_or(Error::DeserialiseField("exporter"))?.to_string(),
        })
    }

    pub(crate) fn to_builder<'a: 'b, 'b>(
        &self,
        builder: &'b mut flatbuffers::FlatBufferBuilder<'a>
    ) -> Result<flatbuffers::WIPOffset<schema::Metadata<'a>>, Error> {
        let input_path = builder.create_string(&self.input_path);
        let input_md5 = builder.create_vector(&self.input_md5[..]);
        let input_sha256 = builder.create_vector(&self.input_sha256[..]);
        let input_format = builder.create_string(self.input_format.into());
        let exporter = builder.create_string(self.exporter());

        let mut mbuilder = schema::MetadataBuilder::new(builder);

        mbuilder.add_input_path(input_path);
        mbuilder.add_input_md5(input_md5);
        mbuilder.add_input_sha256(input_sha256);
        mbuilder.add_input_format(input_format);
        mbuilder.add_input_size(self.file_size);
        mbuilder.add_exporter(exporter);

        Ok(mbuilder.finish())
    }
}
