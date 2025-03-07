use std::fmt::Display;
use std::marker::PhantomData;
use std::ops::{Deref, Index};
use std::str::FromStr;

use fugue_arch::ArchitectureDef;
use fugue_bytes::Endian;

use serde::de::value::StringDeserializer;
use serde::de::{Error, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use thiserror::Error;

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> Default for OneOrMany<T> {
    fn default() -> Self {
        Self::Many(Vec::with_capacity(0))
    }
}

impl<T> From<OneOrMany<T>> for Vec<T> {
    fn from(value: OneOrMany<T>) -> Self {
        match value {
            OneOrMany::One(v) => vec![v],
            OneOrMany::Many(vs) => vs,
        }
    }
}

impl<T> From<Vec<T>> for OneOrMany<T> {
    fn from(value: Vec<T>) -> Self {
        if value.len() == 1 {
            Self::One(value.into_iter().next().unwrap())
        } else {
            Self::Many(value)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AttrWithVal<A, V> {
    pub attr: A,
    pub val: V,
}

struct AttrWithValVisitor<T, A>(PhantomData<(T, A)>);

impl<T, A> Default for AttrWithValVisitor<T, A> {
    fn default() -> Self {
        AttrWithValVisitor(PhantomData)
    }
}

impl<'de, A, V> Visitor<'de> for AttrWithValVisitor<A, V>
where
    A: FromStr,
    V: Deserialize<'de>,
{
    type Value = AttrWithVal<A, V>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("attribute/value pair; e.g., key: value")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: serde::de::MapAccess<'de>,
    {
        let attr = if let Some(attr) = map.next_key::<String>()?.as_deref() {
            attr.parse::<A>()
                .map_err(|_| Error::custom("cannot parse attribute"))?
        } else {
            return Err(Error::custom(format!("expected attribute")))?;
        };

        let val = map.next_value::<V>()?;

        Ok(AttrWithVal { attr, val })
    }
}

impl<'de, A, V> Deserialize<'de> for AttrWithVal<A, V>
where
    A: FromStr,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(AttrWithValVisitor::default())
    }
}

impl<A, V> Serialize for AttrWithVal<A, V>
where
    A: ToString,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&self.attr.to_string(), &self.val)?;
        map.end()
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(
    bound(deserialize = "A: FromStr, V: Deserialize<'de>"),
    bound(serialize = "A: ToString, V: Serialize"),
    untagged
)]
pub enum AttrOptWithVal<A, V> {
    Val(V),
    AttrWithVal(AttrWithVal<A, V>),
}

