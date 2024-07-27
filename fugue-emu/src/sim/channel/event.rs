//! channel events
//! 
//! specifies events for particular channel types

use crate::sim;

/// events that can be sent over a Digital Channel
/// 
/// Hi represents logic high (1) that occurs at sim::Time
/// Lo represents logic low (0) that occurs at sim::Time
#[derive(Clone, Copy, PartialEq, Eq, std::fmt::Debug)]
pub enum Digital {
    Hi(sim::Time),
    Lo(sim::Time),
}