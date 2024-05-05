use fugue_bytes::Endian;
use object::{File, Object as _};

use crate::language::{Language, LanguageBuilder, LanguageBuilderError};
use crate::loader::LoaderError;
use crate::util::BytesOrMapping;

#[ouroboros::self_referencing]
struct ObjectInner<'a> {
    data: BytesOrMapping<'a>,
    #[borrows(data)]
    #[covariant]
    view: File<'this, &'this BytesOrMapping<'a>>,
}

pub struct Object<'a>(ObjectInner<'a>);

impl<'a> Object<'a> {
    pub fn new(data: impl Into<BytesOrMapping<'a>>) -> Result<Self, LoaderError> {
        ObjectInner::try_new(data.into(), |data| {
            File::parse(data).map_err(LoaderError::format)
        })
        .map(Self)
    }

    pub fn endian(&self) -> Endian {
        if self.0.borrow_view().is_little_endian() {
            Endian::Little
        } else {
            Endian::Big
        }
    }

    pub fn language(&self, builder: &LanguageBuilder) -> Result<Language, LoaderError> {
        let convention = match self.0.borrow_view() {
            File::Pe32(_) | File::Pe64(_) => "windows",
            File::Elf32(_) | File::Elf64(_) => "gcc",
            _ => "default",
        };
        self.language_with(builder, convention)
    }

    pub fn language_with(
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
}

#[cfg(test)]
mod test {
    use super::Object;

    use crate::language::LanguageBuilder;
    use crate::util::BytesOrMapping;

    #[test]
    fn test_elf() -> Result<(), Box<dyn std::error::Error>> {
        let lb = LanguageBuilder::new("data/processors")?;
        let elf = Object::new(BytesOrMapping::from_file("tests/ls.elf")?)?;

        let lang = elf.language(&lb)?;

        assert_eq!(lang.translator().architecture().processor(), "x86");

        Ok(())
    }
}
