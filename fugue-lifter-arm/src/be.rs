use fugue_lifter_runtime::lifter::{Language, Lifter};

mod __impl {
    #![allow(unused)]
    include!(concat!(env!("OUT_DIR"), "/arm_be.rs"));
}
pub use __impl::{context, register, space, user_op, LANGUAGE};

pub struct LifterFactory;

impl LifterFactory {
    pub fn new_v8() -> Lifter {
        let mut base = __impl::default_context();

        base.set_variable_default_by_bits(context::T_MODE, 0);
        base.set_variable_default_by_bits(context::L_RSET, 0);

        Lifter::new(&__impl::LANGUAGE, __impl::lifter_with(2, base))
    }

    pub fn new_v8t() -> Lifter {
        let mut base = __impl::default_context();

        base.set_variable_default_by_bits(context::T_MODE, 1);
        base.set_variable_default_by_bits(context::L_RSET, 0);

        Lifter::new(&__impl::LANGUAGE, __impl::lifter_with(2, base))
    }

    pub fn language(&self) -> &'static Language {
        &__impl::LANGUAGE
    }
}
