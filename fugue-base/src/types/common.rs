use std::ops::Deref;

pub enum OwnedOrRef<'a, T> {
    Owned(T),
    Ref(&'a T),
}

impl<'a, T> AsRef<T> for OwnedOrRef<'a, T> {
    fn as_ref(&self) -> &T {
        match self {
            Self::Owned(ref t) => t,
            Self::Ref(t) => t,
        }
    }
}

impl<'a, T> Deref for OwnedOrRef<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<'a, T> From<&'a T> for OwnedOrRef<'a, T> {
    fn from(value: &'a T) -> Self {
        Self::Ref(value)
    }
}

impl<'a, T> From<T> for OwnedOrRef<'a, T> {
    fn from(value: T) -> Self {
        Self::Owned(value)
    }
}
