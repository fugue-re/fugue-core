//! channel module
//! 
//! implements channels with emitters/receivers of various kinds

use std::sync::mpsc;
use std::sync::Arc;

use crate::sim;
use crate::sim::Clock;

pub mod event;
pub use event::*;
pub mod error;
pub use error::Error;

/// a channel for digital signals
/// 
/// data/messages/transactions can be emitted and received on channels
/// acts as a broadcaster
/// 
/// receivers should be registered using a mspc::Sender
// if i ever decide to make simulations multi-threaded, this may need
// to be refactored a bit
pub struct Channel<T: Clone + std::fmt::Debug> {
    clock: Arc<Clock>,
    receivers: Vec<mpsc::Sender<T>>,

}

impl<T: Clone + std::fmt::Debug> Channel<T> {
    /// create a new digital channel with the given clock as a time source
    pub fn new_with(clock: Arc<Clock>) -> Self {
        Self {
            clock,
            receivers: Vec::new(),
        }
    }

    /// get shared reference to channel's clock
    pub fn clock(&self) -> &Clock {
        self.clock.as_ref()
    }

    /// create a channel logger
    pub fn get_logger(&mut self) -> ChannelLogger<T> {
        let (tx, rx) = mpsc::channel();
        self.receivers.push(tx);
        ChannelLogger::new_with(rx)
    }

    /// emit a reference to a byte slice to all receivers
    pub fn emit(&self, data: &T) -> Result<(), sim::Error> {
        for tx in self.receivers.iter() {
            tx.send(data.clone()).map_err(|err| error::Error::emit("digital", err))?;
        }
        Ok(())
    }

    /// add a receiver by registering a mspc::Sender
    /// and returning a mpsc::Receiver
    pub fn receiver(&mut self) -> mpsc::Receiver<T> {
        let (tx, rx) = mpsc::channel();
        self.receivers.push(tx);
        rx
    }
}


/// a logger for digital channel events
pub struct ChannelLogger<T: Clone + std::fmt::Debug> {
    log: Vec<T>,
    rx: mpsc::Receiver<T>,
}

impl<T: Clone + std::fmt::Debug> ChannelLogger<T> {
    
    pub fn new_with(rx: mpsc::Receiver<T>) -> Self {
        Self {
            log: Vec::new(),
            rx,
        }
    }

    /// get a slice of the current log
    pub fn log(&self) -> &[T] {
        &self.log
    }

    /// collect pending events into the log
    pub fn collect_pending(&mut self) {
        for event in self.rx.try_iter() {
            self.log.push(event.clone())
        }
    }

    /// collect the pending events into the log and
    /// return a string representing the collected events
    pub fn display_pending(&mut self) -> String {
        let mut log = String::new();
        for event in self.rx.try_iter() {
            self.log.push(event.clone());
            log = format!("{log}\n{event:?}");
        }
        log
    }
}
