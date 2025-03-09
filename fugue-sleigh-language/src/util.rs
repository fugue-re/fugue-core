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

pub(crate) fn calculate_mask(index: usize) -> u64 {
    UMASKS[if index >= UMASKS.len() {
        UMASKS.len() - 1
    } else {
        index
    }]
}

pub(crate) fn zero_extend(value: i64, size: usize) -> i64 {
    let mask = (!0i64).checked_shl(size as u32)
        .unwrap_or(0)
        .checked_shl(1)
        .unwrap_or(0);
    value & !mask
}
