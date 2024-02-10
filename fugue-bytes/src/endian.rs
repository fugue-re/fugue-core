use std::fmt::Display;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum Endian {
    Big,
    Little,
}

impl Display for Endian {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.is_big() { "BE" } else { "LE" })
    }
}

impl Endian {
    pub fn is_big(&self) -> bool {
        matches!(self, Self::Big)
    }

    pub fn is_little(&self) -> bool {
        matches!(self, Self::Little)
    }
}
