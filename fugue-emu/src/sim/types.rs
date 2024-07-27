//! simulation types
//! 
//! various types used across the simulation module

use crate::sim;

pub type Time = usize;

/// a simulation clock is a time source
/// 
#[derive(Clone)]
pub struct Clock {
    resolution: f64,
    elapsed: Time,
}

impl Clock {
    /// create a new clock (time source)
    pub fn new() -> Self {
        Self {
            resolution: sim::MIN_QUANT,
            elapsed: 0usize,
        }
    }

    /// create a new clock with specified resolution in seconds
    /// 
    /// errors if resolution is lower than minimum
    pub fn new_with(resolution: f64) -> Result<Self, sim::Error> {
        if resolution < sim::MIN_QUANT {
            return Err(sim::Error::Clock(
                format!("failed to create clock with resolution {}", resolution)));
        }
        Ok(Self {
            resolution,
            elapsed: 0usize,
        })
    }

    /// get clock resolution in seconds
    pub fn resolution(&self) -> f64 {
        self.resolution
    }

    /// get elapsed time in ticks since instantiation
    pub fn ticks_elapsed(&self) -> Time {
        self.elapsed
    }

    /// increment elapsed time by one
    pub fn tick(&mut self) {
        self.elapsed += 1
    }

    /// increment elapsed time by n
    pub fn ticks(&mut self, n: Time) {
        self.elapsed += n
    }

    /// get elapsed time in virtual seconds
    pub fn elapsed_seconds(&self) -> f64 {
        (self.elapsed as f64) * self.resolution
    }
}
