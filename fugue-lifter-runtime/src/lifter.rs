use crate::context::ContextBitRange;
use crate::pcode::{LiftingContext, PCodeBuilderContext, PCodeOp, Varnode};
use crate::wrap_offset;

pub trait LanguageImpl {
    const ADDRESS_ALIGNMENT: usize;
    const ADDRESS_BITS: u32;
    const ADDRESS_SIZE: usize;
    const ADDRESS_UPPER_BOUND: u64;

    const CONSTANT_SPACE: u8;
    const DEFAULT_SPACE: u8;

    const REGISTER_SPACE: u8;
    const REGISTER_SPACE_SIZE: usize;

    const UNIQUE_MASK: u64;
    const UNIQUE_SPACE: u8;
    const UNIQUE_SPACE_SIZE: usize;

    const SPACE_WORD_SIZES: &'static [usize];
    const SPACE_UPPER_BOUNDS: &'static [u64];
    const SPACE_BY_NAME: fn(&str) -> Option<u8>;
    const SPACE_NAME: fn(u8) -> Option<&'static str>;

    const CONTEXT_VARIABLE_BY_NAME: fn(&str) -> Option<ContextBitRange>;

    const REGISTER_BY_NAME: fn(&str) -> Option<Varnode>;
    const REGISTER_NAME: fn(&Varnode) -> Option<&'static str>;

    const USER_OP_BY_NAME: fn(&str) -> Option<u16>;
    const USER_OP_BY_ID: fn(u16) -> Option<&'static str>;

    const RESOLVE: fn(u64, &[u8], &mut LiftingContext, bool) -> Option<usize>;
    const DISASSEMBLE: fn(u64, &[u8], &mut LiftingContext, &mut String) -> Option<usize>;
    const LIFT: fn(u64, &[u8], &mut LiftingContext, &mut Vec<PCodeOp>) -> Option<usize>;
}

#[derive(Clone)]
pub struct Language {
    address_alignment: usize,
    address_bits: u32,
    address_size: usize,
    address_upper_bound: u64,

    constant_space: u8,
    default_space: u8,

    register_space: u8,
    register_space_size: usize,

    unique_mask: u64,
    unique_space: u8,
    unique_space_size: usize,

    space_word_sizes: &'static [usize],
    space_upper_bounds: &'static [u64],
    space_by_name: fn(&str) -> Option<u8>,
    space_name: fn(u8) -> Option<&'static str>,

    context_variable_by_name: fn(&str) -> Option<ContextBitRange>,

    register_by_name: fn(&str) -> Option<Varnode>,
    register_name: fn(&Varnode) -> Option<&'static str>,

    user_op_by_name: fn(&str) -> Option<u16>,
    user_op_by_id: fn(u16) -> Option<&'static str>,

    resolve: fn(u64, &[u8], &mut LiftingContext, bool) -> Option<usize>,
    disassemble: fn(u64, &[u8], &mut LiftingContext, &mut String) -> Option<usize>,
    lift: fn(u64, &[u8], &mut LiftingContext, &mut Vec<PCodeOp>) -> Option<usize>,
}

pub struct LanguageFormatter<'a, T> {
    pub(crate) language: &'static Language,
    pub(crate) value: &'a T,
}

impl<'a, T> LanguageFormatter<'a, T> {
    pub fn new(language: &'static Language, value: &'a T) -> Self {
        Self { language, value }
    }

    pub fn wrap<'b, U>(&self, value: &'b U) -> LanguageFormatter<'b, U> {
        LanguageFormatter { language: self.language, value }
    }
}