impl<A, V> From<AttrOptWithVal<A, V>> for (A, V)
where
    A: Default + FromStr,
{
    fn from(v: AttrOptWithVal<A, V>) -> Self {
        match v {
            AttrOptWithVal::Val(v) => (Default::default(), v),
            AttrOptWithVal::AttrWithVal(av) => (av.attr, av.val),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(
    bound(deserialize = "A: FromStr, U: Deserialize<'de>, V: Deserialize<'de>"),
    bound(serialize = "A: ToString, U: Serialize, V: Serialize"),
    untagged
)]
pub enum AttrOptWithValOr<A, U, V>
where
    A: FromStr,
{
    Val(U),
    AttrWithVal(AttrWithVal<A, V>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GroupKind {
    All,
    Any,
    NotAll,
    NotAny,
}

impl Default for GroupKind {
    fn default() -> Self {
        GroupKind::All
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Group<T> {
    kind: GroupKind,
    values: Vec<T>,
}

impl<T> Group<T> {
    #[inline]
    pub fn kind(&self) -> GroupKind {
        self.kind
    }

    #[inline]
    pub fn values(&self) -> &[T] {
        &self.values
    }
}

impl<T> Index<usize> for Group<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.values[index]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Groups<T>(Vec<Group<T>>);

impl<T> Deref for Groups<T> {
    type Target = Vec<Group<T>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Index<usize> for Groups<T> {
    type Output = Group<T>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<T> Groups<T> {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Group<T>> {
        self.0.iter()
    }

    #[inline]
    pub fn groups(&self) -> &[Group<T>] {
        &self.0
    }
}

impl<T> Default for Groups<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

struct GroupVisitor<T>(PhantomData<T>);

impl<T> Default for GroupVisitor<T> {
    fn default() -> Self {
        GroupVisitor(PhantomData)
    }
}

impl<'de, T> Visitor<'de> for GroupVisitor<T>
where
    T: Deserialize<'de>,
{
    type Value = Groups<T>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let kind = Default::default();
        let mut values = Vec::with_capacity(seq.size_hint().unwrap_or(0));

        while let Some(value) = seq.next_element()? {
            values.push(value);
        }

        Ok(Groups(vec![Group { kind, values }]))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut groups = Vec::with_capacity(map.size_hint().unwrap_or(0));

        loop {
            let mut is_not = false;

            let kind = match map.next_key::<String>()?.as_deref() {
                Some("and" | "all") => GroupKind::All,
                Some("any" | "or") => GroupKind::Any,
                Some("not-all") => GroupKind::NotAll,
                Some("not-any") => GroupKind::NotAny,
                Some("not") => {
                    is_not = true;
                    GroupKind::NotAny
                }
                Some(c) => {
                    return Err(Error::custom(format!("unknown grouping operator `{c}`")));
                }
                None => break,
            };

            let values = if is_not {
                vec![map.next_value::<T>()?]
            } else {
                map.next_value::<Vec<T>>()?
            };

            groups.push(Group { kind, values });
        }

        Ok(Groups(groups))
    }
}

impl<'de, T> Deserialize<'de> for Groups<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(GroupVisitor::default())
    }
}

impl<T> Serialize for Groups<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.len()))?;

        for group in self.groups() {
            map.serialize_entry(&group.kind(), group.values())?;
        }

        map.end()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GroupOrValue<T> {
    Group(Groups<GroupOrValue<T>>),
    Value(T),
}

pub trait GroupOrValueVisitor<T> {
    fn matches_value(&self, value: &T) -> bool;
}

impl<T> GroupOrValue<T> {
    pub fn matches<V>(&self, visitor: &V) -> bool
    where
        V: GroupOrValueVisitor<T>,
    {
        match self {
            Self::Value(t) => visitor.matches_value(t),
            Self::Group(group) => group.iter().any(|g| match g.kind() {
                GroupKind::All => g.values().iter().all(|v| v.matches(visitor)),
                GroupKind::Any => g.values().iter().any(|v| v.matches(visitor)),
                GroupKind::NotAll => !g.values().iter().all(|v| v.matches(visitor)),
                GroupKind::NotAny => !g.values().iter().any(|v| v.matches(visitor)),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArchSpec {
    processor: String,
    endian: Endian,
    bits: Option<u32>,
    variant: Option<String>,
    convention: Option<String>,
}

impl ArchSpec {
    pub fn new(processor: impl Into<String>, endian: Endian) -> Self {
        Self::new_with(processor, endian, None, None, None)
    }

    pub fn new_with(
        processor: impl Into<String>,
        endian: Endian,
        bits: impl Into<Option<u32>>,
        variant: impl Into<Option<String>>,
        convention: impl Into<Option<String>>,
    ) -> Self {
        Self {
            processor: processor.into(),
            endian,
            bits: bits.into(),
            variant: variant.into(),
            convention: convention.into(),
        }
    }

    pub fn matches(&self, other: &Self) -> bool {
        (other.processor() == self.processor())
            && (other.endian() == self.endian())
            && (other.bits().is_none() || self.bits().is_none() || other.bits() == self.bits())
            && (other.variant().is_none()
                || self.variant().is_none()
                || other.variant() == self.variant())
            && (other.convention().is_none()
                || self.convention().is_none()
                || other.convention() == self.convention())
    }

    pub fn matches_arch(&self, arch: &ArchitectureDef) -> bool {
        (arch.processor() == self.processor())
            && (arch.endian() == self.endian())
            && (self.bits().is_none() || arch.bits() as u32 == self.bits().unwrap())
            && (self.variant().is_none() || arch.variant() == self.variant().unwrap())
    }

    pub fn processor(&self) -> &str {
        &self.processor
    }

    pub fn endian(&self) -> Endian {
        self.endian
    }

    pub fn bits(&self) -> Option<u32> {
        self.bits
    }

    pub fn variant(&self) -> Option<&str> {
        self.variant.as_deref()
    }

    pub fn convention(&self) -> Option<&str> {
        self.convention.as_deref()
    }
}

#[derive(Debug, Error)]
pub enum LanguageParseError {
    #[error("architecture format is invalid")]
    Format,
    #[error("invalid architecture processor (should be non-empty string and not '*')")]
    Processor,
    #[error("invalid architecture endian (should be LE or BE)")]
    Endian,
    #[error("invalid architecture bits (should be numeric or '*')")]
    Bits,
    #[error("invalid architecture variant (should be non-empty string or '*')")]
    Variant,
    #[error("invalid architecture convention (should be non-empty string or '*')")]
    Convention,
}

impl FromStr for ArchSpec {
    type Err = LanguageParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.splitn(5, ':').collect::<Vec<_>>();
        if parts.len() < 3 {
            return Err(LanguageParseError::Format);
        }

        let processor = if parts[0] == "*" || parts[0].is_empty() {
            return Err(LanguageParseError::Processor);
        } else {
            parts[0].to_owned()
        };

        let endian = match parts[1] {
            "le" | "LE" => Endian::Little,
            "be" | "BE" => Endian::Big,
            _ => {
                return Err(LanguageParseError::Endian);
            }
        };

        let bits = if parts[2] == "*" {
            None
        } else {
            match parts[2].parse::<u32>() {
                Ok(bits) => Some(bits),
                Err(_) => {
                    return Err(LanguageParseError::Bits);
                }
            }
        };

        let variant = if parts.len() < 4 || parts[3] == "*" {
            None
        } else if parts[3].is_empty() {
            return Err(LanguageParseError::Variant);
        } else {
            Some(parts[3].to_owned())
        };

        let convention = if parts.len() < 5 || parts[4] == "*" {
            None
        } else if parts[4].is_empty() {
            return Err(LanguageParseError::Convention);
        } else {
            Some(parts[4].to_owned())
        };

        Ok(ArchSpec {
            processor,
            endian,
            bits,
            variant,
            convention,
        })
    }
}

impl Display for ArchSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let endian = match self.endian {
            Endian::Big => "BE",
            Endian::Little => "LE",
        };

        let bits = self
            .bits
            .as_ref()
            .map(|bits| bits as &dyn Display)
            .unwrap_or_else(|| &"*" as &dyn Display);

        let variant = self.variant.as_deref().unwrap_or("*");
        let convention = self.convention.as_deref().unwrap_or("*");

        write!(
            f,
            "{}:{}:{}:{}:{}",
            self.processor, endian, bits, variant, convention
        )
    }
}

impl<'de> Deserialize<'de> for ArchSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ArchSpec::from_str(&s).map_err(D::Error::custom)
    }
}

impl Serialize for ArchSpec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let parts = self.to_string();
        serializer.serialize_str(&parts)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlatformConstraint {
    Arch(ArchSpec),
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
                Ok(Self::Arch(ArchSpec::deserialize(StringDeserializer::new(av.val))?))
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

pub type PlatformConstraints = GroupOrValue<PlatformConstraint>;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_basic_groups() -> Result<(), Box<dyn std::error::Error>> {
        let map1 = r"
- blah
- blah1
- blah2
";
        let map2 = r"
or:
    - blah
    - blah1
    - blah2
";

        let map3 = r"
or:
    - blah
    - blah1
    - blah2
and:
    - blah4
";

        let map4 = r"
or:
    - not: blah0
    - blah1
    - blah2
not: blah4
";

        let map5 = r"
blah
";

        let map1_group: Groups<String> = serde_yaml::from_str(map1)?;

        assert_eq!(
            map1_group,
            Groups(vec![Group {
                kind: GroupKind::All,
                values: vec!["blah".to_owned(), "blah1".to_owned(), "blah2".to_owned()],
            }])
        );

        let map2_group: Groups<String> = serde_yaml::from_str(map2)?;

        assert_eq!(
            map2_group,
            Groups(vec![Group {
                kind: GroupKind::Any,
                values: vec!["blah".to_owned(), "blah1".to_owned(), "blah2".to_owned()],
            }])
        );

        let map3_group: Groups<String> = serde_yaml::from_str(map3)?;

        assert_eq!(
            map3_group,
            Groups(vec![
                Group {
                    kind: GroupKind::Any,
                    values: vec!["blah".to_owned(), "blah1".to_owned(), "blah2".to_owned()],
                },
                Group {
                    kind: GroupKind::All,
                    values: vec!["blah4".to_owned()],
                },
            ]),
        );

        let map4_group: GroupOrValue<String> = serde_yaml::from_str(map4)?;

        assert_eq!(
            map4_group,
            GroupOrValue::Group(Groups(vec![
                Group {
                    kind: GroupKind::Any,
                    values: vec![
                        GroupOrValue::Group(Groups(vec![Group {
                            kind: GroupKind::NotAny,
                            values: vec![GroupOrValue::Value("blah0".to_owned())]
                        }])),
                        GroupOrValue::Value("blah1".to_owned()),
                        GroupOrValue::Value("blah2".to_owned())
                    ],
                },
                Group {
                    kind: GroupKind::NotAny,
                    values: vec![GroupOrValue::Value("blah4".to_owned())],
                },
            ])),
        );

        let map5_group: GroupOrValue<String> = serde_yaml::from_str(map5)?;

        assert_eq!(map5_group, GroupOrValue::Value("blah".to_owned()));

        Ok(())
    }
}
