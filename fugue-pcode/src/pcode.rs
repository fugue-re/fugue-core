use crate::address::Address;
use crate::bits;
use crate::construct::{ConstructTpl, OpTpl, VarnodeTpl};
use crate::disassembly::{ParserContext, ParserWalker};
use crate::error::disassembly as di;
use crate::opcode::Opcode;
use crate::space::AddressSpace;
use crate::subtable::Constructor;
use crate::space_manager::SpaceManager;
use crate::symbol_table::SymbolTable;
use crate::varnodedata::VarnodeData;
use crate::Translator;

use std::collections::BTreeMap as Map;
use std::fmt;
use std::mem::swap;
use std::sync::Arc;
use snafu::OptionExt;

#[derive(Debug, Clone)]
pub struct PCode {
    pub address: Address,
    pub operations: Vec<PCodeData>,
    pub delay_slots: usize,
    pub length: usize,
}

pub struct PCodeFormatter<'a> {
    pcode: &'a PCode,
    translator: &'a Translator,
}

impl<'a> PCodeFormatter<'a> {
    fn new(pcode: &'a PCode, translator: &'a Translator) -> Self {
        Self {
            pcode,
            translator,
        }
    }
}

impl<'a> fmt::Display for PCodeFormatter<'a> {
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

impl PCode {
    pub fn display<'a>(&'a self, translator: &'a Translator) -> PCodeFormatter<'a> {
        PCodeFormatter::new(self, translator)
    }

    pub fn nop(address: Address, length: usize) -> Self {
        Self {
            address,
            operations: Vec::new(),
            delay_slots: 0,
            length,
        }
    }

    pub fn address(&self) -> &Address {
        &self.address
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
    pub inputs: Vec<VarnodeData>,
}

pub struct PCodeDataFormatter<'a> {
    pcode: &'a PCodeData,
    translator: &'a Translator,
}

impl<'a> PCodeDataFormatter<'a> {
    fn new(pcode: &'a PCodeData, translator: &'a Translator) -> Self {
        Self {
            pcode,
            translator,
        }
    }
}

impl<'a> fmt::Display for PCodeDataFormatter<'a> {
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
    pub fn display<'a>(&'a self, translator: &'a Translator) -> PCodeDataFormatter<'a> {
        PCodeDataFormatter::new(self, translator)
    }
}

pub struct PCodeBuilder<'a, 'b> {
    const_space: Arc<AddressSpace>,
    unique_mask: u64,
    unique_offset: u64,

    issued: Vec<PCodeData>,

    label_base: usize,
    label_count: usize,
    label_refs: Vec<RelativeRecord>,
    labels: Vec<u64>,

    delay_contexts: Map<Address, &'b mut ParserContext<'a>>,
    manager: &'a SpaceManager,
    walker: ParserWalker<'a, 'b>,
}

impl<'a, 'b> PCodeBuilder<'a, 'b> {
    pub fn new(walker: ParserWalker<'a, 'b>, delay_contexts: &'b mut Map<Address, ParserContext<'a>>, translator: &'a Translator) -> Result<Self, di::Error> {
        Ok(Self {
            const_space: translator.manager().constant_space().with_context(|| di::InvalidSpace)?,
            unique_mask: translator.unique_mask(),
            unique_offset: (walker.address().offset() & translator.unique_mask()).checked_shl(4).unwrap_or(0),
            issued: Vec::new(),
            label_base: 0,
            label_count: 0,
            labels: Vec::new(),
            label_refs: Vec::new(),
            delay_contexts: delay_contexts.iter_mut().map(|(a, v)| (a.clone(), v)).collect(),
            manager: translator.manager(),
            walker,
        })
    }

    pub fn label_base(&self) -> usize {
        self.label_base
    }

    pub fn walker(&self) -> &ParserWalker<'a, 'b> {
        &self.walker
    }

    pub fn walker_mut(&mut self) -> &mut ParserWalker<'a, 'b> {
        &mut self.walker
    }

    pub fn set_unique_offset(&mut self, offset: u64) {
        self.unique_offset = (offset & self.unique_mask)
            .checked_shl(4)
            .unwrap_or(0);
    }