impl Language {
    pub const fn new<L: LanguageImpl>() -> Self {
        Self {
            address_alignment: L::ADDRESS_ALIGNMENT,
            address_bits: L::ADDRESS_BITS,
            address_size: L::ADDRESS_SIZE,
            address_upper_bound: L::ADDRESS_UPPER_BOUND,

            constant_space: L::CONSTANT_SPACE,
            default_space: L::DEFAULT_SPACE,

            register_space: L::REGISTER_SPACE,
            register_space_size: L::REGISTER_SPACE_SIZE,

            unique_mask: L::UNIQUE_MASK,
            unique_space: L::UNIQUE_SPACE,
            unique_space_size: L::UNIQUE_SPACE_SIZE,

            space_word_sizes: L::SPACE_WORD_SIZES,
            space_upper_bounds: L::SPACE_UPPER_BOUNDS,
            space_by_name: L::SPACE_BY_NAME,
            space_name: L::SPACE_NAME,

            context_variable_by_name: L::CONTEXT_VARIABLE_BY_NAME,

            register_by_name: L::REGISTER_BY_NAME,
            register_name: L::REGISTER_NAME,

            user_op_by_name: L::USER_OP_BY_NAME,
            user_op_by_id: L::USER_OP_BY_ID,

            resolve: L::RESOLVE,
            disassemble: L::DISASSEMBLE,
            lift: L::LIFT,
        }
    }

    pub fn address_alignment(&self) -> usize {
        self.address_alignment
    }

    pub fn address_bits(&self) -> u32 {
        self.address_bits
    }

    pub fn address_size(&self) -> usize {
        self.address_size
    }

    pub fn address_upper_bound(&self) -> u64 {
        self.address_upper_bound
    }

    pub fn constant_space(&self) -> u8 {
        self.constant_space
    }

    pub fn default_space(&self) -> u8 {
        self.default_space
    }

    pub fn register_space(&self) -> u8 {
        self.register_space
    }

    pub fn register_space_size(&self) -> usize {
        self.register_space_size
    }

    pub fn unique_mask(&self) -> u64 {
        self.unique_mask
    }

    pub fn unique_space(&self) -> u8 {
        self.unique_space
    }

    pub fn unique_space_size(&self) -> usize {
        self.unique_space_size
    }

    pub fn space_name(&self, space: u8) -> Option<&'static str> {
        (self.space_name)(space)
    }

    pub fn space_by_name(&self, name: impl AsRef<str>) -> Option<u8> {
        (self.space_by_name)(name.as_ref())
    }

    pub fn space_word_size(&self, space: u8) -> Option<usize> {
        self.space_word_sizes.get(space as usize).copied()
    }

    pub fn space_upper_bound(&self, space: u8) -> Option<u64> {
        self.space_upper_bounds.get(space as usize).copied()
    }

    pub fn wrap_offset(&self, space: u8, offset: u64) -> Option<u64> {
        self.space_upper_bound(space)
            .map(|highest| wrap_offset(highest, offset))
    }

    pub fn context_variable_by_name(&self, name: impl AsRef<str>) -> Option<ContextBitRange> {
        (self.context_variable_by_name)(name.as_ref())
    }

    pub fn register_by_name(&self, name: impl AsRef<str>) -> Option<Varnode> {
        (self.register_by_name)(name.as_ref())
    }

    pub fn register_name(&self, vnd: &Varnode) -> Option<&'static str> {
        (self.register_name)(vnd)
    }

    pub fn user_op_by_name(&self, name: impl AsRef<str>) -> Option<u16> {
        (self.user_op_by_name)(name.as_ref())
    }

    pub fn user_op_by_id(&self, id: u16) -> Option<&'static str> {
        (self.user_op_by_id)(id)
    }

    pub fn builder(&self) -> PCodeBuilderContext {
        PCodeBuilderContext::new(self.unique_mask)
    }

    pub fn display<'a, T>(&'static self, value: &'a T) -> LanguageFormatter<'a, T> {
        LanguageFormatter::new(self, value)
    }

    pub fn resolve(
        &self,
        address: u64,
        bytes: impl AsRef<[u8]>,
        context: &mut LiftingContext,
        apply_commits: bool,
    ) -> Option<usize> {
        (self.resolve)(address, bytes.as_ref(), context, apply_commits)
    }

    pub fn disassemble(
        &self,
        address: u64,
        bytes: impl AsRef<[u8]>,
        context: &mut LiftingContext,
        disassembly: &mut String,
    ) -> Option<usize> {
        (self.disassemble)(address, bytes.as_ref(), context, disassembly)
    }

    pub fn lift(
        &self,
        address: u64,
        bytes: impl AsRef<[u8]>,
        context: &mut LiftingContext,
        operations: &mut Vec<PCodeOp>,
    ) -> Option<usize> {
        (self.lift)(address, bytes.as_ref(), context, operations)
    }
}

