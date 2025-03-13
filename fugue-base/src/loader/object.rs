use std::borrow::Cow;

use fugue_lifter::{Language, Lifter};
use object::{File, Object as _, ObjectSegment};

use crate::loader::{Loadable, LoadableSegment, LoadableSegmentProperties, LoaderError};
use crate::types::{Address, Attribute, AttributeMap, BytesOrMapping};

#[ouroboros::self_referencing]
struct ObjectInner<'a> {
    data: BytesOrMapping<'a>,
    attrs: AttributeMap,
    #[borrows(data)]
    #[covariant]
    view: File<'this, &'this BytesOrMapping<'a>>,
}

pub struct Object<'a>(ObjectInner<'a>);

impl<'a> Object<'a> {
    pub fn new(data: impl Into<BytesOrMapping<'a>>) -> Result<Self, LoaderError> {
        Self::new_with(data, AttributeMap::new())
    }

    pub fn new_with(
        data: impl Into<BytesOrMapping<'a>>,
        attributes: impl Into<AttributeMap>,
    ) -> Result<Self, LoaderError> {
        ObjectInner::try_new(data.into(), attributes.into(), |data| {
            File::parse(data).map_err(LoaderError::format)
        })
        .map(Self)
    }
}

impl Loadable for Object<'_> {
    fn entry_address(&self) -> Option<Address> {
        Some(self.0.borrow_view().entry().into())
    }

    fn get_attr<T>(&self, key: impl std::borrow::Borrow<str>) -> Option<T>
    where
        T: Attribute,
    {
        self.0.borrow_attrs().get_attr(key)
    }

    fn set_attr(&mut self, key: impl ToString, val: impl Attribute) {
        self.0.with_attrs_mut(|attrs| attrs.set_attr(key, val))
    }

    fn language(&self) -> &'static Language {
        todo!()
    }

    fn lifter(&self) -> Lifter {
        todo!()
    }

    fn segments<'a>(&'a self) -> impl Iterator<Item = LoadableSegment<'a>> + 'a {
        let view = self.0.borrow_view();

        // TODO: we need to apply relocations

        view.segments().into_iter().filter_map(|segm| {
            if segm.size() == 0 {
                return None;
            }

            let address = Address::from(segm.address());
            let data = segm.data().unwrap_or_default();

            let bytes = if data.len() as u64 != segm.size() {
                // we have some partial or fully uninitialised segment?

                let mut data = data.to_owned();
                data.resize(segm.size() as _, 0);

                Cow::Owned(data)
            } else {
                Cow::Borrowed(data)
            };

            Some(LoadableSegment {
                name: segm
                    .name()
                    .ok()
                    .flatten()
                    .map_or_else(|| Cow::Borrowed("LOAD"), |name| Cow::Owned(name.to_owned())),
                address,
                properties: LoadableSegmentProperties::all(),
                bytes,
            })
        })
    }

    /*
    fn language(&self, builder: &LanguageBuilder) -> Result<Language, LoaderError> {
        if let Some(convention) = self.get_attr_as::<CompilerConvention, _>() {
            return self.language_with(builder, convention);
        }

        let convention = match self.0.borrow_view() {
            File::Pe32(_) | File::Pe64(_) => "windows",
            File::Elf32(_) | File::Elf64(_) => "gcc",
            _ => "default",
        };

        self.language_with(builder, convention)
    }

    fn language_with(
        &self,
        builder: &LanguageBuilder,
        convention: impl AsRef<str>,
    ) -> Result<Language, LoaderError> {
        use object::{Architecture as A, Endianness as E};

        let view = self.0.borrow_view();
        let bits = if view.is_64() { 64 } else { 32 };
        let conv = convention.as_ref();

        let language = match (view.architecture(), view.endianness(), bits) {
            (A::Arm, E::Big, 32) => builder.build_with("ARM", Endian::Big, 32, "v7", conv)?,
            (A::Arm, E::Little, 32) => builder.build_with("ARM", Endian::Little, 32, "v7", conv)?,
            (A::Arm, E::Big, 64) => builder.build_with("AARCH64", Endian::Big, 64, "v8A", conv)?,
            (A::Arm, E::Little, 64) => {
                builder.build_with("AARCH64", Endian::Little, 64, "v8A", conv)?
            }
            (A::I386, E::Little, 32) => {
                builder.build_with("x86", Endian::Little, 32, "default", conv)?
            }
            (A::X86_64, E::Little, 64) => {
                builder.build_with("x86", Endian::Little, 64, "default", conv)?
            }
            _ => return Err(LanguageBuilderError::UnsupportedArch.into()),
        };

        Ok(language)
    }
    */
}

#[cfg(test)]
mod test {
    use crate::types::BytesOrMapping;

    use super::Object;

    #[test]
    #[ignore]
    fn test_elf() -> Result<(), Box<dyn std::error::Error>> {
        let _elf = Object::new(BytesOrMapping::from_file("tests/ls.elf")?)?;
        Ok(())
    }
}
