//! peripheral module
//! 
//! defines common emulation peripherals

use fugue_ir::Address;

use crate::context::traits::MappedContext;

pub mod traits;
pub mod error;
pub mod generic;

use traits::*;
pub use error::*;

