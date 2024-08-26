//! channel logger module
//! 

use std::sync::mpsc;

/// a logger for channel events
pub struct ChannelLogger<Event: Clone + std::fmt::Debug> {
    log: Vec<Event>,
    rx: mpsc::Receiver<Event>,
}

impl<Event: Clone + std::fmt::Debug> ChannelLogger<Event> {
    
    pub fn new_with(rx: mpsc::Receiver<Event>) -> Self {
        Self {
            log: Vec::new(),
            rx,
        }
    }

    /// get a slice of the current log
    pub fn log(&self) -> &[Event] {
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
