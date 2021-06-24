use byteorder::ByteOrder;

use crate::{BE, LE};
use crate::endian::Endian;

pub trait Order: ByteOrder + Send + Sync + 'static {
    const ENDIAN: Endian;
    const NATIVE: bool;

    fn read_i8(buf: &[u8]) -> i8 {
        if buf.is_empty() {
            0
        } else {
            buf[0] as i8
        }
    }

    fn write_i8(buf: &mut [u8], n: i8) {
        if !buf.is_empty() {
            buf[0] = n as u8;
        }
    }

    fn read_u8(buf: &[u8]) -> u8 {
        if buf.is_empty() {
            0
        } else {
            buf[0]
        }
    }

    fn write_u8(buf: &mut [u8], n: u8) {
        if !buf.is_empty() {
            buf[0] = n;
        }
    }

    fn read_isize(buf: &[u8]) -> isize;
    fn write_isize(buf: &mut [u8], n: isize);

    fn read_usize(buf: &[u8]) -> usize;
    fn write_usize(buf: &mut [u8], n: usize);
}

impl Order for BE {
    const ENDIAN: Endian = Endian::Big;
    const NATIVE: bool = cfg!(target_endian = "big");

    #[cfg(target_pointer_width = "32")]
    fn read_isize(buf: &[u8]) -> isize {
        Self::read_i32(buf) as isize
    }

    #[cfg(target_pointer_width = "64")]
    fn read_isize(buf: &[u8]) -> isize {
        Self::read_i64(buf) as isize
    }

    #[cfg(target_pointer_width = "32")]
    fn write_isize(buf: &mut [u8], n: isize) {
        Self::write_i32(buf, n as i32)
    }

    #[cfg(target_pointer_width = "64")]
    fn write_isize(buf: &mut [u8], n: isize) {
        Self::write_i64(buf, n as i64)
    }

    #[cfg(target_pointer_width = "32")]
    fn read_usize(buf: &[u8]) -> usize {
        Self::read_u32(buf) as usize
    }

    #[cfg(target_pointer_width = "64")]
    fn read_usize(buf: &[u8]) -> usize {
        Self::read_u64(buf) as usize
    }

    #[cfg(target_pointer_width = "32")]
    fn write_usize(buf: &mut [u8], n: usize) {
        Self::write_u32(buf, n as u32)
    }

    #[cfg(target_pointer_width = "64")]
    fn write_usize(buf: &mut [u8], n: usize) {
        Self::write_u64(buf, n as u64)
    }
}

impl Order for LE {
    const ENDIAN: Endian = Endian::Little;
    const NATIVE: bool = cfg!(target_endian = "little");

    #[cfg(target_pointer_width = "32")]
    fn read_isize(buf: &[u8]) -> isize {
        Self::read_i32(buf) as isize
    }

    #[cfg(target_pointer_width = "64")]
    fn read_isize(buf: &[u8]) -> isize {
        Self::read_i64(buf) as isize
    }

    #[cfg(target_pointer_width = "32")]
    fn write_isize(buf: &mut [u8], n: isize) {
        Self::write_i32(buf, n as i32)
    }

    #[cfg(target_pointer_width = "64")]
    fn write_isize(buf: &mut [u8], n: isize) {
        Self::write_i64(buf, n as i64)
    }

    #[cfg(target_pointer_width = "32")]
    fn read_usize(buf: &[u8]) -> usize {
        Self::read_u32(buf) as usize
    }

    #[cfg(target_pointer_width = "64")]
    fn read_usize(buf: &[u8]) -> usize {
        Self::read_u64(buf) as usize
    }

    #[cfg(target_pointer_width = "32")]
    fn write_usize(buf: &mut [u8], n: usize) {
        Self::write_u32(buf, n as u32)
    }

    #[cfg(target_pointer_width = "64")]
    fn write_usize(buf: &mut [u8], n: usize) {
        Self::write_u64(buf, n as u64)
    }
}
