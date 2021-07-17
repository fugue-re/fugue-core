use crate::address::AddressValue;
use crate::bits;
use crate::disassembly::{Error, ParserContext, ParserWalker};
use crate::disassembly::construct::{ConstructTpl, OpTpl, VarnodeTpl};
use crate::disassembly::symbol::{Constructor, SymbolTable};
use crate::disassembly::Opcode;
use crate::disassembly::VarnodeData;
use crate::float_format::FloatFormat;
use crate::space::AddressSpace;
use crate::space_manager::SpaceManager;
use crate::Translator;

use fnv::FnvHashMap as Map;
use smallvec::{smallvec, SmallVec};
use std::fmt;
use std::mem::swap;
use std::sync::Arc;

use crate::il::pcode::{self, PCode};
use crate::il::ecode::{self, ECode};

#[derive(Debug, Clone)]
pub struct PCodeRaw {
    pub address: AddressValue,
    pub operations: SmallVec<[PCodeData; 16]>,
    pub delay_slots: usize,
    pub length: usize,
}

pub struct PCodeRawFormatter<'a, 'b> {
    pcode: &'b PCodeRaw,
    translator: &'a Translator,
}

impl<'a, 'b> PCodeRawFormatter<'a, 'b> {
    fn new(pcode: &'b PCodeRaw, translator: &'a Translator) -> Self {
        Self {
            pcode,
            translator,
        }
    }
}

impl<'a, 'b> fmt::Display for PCodeRawFormatter<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let len =  self.pcode.operations.len();
        if len > 0 {
            for (i, op) in self.pcode.operations.iter().enumerate() {
                write!(f, "{}.{:02}: {}{}", self.pcode.address, i,
                       op.display(self.translator),
                       if i == len - 1 { "" } else { "\n" })?;
            }
            Ok(())
        } else {
            write!(f, "{}.00: Nop", self.pcode.address)
        }
    }
}

impl PCodeRaw {
    pub fn display<'a, 'b>(&'b self, translator: &'a Translator) -> PCodeRawFormatter<'a, 'b> {
        PCodeRawFormatter::new(self, translator)
    }

    pub fn nop(address: AddressValue, length: usize) -> Self {
        Self {
            address,
            operations: SmallVec::new(),
            delay_slots: 0,
            length,
        }
    }

    pub fn address(&self) -> AddressValue {
        self.address.clone()
    }

    pub fn operations(&self) -> &[PCodeData] {
        self.operations.as_ref()
    }

    pub fn delay_slots(&self) -> usize {
        self.delay_slots
    }

    pub fn length(&self) -> usize {
        self.length
    }
}

#[derive(Debug)]
pub struct RelativeRecord {
    instruction: usize,
    index: usize,
}

impl RelativeRecord {
    pub fn new(instruction: usize, index: usize) -> Self {
        Self {
            instruction,
            index,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PCodeData {
    pub opcode: Opcode,
    pub output: Option<VarnodeData>,
    pub inputs: SmallVec<[VarnodeData; 16]>,
}

pub struct PCodeDataFormatter<'a, 'b> {
    pcode: &'b PCodeData,
    translator: &'a Translator,
}

impl<'a, 'b> PCodeDataFormatter<'a, 'b> {
    fn new(pcode: &'b PCodeData, translator: &'a Translator) -> Self {
        Self {
            pcode,
            translator,
        }
    }
}

impl<'a, 'b> fmt::Display for PCodeDataFormatter<'a, 'b> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?}(", self.pcode.opcode)?;
        if let Some(ref output) = self.pcode.output {
            write!(f, "out={}", output.display(self.translator))?;
        }
        if self.pcode.inputs.len() > 0 {
            write!(f, "{}in=[", if self.pcode.output.is_some() { ", " } else { "" })?;
            for (i, input) in self.pcode.inputs.iter().enumerate() {
                write!(f, "{}{}", if i == 0 { "" } else { ", " }, input.display(self.translator))?;
            }
            write!(f, "]")?;
        }
        write!(f, ")")?;
        Ok(())
    }
}

impl PCodeData {
    pub fn display<'a, 'b>(&'b self, translator: &'a Translator) -> PCodeDataFormatter<'a, 'b> {
        PCodeDataFormatter::new(self, translator)
    }
}

pub struct IRBuilder<'a, 'b, 'c> {
    const_space: Arc<AddressSpace>,
    unique_mask: u64,
    unique_offset: u64,

