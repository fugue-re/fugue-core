use fugue_lifter_runtime::lifter::Lifter;

mod __impl {
    #![allow(unused)]
    include!(concat!(env!("OUT_DIR"), "/x86_64.rs"));
}

pub use __impl::{context, register, space, user_op, LANGUAGE};

pub struct LifterFactory;

impl LifterFactory {
    pub fn new_default() -> Lifter {
        let mut base = __impl::default_context();

        base.set_variable_default_by_bits(context::ADDRSIZE, 2);
        base.set_variable_default_by_bits(context::OPSIZE, 1);
        base.set_variable_default_by_bits(context::REXPREFIX, 0);
        base.set_variable_default_by_bits(context::LONG_MODE, 1);

        Lifter::new(__impl::LANGUAGE, __impl::lifter_with(2, base))
    }

    pub fn new_compat32() -> Lifter {
        let mut base = __impl::default_context();

        base.set_variable_default_by_bits(context::ADDRSIZE, 1);
        base.set_variable_default_by_bits(context::OPSIZE, 1);
        base.set_variable_default_by_bits(context::REXPREFIX, 0);
        base.set_variable_default_by_bits(context::LONG_MODE, 0);

        Lifter::new(__impl::LANGUAGE, __impl::lifter_with(2, base))
    }
}
