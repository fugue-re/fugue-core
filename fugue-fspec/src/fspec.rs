use std::borrow::{Borrow, Cow};
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};

use bitflags::bitflags;

use fugue_ir::disassembly::{IRBuilderArena, PCodeBlock};
use fugue_ir::Translator;
use fugue_sleigh::{CodeBlock, IRBuilder, IRBuilderError};

use serde::de::value::StringDeserializer;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use thiserror::Error;

use crate::common::{
    AttrOptWithVal, AttrWithVal, GroupOrValue, GroupOrValueVisitor, Language, OneOrMany,
};
use crate::pattern::PatternsWithContext;

bitflags! {
    #[derive(
        Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize
    )]
    #[repr(transparent)]
    pub struct FunctionProperties: u8 {
        const NON_RETURNING = 0b0000_0001;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum FunctionProperty {
    NonReturning,
}

impl From<OneOrMany<FunctionProperty>> for FunctionProperties {
    fn from(value: OneOrMany<FunctionProperty>) -> Self {
        Vec::from(value).into()
    }
}

impl From<Vec<FunctionProperty>> for FunctionProperties {
    fn from(values: Vec<FunctionProperty>) -> Self {
        let mut props = FunctionProperties::empty();
        for value in values {
            match value {
                FunctionProperty::NonReturning => {
                    props.insert(FunctionProperties::NON_RETURNING);
                }
            }
        }
        props
    }
}

impl From<FunctionProperties> for Vec<FunctionProperty> {
    fn from(value: FunctionProperties) -> Self {
        let mut props = Vec::new();
        for prop in value.iter() {
            match prop {
                FunctionProperties::NON_RETURNING => {
                    props.push(FunctionProperty::NonReturning);
                }
                _ => (),
            }
        }
        props
    }
}

impl From<FunctionProperties> for OneOrMany<FunctionProperty> {
    fn from(value: FunctionProperties) -> Self {
        Vec::from(value).into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlatformConstraint {
    Arch(Language),
    Platform(String),
}

impl<'de> Deserialize<'de> for PlatformConstraint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let av = AttrWithVal::<String, String>::deserialize(deserializer)?;
        match av.attr.as_ref() {
            "arch" => {
                Ok(Self::Arch(Language::deserialize(StringDeserializer::new(av.val))?))
            }
            "platform" => {
                Ok(Self::Platform(av.val))
            }
            _ => {
                Err(<D::Error as serde::de::Error>::custom(
                    "invalid arch/platform constraint (should be of the form arch: ... or platform: ...)",
                ))
            }
        }
    }
}

impl Serialize for PlatformConstraint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Arch(arch) => AttrWithVal {
                attr: "arch",
                val: arch,
            }
            .serialize(serializer),
            Self::Platform(platform) => AttrWithVal {
                attr: "platform",
                val: platform,
            }
            .serialize(serializer),
        }
    }
}

#[derive(Clone, Default)]
pub struct FunctionPatterns {
    languages: BTreeMap<Language, Vec<PatternsWithContext>>,
    default: Vec<PatternsWithContext>,
}

impl<'de> Deserialize<'de> for FunctionPatterns {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let d = Vec::<AttrOptWithVal<Language, PatternsWithContext>>::deserialize(deserializer)?;
        let mut languages = BTreeMap::new();
        let mut default = Vec::new();

        for d in d.into_iter() {
            let (k, v) = match d {
                AttrOptWithVal::Val(v) => {
                    default.push(v);
                    continue;
                }
                AttrOptWithVal::AttrWithVal(av) => (av.attr, av.val),
            };

            match languages.entry(k) {
                Entry::Vacant(entry) => {
                    entry.insert(vec![v]);
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().push(v);
                }
            }
        }

        Ok(Self { languages, default })
    }
}

impl Serialize for FunctionPatterns {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(None)?;
        for (k, v) in self.languages.iter() {
            seq.serialize_element(&AttrOptWithVal::AttrWithVal(AttrWithVal {
                attr: k,
                val: v,
            }))?;
        }
        for v in self.default.iter() {
            seq.serialize_element(&AttrOptWithVal::<Language, _>::Val(v))?;
        }
        seq.end()
    }
}

impl FunctionPatterns {
    pub fn matches(&self, language: impl Borrow<Language>, bytes: impl AsRef<[u8]>) -> bool {
        let bytes = bytes.as_ref();
        self.languages
            .get(language.borrow())
            .into_iter()
            .flatten()
            .chain(self.default.iter())
            .any(|pat| pat.matches_exact(bytes))
    }

