use std::marker::PhantomData;
use std::sync::Arc;

use fugue_bytes::{ByteCast, Order};

use fugue_ir::convention::Convention;
use fugue_ir::{Address, AddressSpace, Translator, VarnodeData};

use thiserror::Error;

use crate::paged::{self, PagedState};
use crate::register::{self, RegisterState};
use crate::unique::{self, UniqueState};

use crate::traits::{FromStateValues, IntoStateValues};
use crate::traits::{State, StateOps, StateValue};

pub const POINTER_8_SIZE: usize = 1;
pub const POINTER_16_SIZE: usize = 2;
pub const POINTER_32_SIZE: usize = 4;
pub const POINTER_64_SIZE: usize = 8;
pub const MAX_POINTER_SIZE: usize = POINTER_64_SIZE;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Memory(paged::Error),
    #[error(transparent)]
    Register(register::Error),
    #[error(transparent)]
    Temporary(unique::Error),
    #[error("unsupported addess size of `{0}` bytes")]
    UnsupportedAddressSize(usize),
}

#[derive(Debug, Clone)]
pub struct PCodeState<T: StateValue, O: Order> {
    memory: PagedState<T>,
    registers: RegisterState<T, O>,
    temporaries: UniqueState<T>,
    convention: Convention,
    marker: PhantomData<O>,
}

impl<T: StateValue, O: Order> AsRef<Self> for PCodeState<T, O> {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T: StateValue, O: Order> AsMut<Self> for PCodeState<T, O> {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T: StateValue, O: Order> PCodeState<T, O> {
    pub fn new(memory: PagedState<T>, translator: &Translator, convention: &Convention) -> Self {
        Self {
            memory,
            registers: RegisterState::new(translator, convention),
            temporaries: UniqueState::new(translator),
            convention: convention.clone(),
            marker: PhantomData,
        }
    }

    pub fn convention(&self) -> &Convention {
        &self.convention
    }

    pub fn memory(&self) -> &PagedState<T> {
        &self.memory
    }

    pub fn memory_mut(&mut self) -> &mut PagedState<T> {
        &mut self.memory
    }

    pub fn memory_space(&self) -> Arc<AddressSpace> {
        self.memory().address_space()
    }

    pub fn memory_space_ref(&self) -> &AddressSpace {
        self.memory().address_space_ref()
    }

    pub fn registers(&self) -> &RegisterState<T, O> {
        &self.registers
    }

    pub fn registers_mut(&mut self) -> &mut RegisterState<T, O> {
        &mut self.registers
    }

    pub fn temporaries(&self) -> &UniqueState<T> {
        &self.temporaries
    }

    pub fn temporaries_mut(&mut self) -> &mut UniqueState<T> {
        &mut self.temporaries
    }

    pub fn with_operand_values<U, F>(&self, operand: &VarnodeData, f: F) -> Result<U, Error>
    where
        F: FnOnce(&[T]) -> U,
    {
        let value = operand.offset();
        let size = operand.size();

        let space = operand.space();

        if space.is_constant() {
            // max size of value
            let mut values: [T; 8] = Default::default();

            if O::ENDIAN.is_big() {
                for (d, s) in values[..size]
                    .iter_mut()
                    .zip(&value.to_be_bytes()[8 - size..])
                {
                    *d = T::from_byte(*s);
                }
            } else {
                for (d, s) in values[..size].iter_mut().zip(&value.to_le_bytes()[..size]) {
                    *d = T::from_byte(*s);
                }
            }

            Ok(f(&values[..size]))
        } else if space.is_register() {
            self.registers()
                .view_values(value, size)
                .map_err(Error::Register)
                .map(f)
        } else if space.is_unique() {
            self.temporaries()
                .view_values(value, size)
                .map_err(Error::Temporary)
                .map(f)
        } else {
            self.memory()
                .view_values(value, size)
                .map_err(Error::Memory)
                .map(f)
        }
    }

    pub fn with_operand_values_mut<U, F>(&mut self, operand: &VarnodeData, f: F) -> Result<U, Error>
    where
        F: FnOnce(&mut [T]) -> U,
    {
        let value = operand.offset();
        let size = operand.size();

        let space = operand.space();

        if space.is_constant() {
            panic!("cannot mutate constant operand");
        } else if space.is_register() {
            self.registers_mut()
                .view_values_mut(value, size)
                .map_err(Error::Register)
                .map(f)
        } else if space.is_unique() {
            self.temporaries_mut()
                .view_values_mut(value, size)
                .map_err(Error::Temporary)
                .map(f)
        } else {
            self.memory_mut()
                .view_values_mut(value, size)
                .map_err(Error::Memory)
                .map(f)
        }
    }

    pub fn get_operand<V: FromStateValues<T>>(&self, operand: &VarnodeData) -> Result<V, Error> {
        let res = self.with_operand_values(operand, |values| V::from_values::<O>(values));
        res
    }

    pub fn set_operand<V: IntoStateValues<T>>(
        &mut self,
        operand: &VarnodeData,
        value: V,
    ) -> Result<(), Error> {
        self.with_operand_values_mut(operand, |values| value.into_values::<O>(values))
    }

