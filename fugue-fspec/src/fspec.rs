use std::borrow::Cow;
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use bitflags::bitflags;

use serde::de::value::StringDeserializer;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::common::{AttrOptWithVal, AttrWithVal, GroupOrValue, Language, OneOrMany};
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
pub struct FunctionPatterns(BTreeMap<Option<Language>, Vec<PatternsWithContext>>);

impl<'de> Deserialize<'de> for FunctionPatterns {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let d = Vec::<AttrOptWithVal<Language, PatternsWithContext>>::deserialize(deserializer)?;
        let mut m = BTreeMap::new();

        for d in d.into_iter() {
            let (k, v) = match d {
                AttrOptWithVal::Val(v) => (None, v),
                AttrOptWithVal::AttrWithVal(av) => (Some(av.attr), av.val),
            };

            match m.entry(k) {
                Entry::Vacant(entry) => {
                    entry.insert(vec![v]);
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().push(v);
                }
            }
        }

        Ok(Self(m))
    }
}

impl Serialize for FunctionPatterns {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for (k, v) in self.0.iter() {
            seq.serialize_element(&match k {
                None => AttrOptWithVal::Val(v),
                Some(k) => AttrOptWithVal::AttrWithVal(AttrWithVal { attr: k, val: v }),
            })?;
        }
        seq.end()
    }
}

#[derive(Clone)]
pub struct FunctionSpec {
    function: String,
    constraints: Option<GroupOrValue<PlatformConstraint>>,
    properties: FunctionProperties,
    patterns: FunctionPatterns,
    fixup: Option<String>, // TODO: this will be PCode
}

#[derive(Deserialize, Serialize)]
struct FunctionSpecRepr<'a> {
    function: Cow<'a, str>,
    #[serde(default, rename = "where")]
    constraints: Cow<'a, Option<GroupOrValue<PlatformConstraint>>>,
    #[serde(default)]
    properties: Cow<'a, OneOrMany<FunctionProperty>>,
    #[serde(default, alias = "pattern")]
    patterns: Cow<'a, FunctionPatterns>,
    #[serde(default, alias = "stub")]
    fixup: Cow<'a, Option<String>>,
}

impl<'de> Deserialize<'de> for FunctionSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let d = FunctionSpecRepr::deserialize(deserializer)?;

        Ok(Self {
            function: d.function.into_owned(),
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
            function: Cow::Borrowed(&self.function),
            constraints: Cow::Borrowed(&self.constraints),
            properties: Cow::Owned(self.properties.into()),
            patterns: Cow::Borrowed(&self.patterns),
            fixup: Cow::Borrowed(&self.fixup),
        };

        s.serialize(serializer)
    }
}

impl FunctionSpec {
    pub fn function(&self) -> &str {
        &self.function
    }
}

#[cfg(test)]
mod test {
    use fugue_bytes::Endian;

    use crate::common::GroupKind;

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
function: Perl_croak_no_mem
properties: non-returning
where:
  any:
  - platform: posix
  - platform: uefi
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

        assert_eq!(fspec.function, "Perl_croak_no_mem");
        assert!(fspec.properties.contains(FunctionProperties::NON_RETURNING));
        assert_eq!(fspec.patterns.0.len(), 2);

        let constraints = fspec.constraints.unwrap();
        let GroupOrValue::Group(group) = constraints else {
            panic!("expected group")
        };

        assert_eq!(group.len(), 1);
        let group0 = &group.groups()[0];

        assert_eq!(group0.kind(), GroupKind::Any);
        assert_eq!(
            group0.values(),
            [
                GroupOrValue::Value(PlatformConstraint::Platform("posix".to_owned())),
                GroupOrValue::Value(PlatformConstraint::Platform("uefi".to_owned())),
            ]
        );

        Ok(())
    }
}