    pub fn len(&self) -> usize {
        self.languages.len() + !self.default.is_empty() as usize
    }
}

pub type FunctionSpecAliases = BTreeSet<String>;

#[derive(Clone)]
pub struct FunctionSpec {
    name: String,
    aliases: FunctionSpecAliases,
    constraints: Option<GroupOrValue<PlatformConstraint>>,
    properties: FunctionProperties,
    patterns: FunctionPatterns,
    fixup: Option<FunctionStub>,
}

#[derive(Clone)]
pub struct FunctionStub {
    source: String,
    ast: CodeBlock,
}

impl<'de> Deserialize<'de> for FunctionStub {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let source = String::deserialize(deserializer)?;
        let ast =
            CodeBlock::parse(&source).map_err(|e| <D::Error as serde::de::Error>::custom(e))?;

        Ok(Self { source, ast })
    }
}

impl Serialize for FunctionStub {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.source.serialize(serializer)
    }
}

impl FunctionStub {
    pub fn ast(&self) -> &CodeBlock {
        &self.ast
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn to_pcode<'ir>(
        &self,
        translator: &Translator,
        irb: &'ir IRBuilderArena,
    ) -> Result<PCodeBlock<'ir>, IRBuilderError> {
        let mut builder = IRBuilder::new(translator);
        builder.translate_parsed(irb, self.ast())
    }
}

#[derive(Deserialize, Serialize)]
struct FunctionSpecRepr<'a> {
    name: Cow<'a, str>,
    #[serde(default)]
    aliases: Cow<'a, FunctionSpecAliases>,
    #[serde(default, rename = "where")]
    constraints: Cow<'a, Option<GroupOrValue<PlatformConstraint>>>,
    #[serde(default)]
    properties: Cow<'a, OneOrMany<FunctionProperty>>,
    #[serde(default, alias = "pattern")]
    patterns: Cow<'a, FunctionPatterns>,
    #[serde(default, alias = "stub")]
    fixup: Cow<'a, Option<FunctionStub>>,
}

impl<'de> Deserialize<'de> for FunctionSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let d = FunctionSpecRepr::deserialize(deserializer)?;

        Ok(Self {
            name: d.name.into_owned(),
            aliases: d.aliases.into_owned(),
            constraints: d.constraints.into_owned(),
            properties: d.properties.into_owned().into(),
            patterns: d.patterns.into_owned(),
            fixup: d.fixup.into_owned(),
        })
    }
}

impl Serialize for FunctionSpec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = FunctionSpecRepr {
            name: Cow::Borrowed(&self.name),
            aliases: Cow::Borrowed(&self.aliases),
            constraints: Cow::Borrowed(&self.constraints),
            properties: Cow::Owned(self.properties.into()),
            patterns: Cow::Borrowed(&self.patterns),
            fixup: Cow::Borrowed(&self.fixup),
        };

        s.serialize(serializer)
    }
}

impl FunctionSpec {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn aliases(&self) -> &FunctionSpecAliases {
        &self.aliases
    }

    pub fn properties(&self) -> FunctionProperties {
        self.properties
    }

    pub fn patterns(&self) -> &FunctionPatterns {
        &self.patterns
    }

    pub fn fixup(&self) -> Option<&FunctionStub> {
        self.fixup.as_ref()
    }

