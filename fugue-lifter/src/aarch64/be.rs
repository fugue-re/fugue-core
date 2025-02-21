use crate::runtime::lifter::{Language, Lifter};

mod __impl {
    #![allow(unused)]
    include!(concat!(env!("OUT_DIR"), "/aarch64_be.rs"));
}
pub use __impl::{context, register, space, user_op, LANGUAGE};

pub struct LifterFactory;

impl LifterFactory {
    pub fn new_v8a() -> Lifter {
        Lifter::new(
            &__impl::LANGUAGE,
            __impl::lifter_with(2, __impl::default_context()),
        )
    }

    pub fn language(&self) -> &'static Language {
        &__impl::LANGUAGE
    }
}
