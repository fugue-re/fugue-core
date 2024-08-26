//! channel module
//! 
//! implements various channel types for i/o

use crate::sim;

pub mod error;
pub use error::Error;

pub mod logger;
pub mod digital;
pub mod serial;
pub mod i2c;