    issued: SmallVec<[PCodeData; 16]>,

    label_base: usize,
    label_count: usize,
    label_refs: SmallVec<[RelativeRecord; 16]>,
    labels: SmallVec<[u64; 16]>,

    delay_contexts: Map<AddressValue, &'c mut ParserContext<'b>>,

    manager: &'a SpaceManager,
    float_formats: Map<usize, Arc<FloatFormat>>,
    registers: &'a Map<(u64, usize), Arc<str>>,
    user_ops: &'a [Arc<str>],

    walker: ParserWalker<'b, 'c>,
}

impl<'a, 'b, 'c> IRBuilder<'a, 'b, 'c> {
    pub fn new(walker: ParserWalker<'b, 'c>, delay_contexts: &'c mut Map<AddressValue, ParserContext<'b>>, manager: &'a SpaceManager, float_formats: &'a [Arc<FloatFormat>], registers: &'a Map<(u64, usize), Arc<str>>, user_ops: &'a [Arc<str>], unique_mask: u64) -> Result<Self, Error> {
        Ok(Self {
            const_space: manager.constant_space(),
            unique_mask,
            unique_offset: (walker.address().offset() & unique_mask).checked_shl(4).unwrap_or(0),
            issued: SmallVec::new(),
            label_base: 0,
            label_count: 0,
            labels: SmallVec::new(),
            label_refs: SmallVec::new(),
            delay_contexts: delay_contexts.iter_mut().map(|(a, v)| (a.clone(), v)).collect(),
            manager,
            float_formats: float_formats.iter().map(|ff| (ff.bits(), ff.clone())).collect(),
            registers,
            user_ops,
            walker,
        })
    }

    pub fn label_base(&self) -> usize {
        self.label_base
    }

    pub fn walker(&self) -> &ParserWalker<'b, 'c> {
        &self.walker
    }

    pub fn walker_mut(&mut self) -> &mut ParserWalker<'b, 'c> {
        &mut self.walker
    }

    pub fn set_unique_offset(&mut self, offset: u64) {
        self.unique_offset = (offset & self.unique_mask)
            .checked_shl(4)
            .unwrap_or(0);
    }

    pub fn build_empty(&mut self, ctor: &'b Constructor, section_num: Option<usize>, symbols: &'b SymbolTable) -> Result<(), Error> {
        let nops = ctor.operand_count();

        for i in 0..nops {
            let operand = symbols.symbol(self.walker
                                         .constructor()?
                                         .ok_or_else(|| Error::InvalidConstructor)?
                                         .operand(i)).ok_or_else(|| Error::InvalidSymbol)?;
            let symbol = operand.defining_symbol(symbols)?;
            if symbol.is_none() || !symbol.as_ref().unwrap().is_subtable() {
                continue
            }

            self.walker.push_operand(i)?;
            if let Some(ctpl) = self.walker.constructor()?.unwrap().named_template(section_num.ok_or_else(|| Error::InconsistentState)?) {
                self.build(ctpl, section_num, symbols)?;
            } else {
                self.build_empty(self.walker.constructor()?.unwrap(), section_num, symbols)?;
            }
            self.walker.pop_operand()?;
        }
        Ok(())
    }

