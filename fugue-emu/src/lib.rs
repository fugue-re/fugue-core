//! fugue-emu
//! 
//! an emulation library for fugue

pub mod emu;
pub mod context;
pub mod peripheral;
pub mod eval;


#[cfg(test)]
mod tests {
    #[allow(unused)]
    use super::*;

    // a program that computes (((3 ** 2) ** 2) ** 2)
    // compiled with xpack arm-none-eabi-gcc arm64 11.3.1 20220712
    // arm-none-eabi-gcc main.c -mcpu=cortex-m4 -mthumb -nostdlib
    pub static TEST_PROGRAM: &[u8] = &[
        // 00000000 <_start>:
        0x00, 0xf0, 0x01, 0xf8, //  0: bl  6 <main>
        // 00000004 <exit>:
        0xfe, 0xe7,             //  4: b.n 4 <exit>
        // 00000006 <main>:
        0x80, 0xb5,             //  6: push     {r7, lr}
        0x82, 0xb0,             //  8: sub      sp, #8
        0x00, 0xaf,             //  a: add      r7, sp, #0
        0x03, 0x23,             //  c: movs     r3, #3
        0x7b, 0x60,             //  e: str      r3, [r7, #4]
        0x00, 0x23,             // 10: movs     r3, #0
        0x3b, 0x60,             // 12: str      r3, [r7, #0]
        0x06, 0xe0,             // 14: b.n      24 <main+0x1e>
        0x78, 0x68,             // 16: ldr      r0, [r7, #4]
        0x00, 0xf0, 0x0c, 0xf8, // 18: bl       34 <square>
        0x78, 0x60,             // 1c: str      r0, [r7, #4]
        0x3b, 0x68,             // 1e: ldr      r3, [r7, #0]
        0x01, 0x33,             // 20: adds     r3, #1
        0x3b, 0x60,             // 22: str      r3, [r7, #0]
        0x3b, 0x68,             // 24: ldr      r3, [r7, #0]
        0x02, 0x2b,             // 26: cmp      r3, #2
        0xf5, 0xdd,             // 28: ble.n    16 <main+0x10>
        0x7b, 0x68,             // 2a: ldr      r3, [r7, #4]
        0x18, 0x46,             // 2c: mov      r0, r3
        0x08, 0x37,             // 2e: adds     r7, #8
        0xbd, 0x46,             // 30: mov      sp, r7
        0x80, 0xbd,             // 32: pop      {r7, pc}
        // 00000034 <square>:
        0x80, 0xb4,             // 34: push     {r7}
        0x83, 0xb0,             // 36: sub      sp, #12
        0x00, 0xaf,             // 38: add      r7, sp, #0
        0x78, 0x60,             // 3a: str      r0, [r7, #4]
        0x7b, 0x68,             // 3c: ldr      r3, [r7, #4]
        0x03, 0xfb, 0x03, 0xf3, // 3e: mul.w    r3, r3, r3
        0x18, 0x46,             // 42: mov      r0, r3
        0x0c, 0x37,             // 44: adds     r7, #12
        0xbd, 0x46,             // 46: mov      sp, r7
        0x80, 0xbc,             // 48: pop      {r7}
        0x70, 0x47,             // 4a: bx       lr
    ];

    #[test]
    fn it_works() {
        
    }
}
