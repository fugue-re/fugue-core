use std::borrow::Cow;

use gazebo::any::ProvidesStaticType;
use uuid::{uuid, Uuid};

use super::Attribute;

#[derive(ProvidesStaticType)]
pub struct CompilerConvention(Cow<'static, str>);

impl CompilerConvention {
    pub fn new(convention: impl Into<Cow<'static, str>>) -> Self {
        Self(convention.into())
    }
}

impl AsRef<str> for CompilerConvention {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl<'a> Attribute<'a> for CompilerConvention {
    const UUID: Uuid = uuid!("9D5D5EFA-3985-45C5-B803-49217FE3184D");
}
