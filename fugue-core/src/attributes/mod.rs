use std::ops::Deref;

use rustc_hash::FxHashMap;
use uuid::Uuid;

pub use gazebo::any::{AnyLifetime, ProvidesStaticType};

pub mod common;
pub mod platform;

pub struct Attributes<'a> {
    attrs: FxHashMap<Uuid, Box<dyn AnyLifetime<'a>>>,
}

pub trait Attribute<'a>: AnyLifetime<'a> {
    const UUID: Uuid;
}

impl<'a> Attributes<'a> {
    pub fn new() -> Self {
        Self {
            attrs: FxHashMap::default(),
        }
    }

    pub fn set_attr<T>(&mut self, value: T)
    where
        T: Attribute<'a>,
    {
        self.attrs.insert(T::UUID, Box::new(value));
    }

    pub fn get_attr<T>(&self) -> Option<&T>
    where
        T: Attribute<'a>,
    {
        self.attrs.get(&T::UUID).and_then(|v| v.downcast_ref())
    }

    pub fn get_attr_as<T, U>(&self) -> Option<&U>
    where
        T: Attribute<'a> + AsRef<U>,
        U: ?Sized,
    {
        self.get_attr::<T>().map(T::as_ref)
    }

    pub fn get_attr_as_deref<T, U>(&self) -> Option<&U>
    where
        T: Attribute<'a> + Deref<Target = U>,
        U: ?Sized,
    {
        self.get_attr::<T>().map(T::deref)
    }

    pub fn get_attr_mut<T>(&mut self) -> Option<&mut T>
    where
        T: Attribute<'a>,
    {
        self.attrs.get_mut(&T::UUID).and_then(|v| v.downcast_mut())
    }
}
