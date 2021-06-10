pub use byteorder::{BE, LE};
pub use byteorder::NativeEndian as NE;

use byteorder::ByteOrder;

pub mod endian;
pub use endian::Endian;
