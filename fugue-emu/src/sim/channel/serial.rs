//! serial channel
//! 
//! TX/RX channel

use std::sync::mpsc;
use std::sync::Arc;

use crate::sim;
use crate::sim::Clock;

use super::logger::ChannelLogger;

/// serial channel config
#[derive(Clone, Copy, PartialEq, std::fmt::Debug)]
pub struct Config {
    /// idle voltage level
    voltage: f32,
    /// baud rate
    baud: u32,
    /// parity bit
    parity: bool,
    /// data bits size
    data_bits: u8,
    /// stop bits size
    stop_bits: u8,
}

/// serial channel events
/// 
#[derive(Clone, Copy, PartialEq, std::fmt::Debug)]
pub enum Event {
    /// indicates the start of a new serial transaction
    Start(sim::Time),
    /// bytes sent via serial
    Data(u8),
}

/// serial channel for tx/rx between 2 devices
/// that sends/receives bytes only
/// 
/// ignoring flow control (RTS/CTS)
pub struct Channel {
    clock: Arc<Clock>,
    devices: (Option<mpsc::Sender<Event>>, Option<mpsc::Sender<Event>>),
    logger: Option<mpsc::Sender<Event>>,
}

impl Channel {
    /// create new serial channel with provided clock as time source
    pub fn new_with(clock: Arc<Clock>) -> Self {
        Self {
            clock,
            devices: (None, None),
            logger: None,
        }
    }

    /// get shared reference to channel's clock
    pub fn clock(&self) -> &Clock {
        self.clock.as_ref()
    }

    /// emit data to the receiver
    pub fn emit(&self, id: usize, data: &[u8]) -> Result<(), sim::Error> {
        let device = match id {
            0 => &self.devices.0,
            1 => &self.devices.1,
            _ => &None,
        };
        let Some(tx) = device else {
            return Err(super::Error::emit(
                "serial", 
                format!("device {id} not connected")).into());
        };

        tx.send(Event::Start(self.clock.ticks_elapsed()))
            .map_err(|err| super::Error::emit("serial", err))?;
        for byte in data.iter() {
            tx.send(Event::Data(byte.clone()))
                .map_err(|err| super::Error::emit("serial", err))?;
        }
        Ok(())
    }

    /// add a receiver by registering a mspc::Sender
    /// and returning a mpsc::Receiver
    pub fn receiver(&mut self) -> Option<(usize, mpsc::Receiver<Event>)> {
        let (tx, rx) = mpsc::channel();

        if self.devices.0.is_none() {
            self.devices.0 = Some(tx);
            Some((0, rx))
        } else if self.devices.1.is_none() {
            self.devices.1 = Some(tx);
            Some((1, rx))
        } else {
            None
        }
    }

    /// create a channel logger
    pub fn get_logger(&mut self) -> Option<ChannelLogger<Event>> {
        if self.logger.is_none() {
            let (tx, rx) = mpsc::channel();
            self.logger = Some(tx);
            Some(ChannelLogger::new_with(rx))
        } else {
            None
        }
    }
}
