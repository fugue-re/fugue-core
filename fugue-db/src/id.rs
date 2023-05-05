use std::marker::PhantomData;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Serialize, serde::Deserialize)]
#[repr(transparent)]
/// Upper 32 bit: parent
/// Lower 32 bit: index
/// All 1: invalid
pub struct Id<T>(u64, PhantomData<T>);

impl<T> From<u32> for Id<T> {
    fn from(value: u32) -> Self {
        Self(value as i32 as i64 as u64, PhantomData)
    }
}

impl<T> From<u64> for Id<T> {
    fn from(value: u64) -> Self {
        Self(value, PhantomData)
    }
}

impl<T> From<usize> for Id<T> {
    fn from(value: usize) -> Self {
        Self(value as u64, PhantomData)
    }
}

impl<T> Id<T> {
    pub const fn invalid() -> Id<T> {
        Id(0xffffffff_ffffffff, PhantomData)
    }

    pub const fn is_invalid(&self) -> bool {
        self.0 == 0xffffffff_ffffffff
    }

    pub const fn parent(&self) -> usize {
        (self.0 >> 32) as usize
    }

    pub const fn index(&self) -> usize {
        (self.0 & 0xffffffff) as usize
    }

    pub const fn value(&self) -> u64 {
        self.0
    }
}
