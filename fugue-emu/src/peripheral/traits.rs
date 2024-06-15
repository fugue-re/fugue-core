//! peripheral traits
//! 
//! defines various traits for the peripheral module
//! 
//! peripherals should be designed essentially as stateful callbacks
//! so when registering a peripheral with a context, the context should
//! get a mutable reference to the peripheral struct which must implement
//! a read/write varnode callback

// use crate::eval::traits::MappedContext;

// pub trait Peripheral: MappedContext {

// }