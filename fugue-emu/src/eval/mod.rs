//! eval
//! 
//! the eval module defines evaluators to be used with emulation contexts

pub mod traits;
pub mod error;
pub mod types;

pub use error::*;
pub use types::*;

pub mod concrete;
