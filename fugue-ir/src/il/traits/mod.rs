use crate::space::AddressSpaceId;
use crate::translator::Translator;
use fugue_bv::BitVec;

use std::borrow::Cow;
use std::fmt;

pub trait BitSize {
    fn bits(&self) -> usize;
}

pub trait Variable {
    fn space(&self) -> AddressSpaceId;
    fn generation(&self) -> usize;
    fn generation_mut(&mut self) -> &mut usize;
    fn with_generation(&self, generation: usize) -> Self;
}

pub trait TranslateIR<Loc, Val, Var> {
    type TLoc;
    type TVal;
    type TVar;

    fn translate_loc(&self, loc: Loc) -> Self::TLoc;
    fn translate_val(&self, val: Val) -> Self::TVal;
    fn translate_var(&self, var: Var) -> Self::TVar;
}

#[derive(Clone)]
pub struct TranslatorFormatter<'t> {
    pub translator: Option<&'t Translator>,
    pub branch_start: &'t str,
    pub branch_end: &'t str,
    pub keyword_start: &'t str,
    pub keyword_end: &'t str,
    pub location_start: &'t str,
    pub location_end: &'t str,
    pub type_start: &'t str,
    pub type_end: &'t str,
    pub value_start: &'t str,
    pub value_end: &'t str,
    pub variable_start: &'t str,
    pub variable_end: &'t str,
}

impl<'t> Default for TranslatorFormatter<'t> {
    fn default() -> Self {
        Self {
            translator: None,
            branch_start: "",
            branch_end: "",
            keyword_start: "",
            keyword_end: "",
            location_start: "",
            location_end: "",
            type_start: "",
            type_end: "",
            value_start: "",
            value_end: "",
            variable_start: "",
            variable_end: "",
        }
    }
}

impl<'t> From<&'t Translator> for TranslatorFormatter<'t> {
    fn from(t: &'t Translator) -> Self {
        Self {
            translator: Some(t),
            ..Default::default()
        }
    }
}

impl<'t> From<Option<&'t Translator>> for TranslatorFormatter<'t> {
    fn from(translator: Option<&'t Translator>) -> Self {
        Self {
            translator,
            ..Default::default()
        }
    }
}

pub trait TranslatorDisplay<'v, 't> {
    type Target: fmt::Display;

    fn display_with(&'v self, translator: Option<&'t Translator>) -> Self::Target {
        self.display_full(Cow::Owned(TranslatorFormatter::from(translator)))
    }

    fn display_full(&'v self, display: Cow<'t, TranslatorFormatter<'t>>) -> Self::Target;
}

impl BitSize for BitVec {
    fn bits(&self) -> usize {
        self.bits()
    }
}

pub struct BitVecFormatter<'v, 't> {
    bv: &'v BitVec,
    fmt: Cow<'t, TranslatorFormatter<'t>>,
}

impl<'v, 't> fmt::Display for BitVecFormatter<'v, 't> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = self.bv.as_bigint();
        write!(
            f,
            "{}{:#x}{}:{}{}{}",
            self.fmt.value_start,
            &*v,
            self.fmt.value_end,
            self.fmt.value_start,
            self.bv.bits(),
            self.fmt.value_end
        )
    }
}

impl<'v, 't> TranslatorDisplay<'v, 't> for BitVec {
    type Target = BitVecFormatter<'v, 't>;

    fn display_full(
        &'v self,
        fmt: Cow<'t, TranslatorFormatter<'t>>,
    ) -> Self::Target {
        BitVecFormatter {
            bv: self,
            fmt,
        }
    }
}
