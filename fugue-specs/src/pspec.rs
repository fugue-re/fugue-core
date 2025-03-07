use std::borrow::Cow;
use std::fs::File;
use std::io;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::common::{PlatformConstraints, PlatformConstraint, GroupOrValueVisitor};
use crate::pattern::{PatternGroup, PatternMatchIter, PatternOrGroupSeq, PatternsWithContext};

#[derive(Debug, Error)]
pub enum PatternSpecError {
    #[error("cannot parse patterns: {0}")]
    Parse(serde_yaml::Error),
    #[error("cannot parse patterns from `{0}`: {1}")]
    ParseFile(PathBuf, serde_yaml::Error),
    #[error("cannot parse patterns from `{0}`: {1}")]
    ReadFile(PathBuf, io::Error),
}

#[derive(Clone)]
pub struct PatternSpecs {
    author: Option<String>,
    description: Option<String>,
    constraints: Option<PlatformConstraints>,
    groups: Vec<PatternGroup>,
    patterns: Vec<PatternsWithContext>,
}

#[derive(Deserialize, Serialize)]
struct PatternSpecsT<'a> {
    #[serde(default)]
    author: Option<Cow<'a, str>>,
    #[serde(default)]
    description: Option<Cow<'a, str>>,
    #[serde(default)]
    constraints: Option<Cow<'a, PlatformConstraints>>,
    #[serde(
        bound(deserialize = "PatternOrGroupSeq<'a>: Deserialize<'de>"),
        default
    )]
    patterns: PatternOrGroupSeq<'a>,
}

impl PatternSpecs {
    pub fn from_str(input: impl AsRef<str>) -> Result<Self, PatternSpecError> {
        serde_yaml::from_str(input.as_ref()).map_err(PatternSpecError::Parse)
    }

    pub fn from_reader(reader: impl Read) -> Result<Self, PatternSpecError> {
        serde_yaml::from_reader(reader).map_err(PatternSpecError::Parse)
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, PatternSpecError> {
        let path = path.as_ref();
        let file = BufReader::new(
            File::open(path).map_err(|e| PatternSpecError::ReadFile(path.to_owned(), e))?,
        );
        serde_yaml::from_reader(file).map_err(|e| PatternSpecError::ParseFile(path.to_owned(), e))
    }

    pub fn author(&self) -> Option<&str> {
        self.author.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn constraints(&self) -> Option<&PlatformConstraints> {
        self.constraints.as_ref()
    }

    pub fn can_match<V>(&self, visitor: &V) -> bool
    where
        V: GroupOrValueVisitor<PlatformConstraint>,
    {
        self.constraints
            .as_ref()
            .map(|c| c.matches(visitor))
            .unwrap_or(true)
    }

    pub fn matches<'a>(&'a self, bytes: &'a [u8]) -> PatternMatchIter<'a> {
        PatternMatchIter::new(
            self.groups
                .iter()
                .flat_map(|group| group.matches(bytes))
                .chain(
                    self.patterns
                        .iter()
                        .flat_map(|pattern| pattern.matches(bytes)),
                ),
        )
    }
}

impl<'de> Deserialize<'de> for PatternSpecs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let ps = PatternSpecsT::deserialize(deserializer)?;

        Ok(Self {
            author: ps.author.map(Cow::into_owned),
            description: ps.description.map(Cow::into_owned),
            constraints: ps.constraints.map(Cow::into_owned),
            groups: ps.patterns.groups.into_owned(),
            patterns: ps.patterns.patterns.into_owned(),
        })
    }
}

impl Serialize for PatternSpecs {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let t = PatternSpecsT {
            author: self.author.as_deref().map(Cow::Borrowed),
            description: self.description.as_deref().map(Cow::Borrowed),
            constraints: self.constraints.as_ref().map(Cow::Borrowed),
            patterns: PatternOrGroupSeq {
                groups: Cow::Borrowed(&self.groups),
                patterns: Cow::Borrowed(&self.patterns),
            },
        };

        t.serialize(serializer)
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;

    const PAT: &'static str = r#"
constraints:
- arch: ARM:LE:32
patterns:
- pattern-group:
    post:
      context:
      - name: TMode
        value: 1
      patterns:
      - .. b5 1....... b0
      - .. b5 00...... 1c
      - .. b5 .. 46
      - .. b5 .. 01.01...
      - .. b5 .. 68
      - .. b5 .. 01.01... 10...... b0
      - 1....... b5 .. af
      - 100..... b0 .0 b5
      - 00...... 1c .0 b5
      - .. 01.01... .0 b5
      - .. 68 .0 b5
      - 2d e9 .. 0.
      - 4d f8 04 ed
    post-bits: 16
    pre:
      - '.......0 bd'
      - '.......0 bd 00 00'
      - '.......0 bd 00 bf'
      - '.......0 bd c0 46'
      - ff ff
      - c0 46
      - 70 47
      - 70 47 00 00
      - 70 47 c0 46
      - 70 47 00 bf
      - 000..... b0 .0 bd
      - 00 bf
      - af f3 00 80
      - bd e8 .. 0.
      - 46 f7
      - 5d f8 0....... fb
      - 5d f8 04 fb
      - bd e8 .. 100.....
    total-bits: 32
- pattern:
    context:
    - name: TMode
      value: 0
    patterns:
    - .. 0. 8f e2 .. 0. 8c e2 .. 0. bc e5
- pattern:
    context:
    - name: TMode
      value: 1
    patterns:
    - 03 b4 01 48 01 90 01 bd
- pattern:
    context:
    - name: TMode
      value: 1
    patterns:
    - 10 b5
"#;

    #[test]
    fn test_yaml_parse() -> Result<(), Box<dyn std::error::Error>> {
        let _ = serde_yaml::from_reader::<_, PatternSpecs>(Cursor::new(PAT))?;
        let v = serde_yaml::from_str::<PatternSpecs>(PAT)?;
        let _ = serde_yaml::to_string(&v)?;

        Ok(())
    }
}