    pub fn append_build(&mut self, op: &'b OpTpl, section_num: Option<usize>, symbols: &'b SymbolTable) -> Result<(), Error> {
        let index = op.input(0).offset().real() as usize;
        let operand = symbols.symbol(self.walker
                                     .constructor()?
                                     .ok_or_else(|| Error::InvalidConstructor)?
                                     .operand(index)).ok_or_else(|| Error::InvalidSymbol)?;
        let symbol = operand.defining_symbol(symbols)?;
        if symbol.is_none() || !symbol.as_ref().unwrap().is_subtable() {
            return Ok(())
        }

        self.walker.push_operand(index)?;
        let constructor = self.walker.constructor()?.unwrap();
        if let Some(section_num) = section_num {
            if let Some(ctpl) = constructor.named_template(section_num) {
                self.build(ctpl, Some(section_num), symbols)?;
            } else {
                self.build_empty(constructor, Some(section_num), symbols)?;
            }
        } else {
            if let Some(ctpl) = constructor.template() {
                self.build(ctpl, None, symbols)?;
            }
        }
        self.walker.pop_operand()?;
        Ok(())
    }

    pub fn delay_slot(&mut self, symbols: &'b SymbolTable) -> Result<(), Error> {
        let old_unique_offset = self.unique_offset;
        let base_address = self.walker.address();
        let delay_count = self.walker.delay_slot();
        let mut fall_offset = self.walker.length();
        let mut byte_count = 0;

        loop {
            let address = base_address.clone() + fall_offset;
            self.set_unique_offset(address.offset());

            let context = self.delay_contexts.remove(&address).unwrap();
            let mut nwalker = ParserWalker::new(context);
            let length = nwalker.length();

            // swap out
            swap(&mut self.walker, &mut nwalker);

            self.walker.base_state();

            if let Some(ctpl) = self.walker.constructor()?.ok_or_else(|| Error::InvalidConstructor)?.template() {
                self.build(ctpl, None, symbols)?;
            }

            fall_offset += length;
            byte_count += length;

            swap(&mut self.walker, &mut nwalker);

            if byte_count >= delay_count {
                break
            }
        }

        self.unique_offset = old_unique_offset;
        Ok(())
    }

    pub fn generate_location(&mut self, varnode: &'b VarnodeTpl) -> Result<VarnodeData, Error> {
        let space = varnode.space().fix_space(&mut self.walker, self.manager)?
            .ok_or_else(|| Error::InconsistentState)?;
        let size = varnode.size().fix(&mut self.walker, self.manager)?;

        let offset = if space.is_constant() {
            let offset = varnode.offset().fix(&mut self.walker, &self.manager)?;
            offset & bits::calculate_mask(size as usize)
        } else if space.is_unique() {
            let offset = varnode.offset().fix(&mut self.walker, &self.manager)?;
            offset | self.unique_offset
        } else {
            space.wrap_offset(varnode.offset().fix(&mut self.walker, &self.manager)?)
        };

        Ok(VarnodeData::new(space, offset, size as usize))
    }

    pub fn generate_pointer(&mut self, varnode: &'b VarnodeTpl) -> Result<(Arc<AddressSpace>, VarnodeData), Error> {
        let handle = self.walker.handle(
            varnode.offset().handle_index().ok_or_else(|| Error::InconsistentState)?
        )?.ok_or_else(|| Error::InvalidHandle)?;

        let space = handle.offset_space.ok_or_else(|| Error::InconsistentState)?;
        let size = handle.offset_size;

        let offset = if space.is_constant() {
            handle.offset_offset & bits::calculate_mask(size)
        } else if space.is_unique() {
            handle.offset_offset | self.unique_offset
        } else {
            space.wrap_offset(handle.offset_offset)
        };


        Ok((handle.space, VarnodeData::new(space, offset, size)))
    }

    pub fn add_label_ref(&mut self, instruction: usize, input: usize) {
        self.label_refs.push(RelativeRecord::new(instruction, input))
    }

