use std::path::Path;

use chrono::{DateTime, Utc, TimeZone};

use crate::error::Error;
use crate::schema;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExportInfo {
    input_path: String,
    input_md5: [u8; 16],
    input_sha256: [u8; 32],
    file_size: u64,
    start_time: DateTime<Utc>,
    export_time: DateTime<Utc>,
    finish_time: DateTime<Utc>,
    exporter: String,
}

impl Default for ExportInfo {
    fn default() -> Self {
        Self {
            input_path: Default::default(),
            input_md5: Default::default(),
            input_sha256: Default::default(),
            file_size: Default::default(),
            start_time: Utc::now(),
            export_time: Utc::now(),
            finish_time: Utc::now(),
            exporter: Default::default(),
        }
    }
}

impl ExportInfo {
    pub fn input_path(&self) -> &Path {
        self.input_path.as_ref()
    }

    pub fn input_md5(&self) -> [u8; 16] {
        self.input_md5.clone()
    }

    pub fn input_sha256(&self) -> [u8; 32] {
        self.input_sha256.clone()
    }

    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    pub fn start_time(&self) -> &DateTime<Utc> {
        &self.start_time
    }

    pub fn export_time(&self) -> &DateTime<Utc> {
        &self.export_time
    }

    pub fn exporter(&self) -> &str {
        &self.exporter
    }

    pub(crate) fn from_reader(reader: schema::export_info::Reader) -> Result<Self, Error> {
        let mut input_md5 = [0u8; 16];
        let mut input_sha256 = [0u8; 32];
        input_md5.copy_from_slice(reader.get_input_md5().map_err(Error::Deserialisation)?);
        input_sha256.copy_from_slice(reader.get_input_sha256().map_err(Error::Deserialisation)?);
        Ok(Self {
            input_path: reader.get_input_path().map_err(Error::Deserialisation)?.to_string(),
            input_md5,
            input_sha256,
            file_size: reader.get_file_size(),
            start_time: Utc.timestamp_nanos(reader.get_start_time() as i64),
            export_time: Utc.timestamp_nanos(reader.get_export_time() as i64),
            finish_time: Utc.timestamp_nanos(reader.get_finish_time() as i64),
            exporter: reader.get_exporter().map_err(Error::Deserialisation)?.to_string(),
        })
    }

    pub(crate) fn to_builder(&self, builder: &mut schema::export_info::Builder) -> Result<(), Error> {
        builder.set_input_path(&self.input_path);
        builder.set_input_md5(&self.input_md5[..]);
        builder.set_input_sha256(&self.input_sha256[..]);
        builder.set_file_size(self.file_size);
        builder.set_start_time(self.start_time.timestamp_nanos() as u64);
        builder.set_export_time(self.export_time.timestamp_nanos() as u64);
        builder.set_finish_time(self.finish_time.timestamp_nanos() as u64);
        builder.set_exporter(&self.exporter);
        Ok(())
    }
}
