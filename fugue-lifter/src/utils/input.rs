use std::mem::size_of;

use arrayvec::ArrayVec;

use crate::utils::constructor::Constructor;
use crate::utils::context::ContextDatabase;

const MAX_CTOR_STATES: usize = 128;
const MAX_CTXT_CHUNKS: usize = 2;
const MAX_PARSER_DEPTH: usize = 64;

pub const INVALID_HANDLE: u8 = 0xff;
pub const BREADCRUMBS: usize = MAX_PARSER_DEPTH + 1;

#[derive(Copy, Clone)]
pub struct FixedHandle {
    pub space: u8,
    pub size: u8,
    pub offset_space: u8,
    pub offset_offset: u64,
    pub offset_size: u8,
    pub temporary_space: u8,
    pub temporary_offset: u64,
}

impl Default for FixedHandle {
    fn default() -> Self {
        Self {
            space: INVALID_HANDLE,
            size: 0,
            offset_space: INVALID_HANDLE,
            offset_offset: 0,
            offset_size: 0,
            temporary_space: INVALID_HANDLE,
            temporary_offset: 0,
        }
    }
}

#[derive(Copy, Clone)]
pub struct ConstructorNode {
    pub constructor: Option<&'static Constructor>,
    pub handle: Option<FixedHandle>,
    pub operands: u8, // offset into ctors
    pub parent: u8,
    pub offset: u8,
    pub length: u8,
}

impl Default for ConstructorNode {
    fn default() -> Self {
        Self {
            constructor: None,
            handle: None,
            operands: INVALID_HANDLE,
            parent: INVALID_HANDLE,
            offset: 0,
            length: 0,
        }
    }
}

pub type ContextCommitApplier =
    fn(&ParserInput, &mut ContextDatabase, &ContextCommit) -> Option<()>;

pub struct ContextCommit {
    pub applier: ContextCommitApplier,
    pub point: u8,
    pub values: ArrayVec<u32, MAX_CTXT_CHUNKS>,
}

pub struct ParserContext {
    pub buffer: [u8; 16],
    pub context: ArrayVec<u32, MAX_CTXT_CHUNKS>,
    pub constructors: [ConstructorNode; MAX_CTOR_STATES],
    pub commits: ArrayVec<ContextCommit, MAX_CTOR_STATES>,
    pub address: u64,
    pub offset: u8,
    pub delay_slot: u8,
    pub alloc: u8,
}

pub struct ParserInput {
    pub context: ParserContext,
    pub breadcrumb: [u8; BREADCRUMBS],
    pub depth: i8,
    pub point: u8,
}

impl ParserInput {
    pub fn new(address: u64, bytes: &[u8]) -> Self {
        let mut buffer = [0u8; 16];

        let view_len = bytes.len().min(buffer.len());
        buffer[..view_len].copy_from_slice(&bytes[..view_len]);

        let context = ParserContext {
            buffer,
            context: Default::default(),
            constructors: [ConstructorNode::default(); MAX_CTOR_STATES],
            commits: Default::default(),
            address,
            offset: 0,
            delay_slot: 0,
            alloc: 1,
        };

        Self {
            context,
            breadcrumb: [0u8; MAX_PARSER_DEPTH + 1],
            depth: 0,
            point: 0,
        }
    }

    #[inline(always)]
    pub fn base_state(&mut self) {
        self.point = 0;
        self.depth = 0;
        self.breadcrumb[0] = 0;
    }

    #[inline(always)]
    pub fn address(&self) -> u64 {
        self.context.address
    }

    #[inline(always)]
    pub fn next_address(&self) -> u64 {
        self.context.address + self.context.constructors[0].length as u64
    }

    #[inline(always)]
    pub fn resolved(&self) -> bool {
        self.point == INVALID_HANDLE
    }

    #[inline(always)]
    pub fn operand(&self) -> usize {
        unsafe { *self.breadcrumb.get_unchecked(self.depth as usize) as _ }
    }