    #[inline(always)]
    pub fn view_values_from<A>(&self, address: A) -> Result<&[T], Error>
    where
        A: Into<Address>,
    {
        self.memory.view_values_from(address).map_err(Error::Memory)
    }
}

impl<O: Order> PCodeState<u8, O> {
    pub fn program_counter_value(&self) -> Result<Address, Error> {
        self.get_address(&self.registers.program_counter())
    }

    pub fn stack_pointer_value(&self) -> Result<Address, Error> {
        self.get_address(&self.registers.stack_pointer())
    }

    pub fn get_pointer(&self, address: Address) -> Result<Address, Error> {
        let opnd = VarnodeData::new(
            self.memory_space_ref(),
            address.offset(),
            self.memory_space_ref().address_size(),
        );
        self.get_address(&opnd)
    }

    // get value at address
    pub fn get_address(&self, operand: &VarnodeData) -> Result<Address, Error> {
        let mut buf = [0u8; MAX_POINTER_SIZE];
        let size = operand.size();

        let address = if size == POINTER_64_SIZE {
            self.with_operand_values(operand, |values| {
                buf[..size].copy_from_slice(values);
                u64::from_bytes::<O>(values)
            })
        } else if size == POINTER_32_SIZE {
            self.with_operand_values(operand, |values| {
                buf[..size].copy_from_slice(values);
                u32::from_bytes::<O>(values) as u64
            })
        } else if size == POINTER_16_SIZE {
            self.with_operand_values(operand, |values| {
                buf[..size].copy_from_slice(values);
                u16::from_bytes::<O>(values) as u64
            })
        } else if size == POINTER_8_SIZE {
            self.with_operand_values(operand, |values| {
                buf[..size].copy_from_slice(values);
                u8::from_bytes::<O>(values) as u64
            })
        } else {
            return Err(Error::UnsupportedAddressSize(size));
        }?;

        Ok(Address::new(self.memory.address_space_ref(), address))
    }

    pub fn set_program_counter_value<A>(&mut self, value: A) -> Result<(), Error>
    where
        A: Into<Address>,
    {
        self.set_address(&self.registers.program_counter(), value)
    }

    pub fn set_stack_pointer_value<A>(&mut self, value: A) -> Result<(), Error>
    where
        A: Into<Address>,
    {
        self.set_address(&self.registers.stack_pointer(), value)
    }

    pub fn set_address<A>(&mut self, operand: &VarnodeData, value: A) -> Result<(), Error>
    where
        A: Into<Address>,
    {
        let size = operand.size();
        let address = value.into();

        if size == POINTER_64_SIZE {
            self.with_operand_values_mut(operand, |values| {
                u64::from(address).into_bytes::<O>(values)
            })
        } else if size == POINTER_32_SIZE {
            self.with_operand_values_mut(operand, |values| {
                u32::from(address).into_bytes::<O>(values)
            })
        } else if size == POINTER_16_SIZE {
            self.with_operand_values_mut(operand, |values| {
                u16::from(address).into_bytes::<O>(values)
            })
        } else if size == POINTER_8_SIZE {
            self.with_operand_values_mut(operand, |values| {
                u8::from(address).into_bytes::<O>(values)
            })
        } else {
            return Err(Error::UnsupportedAddressSize(size));
        }?;

        Ok(())
    }
}

impl<V: StateValue, O: Order> State for PCodeState<V, O> {
    type Error = Error;

    fn fork(&self) -> Self {
        Self {
            convention: self.convention.clone(),
            registers: self.registers.fork(),
            temporaries: self.temporaries.fork(),
            memory: self.memory.fork(),
            marker: self.marker,
        }
    }

    fn restore(&mut self, other: &Self) {
        self.registers.restore(&other.registers);
        self.temporaries.restore(&other.temporaries);
        self.memory.restore(&other.memory);
    }
}

impl<V: StateValue, O: Order> StateOps for PCodeState<V, O> {
    type Value = V;

    #[inline(always)]
    fn copy_values<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Self::Error>
    where
        F: Into<Address>,
        T: Into<Address>,
    {
        self.memory
            .copy_values(from, to, size)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn get_values<A>(&self, address: A, values: &mut [Self::Value]) -> Result<(), Self::Error>
    where
        A: Into<Address>,
    {
        self.memory
            .get_values(address, values)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn view_values<A>(&self, address: A, size: usize) -> Result<&[Self::Value], Self::Error>
    where
        A: Into<Address>,
    {
        self.memory
            .view_values(address, size)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn view_values_mut<A>(
        &mut self,
        address: A,
        size: usize,
    ) -> Result<&mut [Self::Value], Self::Error>
    where
        A: Into<Address>,
    {
        self.memory
            .view_values_mut(address, size)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn set_values<A>(&mut self, address: A, values: &[Self::Value]) -> Result<(), Self::Error>
    where
        A: Into<Address>,
    {
        self.memory
            .set_values(address, values)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.memory.len()
    }
}
