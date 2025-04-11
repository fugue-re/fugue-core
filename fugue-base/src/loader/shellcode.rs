use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use fallible_iterator::FallibleIterator;

use thiserror::Error;

use crate::lifter::{Language, Lifter, LifterBuilder};
use crate::loader::{Loadable, LoadableSegment, LoadableSegmentProperties, LoaderError};
use crate::types::{Address, AttributeMap, BytesOrMapping};

pub struct Shellcode<'a> {
    address: Address,
    bytes: BytesOrMapping<'a>,
    lifter: Lifter,
    attributes: AttributeMap,
}

impl fmt::Debug for Shellcode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shellcode")
            .field("address", &self.address)
            .field("attributes", &self.attributes)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Error)]
pub enum ShellcodeError {
    #[error("mapping {1} bytes at {0} will overflow the default address space")]
    AddressOverflow(Address, usize),
    #[error("buffer to map must be not be empty")]
    ZeroSized,
}

impl<'a> Shellcode<'a> {
    pub fn new(
        language: impl AsRef<str>,
        address: impl Into<Address>,
        bytes: impl Into<BytesOrMapping<'a>>,
    ) -> Result<Self, LoaderError> {
        Self::new_with(language, address, bytes, AttributeMap::default())
    }

    pub fn new_with(
        language: impl AsRef<str>,
        address: impl Into<Address>,
        bytes: impl Into<BytesOrMapping<'a>>,
        attributes: impl Into<AttributeMap>,
    ) -> Result<Self, LoaderError> {
        let language = language.as_ref();
        let lifter = LifterBuilder::from_str(language).and_then(|builder| builder.build())?;

        let bytes = bytes.into();
        if bytes.is_empty() {
            return Err(LoaderError::format(ShellcodeError::ZeroSized));
        }

        let address = address.into();
        let size = bytes.len();

        let language = lifter.language();
        if !address.range_in_space_bounds(language, size) {
            return Err(LoaderError::format(ShellcodeError::AddressOverflow(
                address, size,
            )));
        }

        Ok(Self {
            address: address.into(),
            bytes: bytes.into(),
            lifter,
            attributes: attributes.into(),
        })
    }

    pub fn address(&self) -> Address {
        self.address
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Loadable for Shellcode<'_> {
    fn entry_address(&self) -> Option<Address> {
        Some(self.address())
    }

    fn segments<'a>(
        &'a self,
    ) -> impl FallibleIterator<Item = LoadableSegment<'a>, Error = LoaderError> + 'a {
        fallible_iterator::once(LoadableSegment {
            name: Cow::Borrowed("LOAD"),
            address: self.address,
            properties: LoadableSegmentProperties::PERM_ALL,
            bytes: Cow::Borrowed(self.bytes()),
        })
    }

    fn segment_range(&self) -> (Address, Address) {
        (self.address, self.address + self.bytes.len() - 1usize)
    }

    fn language(&self) -> &'static Language {
        self.lifter.language()
    }

    fn lifter(&self) -> Lifter {
        self.lifter.clone()
    }

    fn attributes(&self) -> &AttributeMap {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut AttributeMap {
        &mut self.attributes
    }
}

#[cfg(test)]
mod test {
    use fallible_iterator::FallibleIterator;

    use crate::attributes;
    use crate::loader::shellcode::Shellcode;
    use crate::loader::Loadable;
    use crate::types::Address;

    #[test]
    fn test_arm_snippet() -> anyhow::Result<()> {
        let shellcode = Shellcode::new_with(
            "ARM:LE:32",
            0x1000u32,
            &[
                0x07, 0x50, 0xa0, 0xe1, 0x00, 0xb0, 0x95, 0xe5, 0x00, 0x10, 0x94, 0xe5, 0x01, 0x20,
                0xa0, 0xe1, 0x00, 0x20, 0x87, 0xe5,
            ],
            attributes![
                "path" => "/path/to/shellcode.exe",
            ],
        )?;

        let regions = shellcode.segments().collect::<Vec<_>>()?;
        assert_eq!(regions.len(), 1);

        let region = &regions[0];
        assert_eq!(region.address, Address::from(0x1000u32));

        let mut lifter = shellcode.lifter();
        let mut offset = 0usize;
        let mut output = String::new();

        let address = shellcode.address().offset();
        let bytes = shellcode.bytes();

        while offset < bytes.len() {
            let len = lifter
                .disassemble(address + offset as u64, &bytes[offset..], &mut output)
                .expect("valid");
            offset += len;
            output.push('\n');
        }

        assert_eq!(
            output,
            r#"cpy r5,r7
ldr r11,[r5,#0x0]
ldr r1,[r4,#0x0]
cpy r2,r1
str r2,[r7,#0x0]
"#
        );

        Ok(())
    }
}