    pub fn build_empty(&mut self, ctor: &'a Constructor, section_num: Option<usize>, symbols: &'a SymbolTable) -> Result<(), di::Error> {
        let nops = ctor.operand_count();

        for i in 0..nops {
            let operand = symbols.symbol(self.walker
                                         .constructor()?
                                         .with_context(|| di::InvalidConstructor)?
                                         .operand(i)).with_context(|| di::InvalidSymbol)?;
            let symbol = operand.defining_symbol(symbols)?;
            if symbol.is_none() || !symbol.as_ref().unwrap().is_subtable() {
                continue
            }

            self.walker.push_operand(i)?;
            if let Some(ctpl) = self.walker.constructor()?.unwrap().named_template(section_num.with_context(|| di::InconsistentState)?) {
                self.build(ctpl, section_num, symbols)?;
            } else {
                self.build_empty(self.walker.constructor()?.unwrap(), section_num, symbols)?;
            }
            self.walker.pop_operand()?;
        }
        Ok(())
    }

    pub fn append_build(&mut self, op: &'a OpTpl, section_num: Option<usize>, symbols: &'a SymbolTable) -> Result<(), di::Error> {
        let index = op.input(0).offset().real() as usize;
        let operand = symbols.symbol(self.walker
                                     .constructor()?
                                     .with_context(|| di::InvalidConstructor)?
                                     .operand(index)).with_context(|| di::InvalidSymbol)?;
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

    pub fn delay_slot(&mut self, symbols: &'a SymbolTable) -> Result<(), di::Error> {
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

            if let Some(ctpl) = self.walker.constructor()?.with_context(|| di::InvalidConstructor)?.template() {
                self.build(ctpl, None, symbols)?;
            }

            fall_offset += length;
            byte_count += length;

            // restote
            swap(&mut self.walker, &mut nwalker);
            //drop(nwalker);

            if byte_count >= delay_count {
                break
            }
        }

        self.unique_offset = old_unique_offset;
        Ok(())
    }

    pub fn generate_location(&mut self, varnode: &'a VarnodeTpl) -> Result<VarnodeData, di::Error> {
        let space = varnode.space().fix_space(&mut self.walker, self.manager)?
            .with_context(|| di::InconsistentState)?;
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

    pub fn generate_pointer(&mut self, varnode: &'a VarnodeTpl) -> Result<(Arc<AddressSpace>, VarnodeData), di::Error> {
        let handle = self.walker.handle(
            varnode.offset().handle_index().with_context(|| di::InconsistentState)?
        )?.with_context(|| di::InvalidHandle)?;

        let space = handle.offset_space.with_context(|| di::InconsistentState)?;
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

    pub fn dump(&mut self, op: &'a OpTpl) -> Result<(), di::Error> {
        let input_count = op.input_count();
        let mut inputs = Vec::new();

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
                    inputs: vec![index, ptr],
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
                    inputs: vec![index, ptr, outp],
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

    pub fn build(&mut self, constructor: &'a ConstructTpl, section_num: Option<usize>, symbols: &'a SymbolTable) -> Result<(), di::Error> {
        let old_base = self.label_base;
        self.label_base = self.label_count;
        self.label_count += constructor.labels();

        for op in constructor.operations() {
            match op.opcode() {
                Opcode::Build => {
                    self.append_build(op, section_num, symbols)?;
                },
                Opcode::DelaySlot => {
                    self.delay_slot(symbols)?;
                },
                Opcode::CrossBuild => {
                    return di::InconsistentState.fail()
                },
                _ => {
                    self.dump(op)?;
                }
            }
        }

        self.label_base = old_base;
        Ok(())
    }

    pub fn resolve_relatives(&mut self) {
        for rel in &self.label_refs {
            let varnode = &mut self.issued[rel.instruction].inputs[rel.index];
            let id = varnode.offset();
            if id >= self.labels.len() as u64 {
                panic!("no known ways to set a label...")
            }
            let res = (self.labels[id as usize] - rel.index as u64) & bits::calculate_mask(varnode.size());
            varnode.offset = res;
        }
    }

    pub fn emit(self, length: usize) -> PCode {
        let mut slf = self;
        slf.walker.base_state();
        PCode {
            address: slf.walker().address(),
            operations: slf.issued,
            delay_slots: slf.walker.delay_slot(),
            length,
        }
    }
}
