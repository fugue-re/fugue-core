use byteorder::ByteOrder;
use std::cmp::Ordering;

use crate::{BE, LE};
use crate::endian::Endian;
use crate::u24;

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

    fn read_u24(buf: &[u8]) -> u24;
    fn write_u24(buf: &mut [u8], n: u24);

    fn read_isize(buf: &[u8]) -> isize;
    fn write_isize(buf: &mut [u8], n: isize);

    fn read_usize(buf: &[u8]) -> usize;
    fn write_usize(buf: &mut [u8], n: usize);

    fn subpiece(destination: &mut [u8], source: &[u8], amount: usize);
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

    fn read_u24(buf: &[u8]) -> u24 {
        let temp_u32 = u32::from_be_bytes([0, buf[0], buf[1], buf[2]]);
        u24::new(temp_u32)
    }

    fn write_u24(buf: &mut [u8], n: u24) {
        let temp = u32::from(n).to_be_bytes();
        buf[0] = temp[1];
        buf[1] = temp[2];
        buf[2] = temp[3];
    }

    fn subpiece(destination: &mut [u8], source: &[u8], amount: usize) {
        let amount = amount.min(source.len());
        let trimmed = &source[..source.len() - amount];
        match trimmed.len().cmp(&destination.len()) {
            Ordering::Less => {
                destination.copy_from_slice(&trimmed);
                for i in destination[trimmed.len()..].iter_mut() {
                    *i = 0;
                }
            }
            Ordering::Equal => {
                destination.copy_from_slice(&trimmed);
            }
            Ordering::Greater => destination.copy_from_slice(&trimmed[trimmed.len() - destination.len()..]),
        }
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

    fn read_u24(buf: &[u8]) -> u24 {
        let temp_u32 = u32::from_le_bytes([buf[0], buf[1], buf[2], 0]);
        u24::new(temp_u32)
    }

    fn write_u24(buf: &mut [u8], n: u24) {
        let temp = u32::from(n).to_le_bytes();
        buf[0] = temp[0];
        buf[1] = temp[1];
        buf[2] = temp[2];
    }

    fn subpiece(destination: &mut [u8], source: &[u8], amount: usize) {
        let amount = amount.min(source.len());
        let trimmed = &source[amount..];
        match trimmed.len().cmp(&destination.len()) {
            Ordering::Less => {
                destination[..trimmed.len()].copy_from_slice(&trimmed);
                for i in destination[trimmed.len()..].iter_mut() {
                    *i = 0;
                }
            }
            Ordering::Equal | Ordering::Greater => {
                destination.copy_from_slice(&trimmed[..destination.len()]);
            }
        }
    }
}