    pub fn matches<V>(&self, visitor: &V) -> bool
    where
        V: GroupOrValueVisitor<PlatformConstraint>,
    {
        self.constraints
            .as_ref()
            .map(|c| c.matches(visitor))
            .unwrap_or(true)
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct FunctionSpecs {
    author: Option<String>,
    description: Option<String>,
    constraints: Option<GroupOrValue<PlatformConstraint>>,
    functions: Vec<FunctionSpec>,
}

#[derive(Debug, Error)]
pub enum FunctionSpecError {
    #[error("cannot parse function specifications: {0}")]
    Parse(serde_yaml::Error),
    #[error("cannot function specifications from `{0}`: {1}")]
    ParseFile(PathBuf, serde_yaml::Error),
    #[error("cannot parse function specifications from `{0}`: {1}")]
    ReadFile(PathBuf, io::Error),
}

impl FunctionSpecs {
    pub fn from_str(input: impl AsRef<str>) -> Result<Self, FunctionSpecError> {
        serde_yaml::from_str(input.as_ref()).map_err(FunctionSpecError::Parse)
    }

    pub fn from_reader(reader: impl Read) -> Result<Self, FunctionSpecError> {
        serde_yaml::from_reader(reader).map_err(FunctionSpecError::Parse)
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, FunctionSpecError> {
        let path = path.as_ref();
        let file = BufReader::new(
            File::open(path).map_err(|e| FunctionSpecError::ReadFile(path.to_owned(), e))?,
        );
        serde_yaml::from_reader(file).map_err(|e| FunctionSpecError::ParseFile(path.to_owned(), e))
    }

    pub fn author(&self) -> Option<&str> {
        self.author.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn matches<V>(&self, visitor: &V) -> bool
    where
        V: GroupOrValueVisitor<PlatformConstraint>,
    {
        self.constraints
            .as_ref()
            .map(|c| c.matches(visitor))
            .unwrap_or(true)
    }

    pub fn functions(&self) -> &[FunctionSpec] {
        &self.functions
    }

    pub fn functions_matching<'a, 'v, V>(
        &'a self,
        visitor: &'v V,
    ) -> impl Iterator<Item = &'a FunctionSpec> + 'v
    where
        'a: 'v,
        V: GroupOrValueVisitor<PlatformConstraint>,
    {
        self.functions.iter().filter(|f| f.matches(visitor))
    }
}

#[cfg(test)]
mod test {
    use fugue_bytes::Endian;

    use crate::common::GroupOrValueVisitor;

    use super::*;

    #[test]
    fn test_constraint() -> Result<(), Box<dyn std::error::Error>> {
        let input1 = "arch: x86:LE:32";
        let input2 = "platform: posix";

        assert_eq!(
            PlatformConstraint::Arch(Language::new_with("x86", Endian::Little, 32, None, None)),
            serde_yaml::from_str(input1)?
        );

        assert_eq!(
            PlatformConstraint::Platform("posix".to_owned()),
            serde_yaml::from_str(input2)?
        );

        Ok(())
    }

    #[test]
    fn test_function() -> Result<(), Box<dyn std::error::Error>> {
        let input1 = r#"
name: Perl_croak_no_mem
properties: non-returning
where:
  all:
  - any:
    - arch: x86:LE:32
    - arch: x86:LE:64
  - platform: posix
patterns:
- x86:LE:32:
    patterns:
    - 10 b4
    context:
    - name: TMode
      value: 1
- patterns:
  - 10 b4
  context:
  - name: TMode
    value: 1
"#;

        let fspec = serde_yaml::from_str::<FunctionSpec>(input1)?;

        assert_eq!(fspec.name, "Perl_croak_no_mem");
        assert!(fspec.properties.contains(FunctionProperties::NON_RETURNING));
        assert_eq!(fspec.patterns.len(), 2);

        let constraints = fspec.constraints.unwrap();

        // test group matching via where
        struct ArchWithPlatform {
            arch: Language,
            platform: &'static str,
        }

        impl GroupOrValueVisitor<PlatformConstraint> for ArchWithPlatform {
            fn matches_value(&self, value: &PlatformConstraint) -> bool {
                match value {
                    PlatformConstraint::Platform(platform) => platform == self.platform,
                    PlatformConstraint::Arch(arch) => arch.matches(&self.arch),
                }
            }
        }

        assert!(constraints.matches(&ArchWithPlatform {
            arch: Language::new("x86", Endian::Little),
            platform: "posix",
        }));

        assert!(!constraints.matches(&ArchWithPlatform {
            arch: Language::new("x86", Endian::Little),
            platform: "uefi",
        }));

        Ok(())
    }

    #[test]
    fn test_fixup() -> Result<(), Box<dyn std::error::Error>> {
        let input1 = r#"
name: get_pc_thunk_bx
where:
  all:
  - arch: x86:LE:32
  - platform: posix
patterns:
  - 8B 1C 24 C3
fixup: |
  EBX = *ESP;
  ESP = ESP + 4;
"#;

        let fspec = serde_yaml::from_str::<FunctionSpec>(input1)?;

        assert_eq!(fspec.name, "get_pc_thunk_bx");
        assert_eq!(fspec.patterns.len(), 1);
        assert!(fspec.fixup.is_some());

        Ok(())
    }
}
