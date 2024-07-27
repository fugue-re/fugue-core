//! simulation module
//! 
//! the simulation module contains functionality for 
//! managing simulation components

pub mod channel;
pub mod traits;
pub mod error;
pub mod types;

pub use error::*;
pub use traits::*;
pub use types::*;

/// global minimum simulation time resolution
pub const MIN_QUANT: f64 = 1e-6;

