use fugue_bv::BitVec;
use fugue_bytes::traits::ByteCast;
use fugue_bytes::Order;
use fugue_ir::Address;

use paste::paste;

pub use fugue_state_derive::AsState;

pub trait StateValue: Clone + Default {
    fn from_byte(value: u8) -> Self;
}

impl<V> StateValue for V
where
    V: Clone + Default + From<u8>,
{
    #[inline(always)]
    fn from_byte(value: u8) -> Self {
        Self::from(value)
    }
}

pub trait FromStateValues<V: StateValue>: Sized {
    fn from_values<O: Order>(values: &[V]) -> Self;
}

pub trait IntoStateValues<V: StateValue>: Sized {
    fn into_values<O: Order>(self, values: &mut [V]);
}

macro_rules! impl_for {
    ($t:ident) => {
        impl FromStateValues<u8> for $t {
            #[inline(always)]
            fn from_values<O: Order>(buf: &[u8]) -> Self {
                <$t as ByteCast>::from_bytes::<O>(buf)
            }
        }

        impl IntoStateValues<u8> for $t {
            #[inline(always)]
            fn into_values<O: Order>(self, buf: &mut [u8]) {
                <$t as ByteCast>::into_bytes::<O>(&self, buf)
            }
        }
    };
}

macro_rules! impls_for {
    [$($tname:ident),*] => {
        $(
            paste! {
                impl_for!($tname);
            }
        )*
    };
}

impls_for![bool, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize];

impl FromStateValues<u8> for BitVec {
    #[inline(always)]
    fn from_values<O: Order>(values: &[u8]) -> Self {
        BitVec::from_bytes::<O>(values, false)
    }
}

impl IntoStateValues<u8> for BitVec {
    #[inline(always)]
    fn into_values<O: Order>(self, values: &mut [u8]) {
        self.into_bytes::<O>(values)
    }
}

impl IntoStateValues<u8> for &'_ BitVec {
    #[inline(always)]
    fn into_values<O: Order>(self, values: &mut [u8]) {
        if O::ENDIAN.is_big() {
            self.to_be_bytes(values)
        } else {
            self.to_le_bytes(values)
        }
    }
}

pub trait State: Clone {
    type Error: std::error::Error;

    fn fork(&self) -> Self;
    fn restore(&mut self, other: &Self);
}

pub trait StateOps: State {
    type Value: StateValue;

    fn len(&self) -> usize;

    fn copy_values<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Self::Error>
    where
        F: Into<Address>,
        T: Into<Address>;

    fn get_values<A>(&self, address: A, bytes: &mut [Self::Value]) -> Result<(), Self::Error>
    where
        A: Into<Address>;

    fn view_values<A>(&self, address: A, size: usize) -> Result<&[Self::Value], Self::Error>
    where
        A: Into<Address>;

    fn view_values_mut<A>(
        &mut self,
        address: A,
        size: usize,
    ) -> Result<&mut [Self::Value], Self::Error>
    where
        A: Into<Address>;

    fn set_values<A>(&mut self, address: A, bytes: &[Self::Value]) -> Result<(), Self::Error>
    where
        A: Into<Address>;
}

pub trait AsState<S>: State {
    fn state_ref(&self) -> &S;
    fn state_mut(&mut self) -> &mut S;
}

impl<S, T> AsState<S> for T
where
    T: State + AsRef<S> + AsMut<S>,
{
    fn state_ref(&self) -> &S {
        self.as_ref()
    }

    fn state_mut(&mut self) -> &mut S {
        self.as_mut()
    }
}

pub trait AsState2<S, T>: State + AsState<S> + AsState<T> {
    fn state2_ref(&self) -> (&S, &T) {
        (self.state_ref(), self.state_ref())
    }

    fn state2_mut(&mut self) -> (&mut S, &mut T);
}

pub trait AsState3<S, T, U>: State + AsState<S> + AsState<T> + AsState<U> {
    fn state3_ref(&self) -> (&S, &T, &U) {
        (self.state_ref(), self.state_ref(), self.state_ref())
    }

    fn state3_mut(&mut self) -> (&mut S, &mut T, &mut U);
}

pub trait AsState4<S, T, U, V>: State + AsState<S> + AsState<T> + AsState<U> + AsState<V> {
    fn state4_ref(&self) -> (&S, &T, &U, &V) {
        (
            self.state_ref(),
            self.state_ref(),
            self.state_ref(),
            self.state_ref(),
        )
    }

    fn state4_mut(&mut self) -> (&mut S, &mut T, &mut U, &mut V);
}
