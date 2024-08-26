//! digital signal channel
//! 

use std::sync::mpsc;
use std::sync::Arc;

use crate::sim;
use crate::sim::Clock;

use super::logger::ChannelLogger;

/// events that can be sent over a Digital Channel
#[derive(Clone, Copy, PartialEq, std::fmt::Debug)]
pub enum Event {
    /// Hi represents logic high (1) that occurs at sim::Time
    Hi(sim::Time),
    /// Lo represents logic low (0) that occurs at sim::Time
    Lo(sim::Time),
}


/// a channel for digital signals
/// 
/// data/messages/transactions can be emitted and received on channels
/// acts as a broadcaster
/// 
/// receivers should be registered using a mspc::Sender
// if i ever decide to make simulations multi-threaded, this may need
// to be refactored a bit
pub struct Channel {
    clock: Arc<Clock>,
    receivers: Vec<mpsc::Sender<Event>>,
}

impl Channel {
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
    pub fn get_logger(&mut self) -> ChannelLogger<Event> {
        let (tx, rx) = mpsc::channel();
        self.receivers.push(tx);
        ChannelLogger::new_with(rx)
    }

    /// emit a reference to a digital event to all receivers
    pub fn emit(&self, data: &Event) -> Result<(), sim::Error> {
        for tx in self.receivers.iter() {
            tx.send(data.clone()).map_err(|err| super::Error::emit("digital", err))?;
        }
        Ok(())
    }

    /// add a receiver by registering a mspc::Sender
    /// and returning a mpsc::Receiver
    pub fn receiver(&mut self) -> mpsc::Receiver<Event> {
        let (tx, rx) = mpsc::channel();
        self.receivers.push(tx);
        rx
    }
}


