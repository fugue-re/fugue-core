use crate::runtime::context::ContextBitRange;
use crate::runtime::pcode::{LiftingContext, PCodeOp, Varnode};
use crate::runtime::wrap_offset;

pub struct Language {
    pub address_alignment: usize,
    pub address_bits: u32,
    pub address_size: usize,
    pub address_upper_bound: u64,

    pub constant_space: u8,
    pub default_space: u8,

    pub register_space: u8,
    pub register_space_size: usize,

    pub unique_mask: u64,
    pub unique_space: u8,
    pub unique_space_size: usize,

    pub space_word_sizes: &'static [usize],
    pub space_upper_bounds: &'static [u64],
    pub space_by_name: fn(&str) -> Option<u8>,
    pub space_name: fn(u8) -> Option<&'static str>,

    pub context_variable_by_name: fn(&str) -> Option<ContextBitRange>,

    pub register_by_name: fn(&str) -> Option<Varnode>,
    pub register_name: fn(&Varnode) -> Option<&'static str>,

    pub user_op_by_name: fn(&str) -> Option<u16>,
    pub user_op_by_id: fn(u16) -> &'static str,

    pub resolve: fn(u64, &[u8], &mut LiftingContext, bool) -> Option<usize>,
    pub disassemble: fn(u64, &[u8], &mut LiftingContext, &mut String) -> Option<usize>,
    pub lift: fn(u64, &[u8], &mut LiftingContext, &mut Vec<PCodeOp>) -> Option<usize>,
}

pub struct Lifter {
    language: &'static Language,
    context: LiftingContext,
}

impl Lifter {
    pub fn new(language: &'static Language, context: LiftingContext) -> Self {
        Self { language, context }
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
        (self.language.space_name)(space)
    }

    pub fn space_by_name(&self, name: impl AsRef<str>) -> Option<u8> {
        (self.language.space_by_name)(name.as_ref())
    }

    pub fn space_word_size(&self, space: u8) -> Option<usize> {
        self.language.space_word_sizes.get(space as usize).copied()
    }

    pub fn space_upper_bound(&self, space: u8) -> Option<u64> {
        self.language
            .space_upper_bounds
            .get(space as usize)
            .copied()
    }

    pub fn wrap_offset(&self, space: u8, offset: u64) -> Option<u64> {
        self.space_upper_bound(space)
            .map(|highest| wrap_offset(highest, offset))
    }

    pub fn register_by_name(&self, name: impl AsRef<str>) -> Option<Varnode> {
        (self.language.register_by_name)(name.as_ref())
    }

    pub fn register_name(&self, vnd: &Varnode) -> Option<&'static str> {
        (self.language.register_name)(vnd)
    }

    pub fn user_op_by_name(&self, name: impl AsRef<str>) -> Option<u16> {
        (self.language.user_op_by_name)(name.as_ref())
    }

    pub fn user_op_by_id(&self, id: u16) -> &'static str {
        (self.language.user_op_by_id)(id)
    }

    pub fn resolve(
        &mut self,
        address: u64,
        bytes: impl AsRef<[u8]>,
        apply_commits: bool,
    ) -> Option<usize> {
        (self.language.resolve)(address, bytes.as_ref(), &mut self.context, apply_commits)
    }

    pub fn disassemble(
        &mut self,
        address: u64,
        bytes: impl AsRef<[u8]>,
        disassembly: &mut String,
    ) -> Option<usize> {
        (self.language.disassemble)(address, bytes.as_ref(), &mut self.context, disassembly)
    }

    pub fn lift(
        &mut self,
        address: u64,
        bytes: impl AsRef<[u8]>,
        operations: &mut Vec<PCodeOp>,
    ) -> Option<usize> {
        (self.language.lift)(address, bytes.as_ref(), &mut self.context, operations)
    }
}