pub struct Lifter {
    language: &'static Language,
    context: LiftingContext,
}

impl Lifter {
    pub fn new(language: &'static Language, context: LiftingContext) -> Self {
        Self { language, context }
    }

    pub fn language(&self) -> &'static Language {
        self.language
    }

    pub fn address_alignment(&self) -> usize {
        self.language.address_alignment
    }

    pub fn address_bits(&self) -> u32 {
        self.language.address_bits
    }

    pub fn address_size(&self) -> usize {
        self.language.address_size
    }

    pub fn address_upper_bound(&self) -> u64 {
        self.language.address_upper_bound
    }

    pub fn constant_space(&self) -> u8 {
        self.language.constant_space
    }

    pub fn default_space(&self) -> u8 {
        self.language.default_space
    }

    pub fn register_space(&self) -> u8 {
        self.language.register_space
    }

    pub fn register_space_size(&self) -> usize {
        self.language.register_space_size
    }

    pub fn unique_mask(&self) -> u64 {
        self.language.unique_mask
    }

    pub fn unique_space(&self) -> u8 {
        self.language.unique_space
    }

    pub fn unique_space_size(&self) -> usize {
        self.language.unique_space_size
    }

    pub fn space_name(&self, space: u8) -> Option<&'static str> {
        self.language.space_name(space)
    }

    pub fn space_by_name(&self, name: impl AsRef<str>) -> Option<u8> {
        self.language.space_by_name(name.as_ref())
    }

    pub fn space_word_size(&self, space: u8) -> Option<usize> {
        self.language.space_word_size(space)
    }

    pub fn space_upper_bound(&self, space: u8) -> Option<u64> {
        self.language.space_upper_bound(space)
    }

    pub fn wrap_offset(&self, space: u8, offset: u64) -> Option<u64> {
        self.language.wrap_offset(space, offset)
    }

    pub fn context_variable_by_name(&self, name: impl AsRef<str>) -> Option<ContextBitRange> {
        self.language.context_variable_by_name(name)
    }

    pub fn register_by_name(&self, name: impl AsRef<str>) -> Option<Varnode> {
        self.language.register_by_name(name)
    }

    pub fn register_name(&self, vnd: &Varnode) -> Option<&'static str> {
        self.language.register_name(vnd)
    }

    pub fn user_op_by_name(&self, name: impl AsRef<str>) -> Option<u16> {
        self.language.user_op_by_name(name)
    }

    pub fn user_op_by_id(&self, id: u16) -> Option<&'static str> {
        self.language.user_op_by_id(id)
    }

    pub fn builder(&self) -> PCodeBuilderContext {
        self.language.builder()
    }

    pub fn resolve(
        &mut self,
        address: u64,
        bytes: impl AsRef<[u8]>,
        apply_commits: bool,
    ) -> Option<usize> {
        self.language
            .resolve(address, bytes, &mut self.context, apply_commits)
    }

    pub fn disassemble(
        &mut self,
        address: u64,
        bytes: impl AsRef<[u8]>,
        disassembly: &mut String,
    ) -> Option<usize> {
        self.language
            .disassemble(address, bytes, &mut self.context, disassembly)
    }

    pub fn lift(
        &mut self,
        address: u64,
        bytes: impl AsRef<[u8]>,
        operations: &mut Vec<PCodeOp>,
    ) -> Option<usize> {
        self.language
            .lift(address, bytes, &mut self.context, operations)
    }
}
