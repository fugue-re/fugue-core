#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct VarnodeData {
    pub(crate) space: u32,
    pub(crate) offset: u64,
    pub(crate) size: u32,
}

impl VarnodeData {
    pub fn new(space: u32, offset: u64, size: usize) -> Self {
        Self {
            space,
            offset,
            size: size as _,
        }
    }

    pub fn space(&self) -> u32 {
        self.space
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn size(&self) -> usize {
        self.size as _
    }

    pub fn bits(&self) -> u32 {
        self.size * 8
    }
}
