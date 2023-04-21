use paste::paste;
use crate::u24;

use crate::order::Order;

pub trait ByteCast: Copy {
    const SIZEOF: usize;
    const SIGNED: bool;

    fn from_bytes<O: Order>(buf: &[u8]) -> Self;
    fn into_bytes<O: Order>(&self, buf: &mut [u8]);
}

macro_rules! impl_for {
    ($t:ident, $read:ident, $write:ident, $signed:ident) => {
        impl ByteCast for $t {
            const SIZEOF: usize = std::mem::size_of::<$t>();
            const SIGNED: bool = $signed;

            fn from_bytes<O: Order>(buf: &[u8]) -> Self {
                O::$read(buf)
            }

            fn into_bytes<O: Order>(&self, buf: &mut [u8]) {
                O::$write(buf, *self)
            }
        }
    };
}

macro_rules! impls_for {
    ([$($tname:ident),*], $signed:ident) => {
        $(
            paste! {
                impl_for!($tname, [<read_ $tname>], [<write_ $tname>], $signed);
            }
        )*
    };
}

impl ByteCast for bool {
    const SIZEOF: usize = 1;
    const SIGNED: bool = false;

    fn from_bytes<O: Order>(buf: &[u8]) -> Self {
        !buf.is_empty() && buf[0] != 0
    }

    fn into_bytes<O: Order>(&self, buf: &mut [u8]) {
        O::write_u8(buf, if *self { 1 } else { 0 })
    }
}

impls_for! { [i8, i16, i32, i64, i128, isize], true }
impls_for! { [u8, u16, u32, u64, u128, usize], false }


impl ByteCast for u24 {
    const SIZEOF: usize = std::mem::size_of::<u24>();
    const SIGNED: bool = false;

    fn from_bytes<O: Order>(buf: &[u8]) -> Self {
        <O as Order>::read_u24(buf)
    }

    fn into_bytes<O: Order>(&self, buf: &mut [u8]) {
        <O as Order>::write_u24(buf, *self)
    }
}