    pub fn dump(&mut self, op: &'b OpTpl) -> Result<(), Error> {
        let input_count = op.input_count();
        let mut inputs = SmallVec::<[_; 16]>::new();

        for i in 0..input_count {
            let input = op.input(i);
            if input.is_dynamic(&mut self.walker)? {
                let varnode = self.generate_location(input)?;
                let (spc, ptr) = self.generate_pointer(input)?;
                let index = VarnodeData::new(self.const_space.clone(),
                                             spc.index() as u64,
                                             0);
                self.issued.push(PCodeData {
                    opcode: Opcode::Load,
                    inputs: smallvec![index, ptr],
                    output: Some(varnode.clone()),
                });
                inputs.push(varnode);
            } else {
                inputs.push(self.generate_location(input)?);
            }
        }

        if input_count > 0 && op.input(0).is_relative() {
            inputs[0].offset += self.label_base() as u64;
            self.add_label_ref(self.issued.len(), 0);
        }

        if let Some(output) = op.output() {
            let outp = self.generate_location(output)?;
            self.issued.push(PCodeData {
                opcode: op.opcode(),
                inputs,
                output: Some(outp.clone()),
            });

            if output.is_dynamic(&mut self.walker)? {
                let (spc, ptr) = self.generate_pointer(output)?;
                let index = VarnodeData::new(self.const_space.clone(),
                                             spc.index() as u64,
                                             0);
                self.issued.push(PCodeData {
                    opcode: Opcode::Store,
                    inputs: smallvec![index, ptr, outp],
                    output: None,
                })
            }
        } else {
            self.issued.push(PCodeData {
                opcode: op.opcode(),
                inputs,
                output: None,
            });
        }
        Ok(())
    }

    pub fn build(&mut self, constructor: &'b ConstructTpl, section_num: Option<usize>, symbols: &'b SymbolTable) -> Result<(), Error> {
        let old_base = self.label_base;
        self.label_base = self.label_count;
        self.label_count += constructor.labels();

        self.labels.resize_with(self.label_count, Default::default);

        for op in constructor.operations() {
            match op.opcode() {
                Opcode::Build => {
                    self.append_build(op, section_num, symbols)?;
                },
                Opcode::DelaySlot => {
                    self.delay_slot(symbols)?;
                },
                Opcode::CrossBuild => {
                    return Err(Error::InconsistentState)
                },
                _ => {
                    self.dump(op)?;
                }
            }
        }

        self.label_base = old_base;
        Ok(())
    }

    pub fn resolve_relatives(&mut self) -> Result<(), Error> {
        for rel in &self.label_refs {
            let varnode = &mut self.issued[rel.instruction].inputs[rel.index];
            let id = varnode.offset();
            if id >= self.labels.len() as u64 {
                return Err(Error::Invariant(format!("no known ways to set label {}", id)));
            }
            let res = (self.labels[id as usize] - rel.index as u64) & bits::calculate_mask(varnode.size());
            varnode.offset = res;
        }
        Ok(())
    }

    pub fn emit_raw(self, length: usize) -> PCodeRaw {
        let mut slf = self;
        slf.walker.base_state();
        PCodeRaw {
            address: slf.walker().address(),
            operations: slf.issued,
            delay_slots: slf.walker.delay_slot(),
            length,
        }
    }

    pub fn emit_pcode(self, length: usize) -> PCode {
        let mut slf = self;
        slf.walker.base_state();

        let address = slf.walker.address();
        let delay_slots = slf.walker.delay_slot();

        let manager = slf.manager;
        let registers = slf.registers;
        let user_ops = slf.user_ops;

        PCode {
            operations: slf.issued.into_iter()
                .map(|op|
                     pcode::PCodeOp::from_parts(
                         manager,
                         registers,
                         user_ops,
                         op.opcode,
                         op.inputs,
                         op.output,
                     )
                )
                .collect(),
            address,
            delay_slots,
            length,
        }
    }

    pub fn emit_ecode(self, length: usize) -> ECode {
        let mut slf = self;
        slf.walker.base_state();

        let address = slf.walker.address();
        let delay_slots = slf.walker.delay_slot();

        let manager = slf.manager;
        let float_formats = slf.float_formats;
        let user_ops = slf.user_ops;

        ECode {
            operations: slf.issued.into_iter()
                .enumerate()
                .map(|(i, op)|
                     ecode::Stmt::from_parts(
                         manager,
                         &float_formats,
                         user_ops,
                         &address,
                         i,
                         op.opcode,
                         op.inputs,
                         op.output
                     )
                )
                .collect(),
            address,
            delay_slots,
            length,
        }
    }
}
