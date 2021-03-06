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

pub fn calculate_mask(index: usize) -> u64 {
    UMASKS[if index >= UMASKS.len() {
        UMASKS.len() - 1
    } else {
        index
    }]
}

pub fn sign_extend(value: i64, size: usize) -> i64 {
    let mask = (!0i64).checked_shl(size as u32).unwrap_or(0);
    if (value.checked_shr(size as u32)
        .unwrap_or(if value < 0 { -1 } else { 0 }) & 1) != 0 {
        value | mask
    } else {
        value & !mask
    }
}

pub fn zero_extend(value: i64, size: usize) -> i64 {
    let mask = (!0i64).checked_shl(size as u32)
        .unwrap_or(0)
        .checked_shl(1)
        .unwrap_or(0);
    value & !mask
}

pub fn byte_swap(value: i64, size: usize) -> i64 {
    let mut res = 0i64;
    let mut val = value;
    let mut size = size;
    while size > 0 {
        res = res.checked_shl(8).unwrap_or(0);
        res |= val & 0xff;
        val = val.checked_shr(8)
            .unwrap_or(if val < 0 { -1 } else { 0 });
        size -= 1;
    }
    res
}
