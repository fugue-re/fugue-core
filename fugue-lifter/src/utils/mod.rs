pub mod constructor;
pub mod context;
pub mod input;
pub mod partmap;
pub mod varnode;

pub use constructor::{Constructor, Operand, OperandResolver};
pub use context::ContextDatabase;
pub use input::{ContextCommit, FixedHandle, ParserInput};

const UMASKS: [u64; 9] = [
    0,
    0xff,
    0xffff,
    0xffffff,
    0xffffffff,
    0xffffffffff,
    0xffffffffffff,
    0xffffffffffffff,
    0xffffffffffffffff,
];

#[inline(always)]
pub fn calculate_mask(index: usize) -> u64 {
    UMASKS[if index >= UMASKS.len() {
        UMASKS.len() - 1
    } else {
        index
    }]
}

#[inline(always)]
pub fn sign_extend(value: i64, size: usize) -> i64 {
    let mask = (!0i64).checked_shl(size as u32).unwrap_or(0);
    if (value
        .checked_shr(size as u32)
        .unwrap_or(if value < 0 { -1 } else { 0 })
        & 1)
        != 0
    {
        value | mask
    } else {
        value & !mask
    }
}

#[inline(always)]
pub fn zero_extend(value: i64, size: usize) -> i64 {
    let mask = (!0i64)
        .checked_shl(size as u32)
        .unwrap_or(0)
        .checked_shl(1)
        .unwrap_or(0);
    value & !mask
}

#[inline(always)]
pub fn byte_swap(value: i64, size: usize) -> i64 {
    let mut res = 0i64;
    let mut val = value;
    let mut size = size;
    while size > 0 {
        res = res.checked_shl(8).unwrap_or(0);
        res |= val & 0xff;
        val = val.checked_shr(8).unwrap_or(if val < 0 { -1 } else { 0 });
        size -= 1;
    }
    res
}

#[inline(always)]
pub fn wrap_offset(highest: u64, offset: u64) -> u64 {
    if offset <= highest {
        offset
    } else {
        let m = (highest + 1) as i64;
        let r = (offset as i64) % m;
        (if r < 0 { r + m } else { r }) as u64
    }
}
