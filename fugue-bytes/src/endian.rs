#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum Endian {
    Big,
    Little,
}

impl Endian {
    pub fn is_big(&self) -> bool {
        matches!(self, Self::Big)
    }

    pub fn is_little(&self) -> bool {
        matches!(self, Self::Little)
    }
}