    #[inline]
    pub fn constructor(&self) -> &'static Constructor {
        unsafe {
            self.context
                .constructors
                .get_unchecked(self.point as usize)
                .constructor
                .unwrap_unchecked()
        }
    }

    #[inline]
    pub fn set_context(&mut self, db: &ContextDatabase) {
        self.context.context = db
            .get_context(self.context.address)
            .try_into()
            .expect("context is correct size");
    }

    #[inline(always)]
    pub fn instruction_bytes(&self, start: usize, size: usize) -> Option<u32> {
        self.instruction_bytes_with(start, size, self.context.offset as _)
    }

    #[inline(always)]
    pub fn instruction_bytes_with(&self, start: usize, size: usize, offset: usize) -> Option<u32> {
        let offset = offset + start;
        let end = offset + size;

        if offset > self.context.buffer.len() {
            return None;
        }

        let mut result = 0u32;
        unsafe {
            for i in offset..end.min(self.context.buffer.len()) {
                result = (result << 8) | *self.context.buffer.get_unchecked(i) as u32;
            }
        }

        Some(result)
    }

    #[inline(always)]
    pub fn instruction_bits(&self, start: usize, size: usize) -> Option<u32> {
        self.instruction_bits_with(start, size, self.context.offset as _)
    }

    #[inline(always)]
    pub fn instruction_bits_with(&self, start: usize, size: usize, offset: usize) -> Option<u32> {
        let bit_offset = start % 8;
        let byte_offset = offset + (start / 8);
        let total_bits = bit_offset + size;
        let bytes_needed = (total_bits + 7) / 8;

        if byte_offset >= self.context.buffer.len() {
            return None;
        }

        let available_bytes = self.context.buffer.len() - byte_offset;
        let bytes_to_read = bytes_needed.min(available_bytes);

        let mut result = 0u32;
        unsafe {
            for i in 0..bytes_to_read {
                result =
                    (result << 8) | (*self.context.buffer.get_unchecked(byte_offset + i) as u32);
            }
        }

        result = result
            .checked_shl((32 - (bytes_to_read * 8) + bit_offset) as u32)
            .unwrap_or(0);
        result = result.checked_shr((32 - size) as u32).unwrap_or(0);

        Some(result)
    }

    #[inline(always)]
    pub fn context_bytes(&self, start: usize, size: usize) -> u32 {
        let start_off = start / size_of::<u32>();
        let bytes_off = start % size_of::<u32>();

        let unused = size_of::<u32>() - size;
        let mut result = self.context.context[start_off];

        result <<= bytes_off as u32 * 8;
        result = result.checked_shr(unused as u32 * 8).unwrap_or(0);

        let remaining = (bytes_off + size) as i32 - (8 * size_of::<u32>()) as i32;
        if remaining <= 0 {
            return result;
        }

        if start_off + 1 < self.context.context.len() {
            let mut nresult = self.context.context[start_off + 1];
            let unused = size_of::<u32>() - remaining as usize;
            nresult = nresult.checked_shr(unused as u32 * 8).unwrap_or(0);
            result |= nresult;
        }

        result
    }

    #[inline(always)]
    pub fn context_bits(&self, start: usize, size: usize) -> u32 {
        let start_off = start / (8 * size_of::<u32>());
        let bits_off = start % (8 * size_of::<u32>());

        let unused = 8 * size_of::<u32>() - size;
        let mut result = self.context.context[start_off];

        result <<= bits_off as u32;
        result = result.checked_shr(unused as u32).unwrap_or(0);

        let remaining = (bits_off + size) as i32 - (8 * size_of::<u32>()) as i32;
        if remaining <= 0 {
            return result;
        }

        if start_off + 1 < self.context.context.len() {
            let mut nresult = self.context.context[start_off + 1];
            let unused = 8 * size_of::<u32>() - remaining as usize;
            nresult = nresult.checked_shr(unused as u32).unwrap_or(0);
            result |= nresult;
        }

        result
    }

    #[inline]
    pub fn calculate_length(&mut self, length: usize, operands: usize) {
        unsafe {
            let state = self.context.constructors.get_unchecked(self.point as usize);
            let offset = state.offset as usize;
            let mut max_length = length + offset;

            for opid in 0..operands {
                let op = self
                    .context
                    .constructors
                    .get_unchecked(state.operands as usize + opid);

                let op_len = op.length + op.offset;
                max_length = max_length.max(op_len as usize);
            }

            self.context
                .constructors
                .get_unchecked_mut(self.point as usize)
                .length = (max_length - offset) as u8;
        }
    }

    #[inline(always)]
    pub fn offset(&self) -> usize {
        unsafe {
            self.context
                .constructors
                .get_unchecked(self.point as usize)
                .offset as _
        }
    }

    #[inline(always)]
    pub fn set_offset(&mut self, offset: usize) {
        unsafe {
            self.context
                .constructors
                .get_unchecked_mut(self.point as usize)
                .offset = offset as _;
        }
    }

    #[inline(always)]
    pub fn offset_for_operand(&mut self, index: usize) -> usize {
        unsafe {
            let opid = self
                .context
                .constructors
                .get_unchecked_mut(self.point as usize)
                .operands as usize
                + index;
            let op = self.context.constructors.get_unchecked(opid as usize);
            op.offset as usize + op.length as usize
        }
    }

    #[inline(always)]
    pub fn set_constructor(&mut self, ctor: &'static Constructor) {
        unsafe {
            self.context
                .constructors
                .get_unchecked_mut(self.point as usize)
                .constructor = Some(ctor);
        }
    }

    #[inline(always)]
    pub fn set_context_word(&mut self, num: usize, value: u32, mask: u32) {
        unsafe {
            *self.context.context.get_unchecked_mut(num) =
                (*self.context.context.get_unchecked(num) & !mask) | (mask & value);
        }
    }

    #[inline(always)]
    pub fn set_current_length(&mut self, length: usize) {
        unsafe {
            self.context
                .constructors
                .get_unchecked_mut(self.point as usize)
                .length = length as _;
        }
    }

    #[inline(always)]
    pub fn set_delay_slot(&mut self, count: usize) {
        self.context.delay_slot = count as _;
    }

    #[inline(always)]
    pub fn allocate_operands(&mut self, operands: usize) -> usize {
        unsafe {
            let id = self.context.alloc;

            for opid in 0..operands {
                let op = self
                    .context
                    .constructors
                    .get_unchecked_mut(id as usize + opid);

                op.parent = self.point;
                op.constructor = None;
                op.operands = INVALID_HANDLE;
                op.offset = 0;
                op.length = 0;
            }

            self.context.alloc += operands as u8;

            self.context
                .constructors
                .get_unchecked_mut(self.point as usize)
                .operands = id;

            // NOTE: we don't need this bit -- breadcrumb 0 is the ctor and we
            // can get the same effect by using push operand when we explore
            /*
            self.breadcrumb[self.depth as usize] += 1;
            self.depth += 1;

            self.point = id; // make it the first operand?
            self.breadcrumb[self.depth as usize] = 0;
            */

            id as _
        }
    }

    #[inline(always)]
    pub fn push_operand(&mut self, operand: usize) {
        unsafe {
            *self.breadcrumb.get_unchecked_mut(self.depth as usize) = operand as u8 + 1;
            self.depth += 1;
            self.point = self
                .context
                .constructors
                .get_unchecked(self.point as usize)
                .operands
                + operand as u8;
            *self.breadcrumb.get_unchecked_mut(self.depth as usize) = 0;
        }
    }

    #[inline(always)]
    pub fn pop_operand(&mut self) {
        unsafe {
            self.point = self
                .context
                .constructors
                .get_unchecked(self.point as usize)
                .parent;
            self.depth -= 1; // here's where it can go to -1 (when we pop the last ctor)
        }
    }

    #[inline(always)]
    pub fn register_context_commit(&mut self, commit: ContextCommit) {
        self.context.commits.push(commit);
    }

    #[inline]
    pub fn apply_commits(&self, db: &mut ContextDatabase) {
        for commit in self.context.commits.iter() {
            (commit.applier)(self, db, commit);
        }
    }

    #[inline(always)]
    pub fn parent_handle(&self) -> Option<&FixedHandle> {
        self.context.constructors[self.point as usize]
            .handle
            .as_ref()
    }

    #[inline(always)]
    pub fn parent_handle_mut(&mut self) -> Option<&mut FixedHandle> {
        unsafe {
            self.context
                .constructors
                .get_unchecked_mut(self.point as usize)
                .handle
                .as_mut()
        }
    }

    #[inline(always)]
    pub fn set_parent_handle(&mut self, handle: FixedHandle) {
        unsafe {
            self.context
                .constructors
                .get_unchecked_mut(self.point as usize)
                .handle = Some(handle);
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.context.constructors[0].length as _
    }
}
