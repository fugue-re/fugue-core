#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
