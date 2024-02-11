use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use fugue_bytes::Order;
use fugue_ir::convention::{Convention, ReturnAddress};
use fugue_ir::il::pcode::{Operand, Register};
use fugue_ir::register::RegisterNames;
use fugue_ir::{Address, Translator};

use crate::{FromStateValues, IntoStateValues, State, StateOps, StateValue};
use crate::flat::FlatState;

pub use crate::flat::Error;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum ReturnLocation {
    Register(Operand),
    Relative(Operand, u64),
}

impl ReturnLocation {
    pub fn from_convention(translator: &Translator, convention: &Convention) -> Self {
        match convention.return_address() {
            ReturnAddress::Register { varnode, .. } => {
                Self::Register(Operand::from_varnode(translator, *varnode))
            },
            ReturnAddress::StackRelative { offset, .. } => {
                Self::Relative(
                    Operand::from_varnode(translator, *convention.stack_pointer().varnode()),
                    *offset,
                )
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegisterState<T: StateValue, O: Order> {
    program_counter: Arc<Operand>,
    stack_pointer: Arc<Operand>,
    register_names: Arc<RegisterNames>,
    return_location: Arc<ReturnLocation>,
    inner: FlatState<T>,
    marker: PhantomData<O>,
}

impl<T: StateValue, O: Order> AsRef<Self> for RegisterState<T, O> {
    #[inline(always)]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T: StateValue, O: Order> AsMut<Self> for RegisterState<T, O> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T: StateValue, O: Order> AsRef<FlatState<T>> for RegisterState<T, O> {
    #[inline(always)]
    fn as_ref(&self) -> &FlatState<T> {
        &self.inner
    }
}

impl<T: StateValue, O: Order> AsMut<FlatState<T>> for RegisterState<T, O> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut FlatState<T> {
        &mut self.inner
    }
}

impl<T: StateValue, O: Order> Deref for RegisterState<T, O> {
    type Target = FlatState<T>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: StateValue, O: Order> DerefMut for RegisterState<T, O> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: StateValue, O: Order> From<RegisterState<T, O>> for FlatState<T> {
    fn from(t: RegisterState<T, O>) -> Self {
        t.inner
    }
}

impl<V: StateValue, O: Order> State for RegisterState<V, O> {
    type Error = Error;

    #[inline(always)]
    fn fork(&self) -> Self {
        Self {
            inner: self.inner.fork(),
            stack_pointer: self.stack_pointer.clone(),
            program_counter: self.program_counter.clone(),
            return_location: self.return_location.clone(),
            register_names: self.register_names.clone(),
            marker: PhantomData,
        }
    }

    #[inline(always)]
    fn restore(&mut self, other: &Self) {
        self.inner.restore(&other.inner)
    }
}

impl<V: StateValue, O: Order> StateOps for RegisterState<V, O> {
    type Value = V;

    #[inline(always)]
    fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline(always)]
    fn copy_values<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Self::Error>
    where F: Into<Address>,
          T: Into<Address> {
        self.inner.copy_values(from, to, size)
    }

    #[inline(always)]
    fn get_values<A>(&self, address: A, bytes: &mut [Self::Value]) -> Result<(), Self::Error>
    where A: Into<Address> {
        self.inner.get_values(address, bytes)
    }

    #[inline(always)]
    fn view_values<A>(&self, address: A, size: usize) -> Result<&[Self::Value], Self::Error>
    where A: Into<Address> {
        self.inner.view_values(address, size)
    }

    #[inline(always)]
    fn view_values_mut<A>(&mut self, address: A, size: usize) -> Result<&mut [Self::Value], Self::Error>
    where A: Into<Address> {
        self.inner.view_values_mut(address, size)
    }

    #[inline(always)]
    fn set_values<A>(&mut self, address: A, bytes: &[Self::Value]) -> Result<(), Self::Error>
    where A: Into<Address> {
        self.inner.set_values(address, bytes)
    }
}

impl<T: StateValue, O: Order> RegisterState<T, O> {
    pub fn new(translator: &Translator, convention: &Convention) -> Self {
        let program_counter = Arc::new(Operand::from_varnode(translator, *translator.program_counter()));
        let stack_pointer = Arc::new(Operand::from_varnode(translator, *convention.stack_pointer().varnode()));
        let return_location = Arc::new(ReturnLocation::from_convention(translator, convention));

        let space = translator.manager().register_space();
        let register_names = translator.registers().clone();
        let size = translator.register_space_size();

        log::debug!("register space size: {} bytes", size);

        Self {
            inner: FlatState::new(space, size),
            program_counter,
            stack_pointer,
            return_location,
            register_names,
            marker: PhantomData,
        }
    }

    pub fn program_counter(&self) -> Arc<Operand> {
        self.program_counter.clone()
    }

    pub fn stack_pointer(&self) -> Arc<Operand> {
        self.stack_pointer.clone()
    }

    pub fn return_location(&self) -> Arc<ReturnLocation> {
        self.return_location.clone()
    }

    pub fn get_register_values(&self, register: &Register, values: &mut [T]) -> Result<(), Error> {
        let view = self.view_values(register.offset(), register.size())?;
        values.clone_from_slice(view);
        Ok(())
    }

    pub fn register_names(&self) -> &Arc<RegisterNames> {
        &self.register_names
    }

    pub fn register_by_name<N>(&self, name: N) -> Option<Register>
    where N: AsRef<str> {
        self.register_names
            .get_by_name(name)
            .map(|(name, offset, size)| Register::new(name.clone(), offset, size))
    }

    pub fn get_register<V: FromStateValues<T>>(&self, register: &Register) -> Result<V, Error> {
        Ok(V::from_values::<O>(self.view_values(register.offset(), register.size())?))
    }

    pub fn set_register_values(&mut self, register: &Register, values: &[T]) -> Result<(), Error> {
        let view = self.view_values_mut(register.offset(), register.size())?;
        view.clone_from_slice(values);
        Ok(())
    }

    pub fn set_register<V: IntoStateValues<T>>(&mut self, register: &Register, value: V) -> Result<(), Error> {
        value.into_values::<O>(self.view_values_mut(register.offset(), register.size())?);
        Ok(())
    }
}
