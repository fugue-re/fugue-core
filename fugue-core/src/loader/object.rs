use std::borrow::Cow;

use fugue_bytes::Endian;
use fugue_ir::Address;

use object::{File, Object as _, ObjectKind, ObjectSegment};

use crate::attributes::common::CompilerConvention;
use crate::attributes::{Attribute, Attributes};
use crate::language::{Language, LanguageBuilder, LanguageBuilderError};
use crate::loader::{Loadable, LoadableSegment, LoaderError};
use crate::util::BytesOrMapping;

#[ouroboros::self_referencing]
struct ObjectInner<'a> {
    data: BytesOrMapping<'a>,
    attrs: Attributes<'a>,
    #[borrows(data)]
    #[covariant]
    view: File<'this, &'this BytesOrMapping<'a>>,
}

pub struct Object<'a>(ObjectInner<'a>);

impl<'a> Loadable<'a> for Object<'a> {
    fn new(data: impl Into<BytesOrMapping<'a>>) -> Result<Self, LoaderError> {
        ObjectInner::try_new(data.into(), Attributes::new(), |data| {
            File::parse(data).map_err(LoaderError::format)
        })
        .map(Self)
    }

    fn endian(&self) -> Endian {
        if self.0.borrow_view().is_little_endian() {
            Endian::Little
        } else {
            Endian::Big
        }
    }

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

    fn get_attr<T>(&self) -> Option<&T>
    where
        T: Attribute<'a>,
    {
        self.0.borrow_attrs().get_attr::<T>()
    }

    fn set_attr<T>(&mut self, attr: T)
    where
        T: Attribute<'a>,
    {
        self.0.with_attrs_mut(|attrs| attrs.set_attr(attr));
    }

    fn entry(&self) -> Option<Address> {
        let view = self.0.borrow_view();

        if matches!(view.kind(), ObjectKind::Executable) {
            Some(Address::from(view.entry()))
        } else {
            None
        }
    }

    fn segments<'slf>(&'slf self) -> impl Iterator<Item = super::LoadableSegment<'slf>> {
        let view = self.0.borrow_view();

        // TODO: we need to apply relocations

        view.segments().into_iter().filter_map(|segm| {
            if segm.size() == 0 {
                return None;
            }

            let addr = Address::from(segm.address());
            let data = segm.data().unwrap_or_default();

            let data = if data.len() as u64 != segm.size() {
                // we have some partial or fully uninitialised segment?

                let mut data = data.to_owned();
                data.resize(segm.size() as _, 0);

                Cow::Owned(data)
            } else {
                Cow::Borrowed(data)
            };

            Some(LoadableSegment::new(addr, data))
        })
    }
}

#[cfg(test)]
mod test {
    use super::Object;

    use crate::language::LanguageBuilder;
    use crate::loader::Loadable;
    use crate::util::BytesOrMapping;

    #[test]
    #[ignore]
    fn test_elf() -> Result<(), Box<dyn std::error::Error>> {
        let lb = LanguageBuilder::new("data/processors")?;
        let elf = Object::new(BytesOrMapping::from_file("tests/ls.elf")?)?;

        let lang = elf.language(&lb)?;

        assert_eq!(lang.translator().architecture().processor(), "x86");

        Ok(())
    }
}
