use crate::deserialise::parse::XmlExt;
use crate::deserialise::Error as DeserialiseError;

use crate::disassembly::construct::ConstructTpl;
use crate::disassembly::pattern::PatternExpression;
use crate::disassembly::symbol::{Operands, Symbol, SymbolTable};
use crate::disassembly::{Error, ParserWalker};

use crate::space_manager::SpaceManager;

use std::convert::TryFrom;
use std::fmt;
use std::mem::size_of;

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Context {
    Operator {
        num: usize,
        shift: u32,
        mask: u32,
        pattern_value: PatternExpression,
    },
    Commit {
        symbol_id: usize,
        num: usize,
        mask: u32,
        flow: bool,
    },
}

impl Context {
    pub fn apply<'b, 'c, 'z>(
        &'b self,
        walker: &mut ParserWalker<'b, 'c, 'z>,
        symbols: &'b SymbolTable,
    ) -> Result<(), Error> {
        Ok(match self {
            Self::Operator {
                num,
                shift,
                mask,
                pattern_value,
            } => {
                let value = pattern_value.value(walker, symbols)? as u32;
                let v = value.checked_shl(*shift).unwrap_or(0);
                walker.set_context_word(*num, v, *mask);
            }
            Self::Commit {
                symbol_id,
                num,
                mask,
                flow,
            } => {
                let sym = symbols.unchecked_symbol(*symbol_id); //.ok_or_else(|| Error::InvalidSymbol)?;
                walker.add_commit(sym, *num, *mask, *flow);
            }
        })
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        Ok(match input.tag_name().name() {
            "context_op" => Self::Operator {
                num: input.attribute_int("i")?,
                shift: input.attribute_int("shift")?,
                mask: input.attribute_int("mask")?,
                pattern_value: input
                    .children()
                    .filter(xml::Node::is_element)
                    .next()
                    .map(PatternExpression::from_xml)
                    .ok_or_else(|| {
                        DeserialiseError::Invariant("missing pattern for context_op")
                    })??,
            },
            "commit" => Self::Commit {
                symbol_id: input.attribute_int("id")?,
                num: input.attribute_int("num")?,
                mask: input.attribute_int("mask")?,
                flow: input.attribute_bool("flow")?,
            },
            name => return Err(DeserialiseError::TagUnexpected(name.to_owned())),
        })
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Constructor {
    id: (usize, usize),
    parent_id: usize,
    first_whitespace: Option<usize>,
    min_length: usize,
    source_file_index: usize,
    line_number: usize,
    operands: Vec<usize>,
    print_pieces: Vec<String>,
    context: Vec<Context>,
    template: Option<ConstructTpl>,
    named_template: Vec<Option<ConstructTpl>>,
    flow_through_index: Option<usize>,
}

impl PartialEq for Constructor {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Constructor {}

impl Constructor {
    pub fn apply_context<'b, 'c, 'z>(
        &'b self,
        walker: &mut ParserWalker<'b, 'c, 'z>,
        symbols: &'b SymbolTable,
    ) -> Result<(), Error> {
        for context in &self.context {
            context.apply(walker, symbols)?;
        }
        Ok(())
    }

    pub fn minimum_length(&self) -> usize {
        self.min_length
    }

    pub(crate) fn operands<'b, 'c, 'z>(
        &'b self,
        walker: &mut ParserWalker<'b, 'c, 'z>,
        symbols: &'b SymbolTable,
    ) -> Operands<'b> {
        let mut operands = Operands::new();
        self.operands_into(&mut operands, walker, symbols);
        operands
    }

    pub(crate) fn operands_into<'b, 'c, 'z>(
        &'b self,
        operands: &mut Operands<'b>,
        walker: &mut ParserWalker<'b, 'c, 'z>,
        symbols: &'b SymbolTable,
    ) {
        if let Some(index) = self.flow_through_index {
            match symbols
                .unchecked_symbol(self.operands[index])
                .defining_symbol(symbols)
            {
                Some(Symbol::Subtable { .. }) => {
                    walker.unchecked_push_operand(index);
                    walker
                        .unchecked_constructor()
                        .operands_into(operands, walker, symbols);
                    walker.unchecked_pop_operand();
                    return;
                }
                _ => (),
            }
        }
        if let Some(first_whitespace) = self.first_whitespace {
            for i in (first_whitespace + 1)..self.print_pieces.len() {
                if self.print_pieces[i].as_bytes()[0] == b'\n' {
                    let index = (self.print_pieces[i].as_bytes()[1] - b'A') as usize;
                    symbols
                        .unchecked_symbol(self.operands[index])
                        .collect_operands(operands, walker, symbols);
                }
            }
        }
    }

    pub(crate) fn collect_operands<'b, 'c, 'z>(
        &'b self,
        operands: &mut Operands<'b>,
        walker: &mut ParserWalker<'b, 'c, 'z>,
        symbols: &'b SymbolTable,
    ) {
        for p in &self.print_pieces {
            if p.as_bytes()[0] == b'\n' {
                let index = (p.as_bytes()[1] - b'A') as usize;
                symbols
                    .symbol(self.operands[index])
                    .expect("symbol")
                    .collect_operands(operands, walker, symbols);
            }
        }
    }

    pub fn format<'b, 'c, 'z>(
        &'b self,
        fmt: &mut fmt::Formatter,
        walker: &mut ParserWalker<'b, 'c, 'z>,
        symbols: &'b SymbolTable,
    ) -> Result<(), fmt::Error> {
        for p in &self.print_pieces {
            if p.as_bytes()[0] == b'\n' {
                let index = (p.as_bytes()[1] - b'A') as usize;
                symbols
                    .symbol(self.operands[index])
                    .expect("symbol")
                    .format(fmt, walker, symbols)?;
            } else {
                write!(fmt, "{}", p)?;
            }
        }
        Ok(())
    }

    pub fn format_mnemonic<'b, 'c, 'z>(
        &'b self,
        fmt: &mut fmt::Formatter,
        walker: &mut ParserWalker<'b, 'c, 'z>,
        symbols: &'b SymbolTable,
    ) -> Result<(), fmt::Error> {
        if let Some(index) = self.flow_through_index {
            match symbols
                .unchecked_symbol(self.operands[index])
                .defining_symbol(symbols)
            {
                Some(Symbol::Subtable { .. }) => {
                    walker.unchecked_push_operand(index);
                    walker
                        .unchecked_constructor()
                        .format_mnemonic(fmt, walker, symbols)?;
                    walker.unchecked_pop_operand();
                    return Ok(());
                }
                _ => (),
            }
        }
        let end = self.first_whitespace.unwrap_or(self.print_pieces.len());
        for i in 0..end {
            if self.print_pieces[i].as_bytes()[0] == b'\n' {
                let index = (self.print_pieces[i].as_bytes()[1] - b'A') as usize;
                symbols
                    .unchecked_symbol(self.operands[index])
                    .format(fmt, walker, symbols)?;
            } else {
                write!(fmt, "{}", self.print_pieces[i])?;
            }
        }
        Ok(())
    }

    pub fn format_body<'b, 'c, 'z>(
        &'b self,
        fmt: &mut fmt::Formatter,
        walker: &mut ParserWalker<'b, 'c, 'z>,
        symbols: &'b SymbolTable,
    ) -> Result<(), fmt::Error> {
        if let Some(index) = self.flow_through_index {
            match symbols
                .unchecked_symbol(self.operands[index])
                .defining_symbol(symbols)
            {
                Some(Symbol::Subtable { .. }) => {
                    walker.unchecked_push_operand(index);
                    walker
                        .unchecked_constructor()
                        .format_body(fmt, walker, symbols)?;
                    walker.unchecked_pop_operand();
                    return Ok(());
                }
                _ => (),
            }
        }
        if let Some(first_whitespace) = self.first_whitespace {
            for i in (first_whitespace + 1)..self.print_pieces.len() {
                if self.print_pieces[i].as_bytes()[0] == b'\n' {
                    let index = (self.print_pieces[i].as_bytes()[1] - b'A') as usize;
                    symbols
                        .unchecked_symbol(self.operands[index])
                        .format(fmt, walker, symbols)?;
                } else {
                    write!(fmt, "{}", self.print_pieces[i])?;
                }
            }
        }
        Ok(())
    }

    pub fn operand(&self, index: usize) -> usize {
        self.operands[index]
    }

    pub fn operand_count(&self) -> usize {
        self.operands.len()
    }

    pub fn template(&self) -> Option<&ConstructTpl> {
        self.template.as_ref()
    }

    pub fn unchecked_template(&self) -> &ConstructTpl {
        if let Some(ref templ) = self.template {
            templ
        } else {
            unreachable!()
        }
    }

    pub fn named_template(&self, index: usize) -> Option<&ConstructTpl> {
        self.named_template.get(index).and_then(|v| v.as_ref())
    }

    pub fn unchecked_named_template(&self, index: usize) -> &ConstructTpl {
        if let Some(ref named) = unsafe { self.named_template.get_unchecked(index) } {
            named
        } else {
            unreachable!()
        }
    }

    pub fn from_xml(
        input: xml::Node,
        id: (usize, usize),
        manager: &SpaceManager,
    ) -> Result<Self, DeserialiseError> {
        let mut operands = Vec::new();
        let mut print_pieces = Vec::new();
        let mut context = Vec::new();
        let mut template = None;
        let mut named_template = Vec::<Option<ConstructTpl>>::new();

        for input in input.children().filter(xml::Node::is_element) {
            match input.tag_name().name() {
                "oper" => {
                    operands.push(input.attribute_int("id")?);
                }
                "print" => {
                    print_pieces.push(input.attribute_string("piece")?);
                }
                "opprint" => {
                    let index = input.attribute_int::<u8>("id")?;
                    print_pieces.push(format!("\n{}", char::from(index + b'A')));
                }
                "context_op" | "commit" => {
                    context.push(Context::from_xml(input)?);
                }
                _ => {
                    let cur = ConstructTpl::from_xml(input, manager)?;
                    if let Some(section_id) = cur.section_id() {
                        if named_template.len() <= section_id {
                            named_template.resize_with(section_id + 1, Default::default);
                        }

                        if named_template[section_id].is_some() {
                            return Err(DeserialiseError::Invariant("duplicate named section"));
                        }

                        named_template[section_id] = Some(cur);
                    } else if template.is_none() {
                        template = Some(cur);
                    } else {
                        return Err(DeserialiseError::Invariant("duplicate main section"));
                    }
                }
            }
        }

        let flow_through_index =
            if print_pieces.len() == 1 && print_pieces[0].chars().nth(0).unwrap() == '\n' {
                Some((print_pieces[0].chars().nth(1).unwrap() as u8 - b'A') as usize)
            } else {
                None
            };

        let (source_file_index, line_number) = input.attribute_line_number("line")?;

        Ok(Self {
            id,
            parent_id: input.attribute_int("parent")?,
            first_whitespace: input.attribute_int::<i64>("first").map(|i| {
                if i < 0 {
                    None
                } else {
                    Some(i as usize)
                }
            })?,
            min_length: input.attribute_int("length")?,
            source_file_index,
            line_number,
            operands,
            print_pieces,
            context,
            template,
            named_template,
            flow_through_index,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct DecisionNode {
    number: usize,
    context_decision: bool,
    start_bit: usize,
    size: usize,
    patterns: Vec<DecisionPair>,
    children: Vec<DecisionNode>,
}

impl DecisionNode {
    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        let inputs = input.children().filter(xml::Node::is_element);
        let mut patterns = Vec::new();
        let mut children = Vec::new();
        for input in inputs {
            match input.tag_name().name() {
                "pair" => {
                    let id = input.attribute_int("id")?;
                    let pattern = DisjointPattern::from_xml(
                        input
                            .children()
                            .filter(xml::Node::is_element)
                            .next()
                            .ok_or_else(|| {
                                DeserialiseError::Invariant("no pattern for disjoint pattern")
                            })?,
                    )?;
                    patterns.push(DecisionPair { id, pattern });
                }
                "decision" => {
                    children.push(Self::from_xml(input)?);
                }
                _ => (),
            }
        }
        Ok(Self {
            number: input.attribute_int("number")?,
            context_decision: input.attribute_bool("context")?,
            start_bit: input.attribute_int("start")?,
            size: input.attribute_int("size")?,
            patterns,
            children,
        })
    }

    pub fn resolve<'b, 'c, 'z>(
        &'b self,
        walker: &mut ParserWalker<'b, 'c, 'z>,
        ctors: &'b [Constructor],
    ) -> Result<&'b Constructor, Error> {
        if self.size == 0 {
            for pattern in self.patterns.iter() {
                if pattern.is_match(walker) {
                    return ctors
                        .get(pattern.id)
                        .ok_or_else(|| Error::InvalidConstructor);
                }
            }
            Err(Error::InstructionResolution)
        } else {
            let val = if self.context_decision {
                walker.context_bits(self.start_bit, self.size)
            } else {
                walker.unchecked_instruction_bits(self.start_bit, self.size)
            };

            self.children[val as usize].resolve(walker, ctors)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct DecisionPair {
    id: usize,
    pattern: DisjointPattern,
}

impl DecisionPair {
    /*
    #[inline(always)]
    pub fn is_match<'b, 'c>(&'b self, walker: &ParserWalker<'b, 'c>) -> Result<bool, Error> {
        self.pattern.is_match(walker)
    }
    */

    #[inline(always)]
    pub fn is_match<'b, 'c, 'z>(&'b self, walker: &ParserWalker<'b, 'c, 'z>) -> bool {
        self.pattern.is_match(walker)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum DisjointPattern {
    Instruction(InstructionPattern),
    Context(ContextPattern),
    Combine {
        context: ContextPattern,
        instruction: InstructionPattern,
    },
}

impl DisjointPattern {
    #[inline(always)]
    pub fn is_match<'b, 'c, 'z>(&'b self, walker: &ParserWalker<'b, 'c, 'z>) -> bool {
        match self {
            Self::Instruction(ref pat) => pat.is_match(walker),
            Self::Context(ref pat) => pat.is_match(walker),
            Self::Combine {
                ref context,
                ref instruction,
            } => instruction.is_match(walker) && context.is_match(walker),
        }
    }

    /*
    #[inline(always)]
    pub fn is_match<'b, 'c>(&'b self, walker: &ParserWalker<'b, 'c>) -> Result<bool, Error> {
        Ok(match self {
            Self::Instruction(ref pat) => pat.is_match(walker)?,
            Self::Context(ref pat) => pat.is_match(walker),
            Self::Combine {
                ref context,
                ref instruction,
            } => instruction.is_match(walker)? && context.is_match(walker),
        })
    }
    */

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        Ok(match input.tag_name().name() {
            "instruct_pat" => Self::Instruction(InstructionPattern::from_xml(input)?),
            "context_pat" => Self::Context(ContextPattern::from_xml(input)?),
            _ => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Combine {
                    context: ContextPattern::from_xml(
                        children.next().ok_or_else(|| {
                            DeserialiseError::Invariant("missing context pattern")
                        })?,
                    )?,
                    instruction: InstructionPattern::from_xml(children.next().ok_or_else(
                        || DeserialiseError::Invariant("missing instruction pattern"),
                    )?)?,
                }
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct InstructionPattern {
    mask_value: PatternBlock,
}

impl InstructionPattern {
    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        Ok(Self {
            mask_value: PatternBlock::from_xml(
                input
                    .children()
                    .filter(xml::Node::is_element)
                    .next()
                    .ok_or_else(|| DeserialiseError::Invariant("missing pattern block"))?,
            )?,
        })
    }

    /*
    #[inline(always)]
    pub fn is_match<'b, 'c>(&'b self, walker: &ParserWalker<'b, 'c>) -> Result<bool, Error> {
        self.mask_value.is_instruction_match(walker)
    }
    */

    #[inline(always)]
    pub fn is_match<'b, 'c, 'z>(&'b self, walker: &ParserWalker<'b, 'c, 'z>) -> bool {
        self.mask_value.is_instruction_match(walker)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ContextPattern {
    mask_value: PatternBlock,
}

impl ContextPattern {
    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        Ok(Self {
            mask_value: PatternBlock::from_xml(
                input
                    .children()
                    .filter(xml::Node::is_element)
                    .next()
                    .ok_or_else(|| DeserialiseError::Invariant("missing pattern block"))?,
            )?,
        })
    }

    #[inline(always)]
    pub fn is_match<'b, 'c, 'z>(&'b self, walker: &ParserWalker<'b, 'c, 'z>) -> bool {
        self.mask_value.is_context_match(walker)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct PatternBlock {
    offset: usize,
    non_zero_size: Option<usize>,
    masks: Vec<u32>,
    values: Vec<u32>,
}

impl PatternBlock {
    const ALWAYS_TRUE: Option<usize> = Some(0);
    const ALWAYS_FALSE: Option<usize> = None;

    pub fn new(always: bool) -> Self {
        Self {
            offset: 0,
            non_zero_size: if always {
                Self::ALWAYS_TRUE
            } else {
                Self::ALWAYS_FALSE
            },
            masks: Vec::new(),
            values: Vec::new(),
        }
    }

    pub fn always_true(&self) -> bool {
        self.non_zero_size == Self::ALWAYS_TRUE
    }

    pub fn always_false(&self) -> bool {
        self.non_zero_size == Self::ALWAYS_FALSE
    }

    pub fn is_context_match<'b, 'c, 'z>(&'b self, walker: &ParserWalker<'b, 'c, 'z>) -> bool {
        match self.non_zero_size {
            Self::ALWAYS_FALSE => false,
            Self::ALWAYS_TRUE => true,
            _ => {
                let mut offset = self.offset;
                for i in 0..self.values.len() {
                    let data = walker.context_bytes(offset, size_of::<u32>());
                    if self.masks[i] & data != self.values[i] {
                        return false;
                    }
                    offset += size_of::<u32>();
                }
                true
            }
        }
    }

    /*
    pub fn is_instruction_match<'b, 'c>(&'b self, walker: &ParserWalker<'b, 'c>) -> Result<bool, Error> {
        Ok(match self.non_zero_size {
            Self::ALWAYS_FALSE => false,
            Self::ALWAYS_TRUE => true,
            _ => {
                let mut offset = self.offset;
                for i in 0..self.values.len() {
                    let data = walker.instruction_bytes(offset, size_of::<u32>())?;
                    if self.masks[i] & data != self.values[i] {
                        return Ok(false)
                    }
                    offset += size_of::<u32>();
                }
                true
            }
        })
    }
    */

    pub fn is_instruction_match<'b, 'c, 'z>(&'b self, walker: &ParserWalker<'b, 'c, 'z>) -> bool {
        match self.non_zero_size {
            Self::ALWAYS_FALSE => false,
            Self::ALWAYS_TRUE => true,
            _ => {
                let mut offset = self.offset;
                for i in 0..self.values.len() {
                    let data = walker.unchecked_instruction_bytes(offset, size_of::<u32>());
                    if self.masks[i] & data != self.values[i] {
                        return false;
                    }
                    offset += size_of::<u32>();
                }
                true
            }
        }
    }

    pub fn shift(&mut self, shift: isize) {
        let noffset = isize::try_from(self.offset).expect("PatternBlock shift offset") + shift;
        self.offset = if noffset < 0 {
            0usize
        } else {
            noffset as usize
        };
        self.normalise()
    }

    pub fn normalise(&mut self) {
        if self.non_zero_size == Self::ALWAYS_FALSE || self.non_zero_size == Self::ALWAYS_TRUE {
            self.offset = 0;
            self.masks.clear();
            self.values.clear();
            return;
        }

        let mut masks = &mut self.masks[..];
        let mut values = &mut self.values[..];

        if let Some(index) = masks.iter().position(|v| *v != 0) {
            self.offset += index * size_of::<u32>();
            masks = &mut masks[index..];
            values = &mut values[index..];
        }

        if !masks.is_empty() {
            let mut suboff = 0;
            let mut tmp = *masks.first().unwrap();
            while tmp != 0 {
                suboff += 1;
                tmp >>= 8;
            }

            suboff = size_of::<u32>() - suboff;
            if suboff != 0 {
                self.offset += suboff;

                for i in 0..masks.len() - 1 {
                    tmp = masks[i] << (suboff * 8);
                    tmp = tmp | (masks[i + 1] >> ((size_of::<u32>() - suboff) * 8));
                    masks[i] = tmp;

                    tmp = values[i] << (suboff * 8);
                    tmp = tmp | (values[i + 1] >> ((size_of::<u32>() - suboff) * 8));
                    values[i] = tmp;
                }

                *masks.last_mut().unwrap() <<= suboff * 8;
                *values.last_mut().unwrap() <<= suboff * 8;

                let rindex = masks
                    .iter()
                    .rposition(|v| *v != 0)
                    .map(|v| v + 1)
                    .unwrap_or(0);

                masks = &mut masks[..rindex];
                values = &mut values[..rindex];
            }
        }

        if masks.is_empty() {
            self.offset = 0;
            self.non_zero_size = Self::ALWAYS_TRUE;
            self.masks.clear();
            self.values.clear();
            return;
        }

        // this can probably be done in-place without
        // the extra vec allocation

        self.masks = masks.to_vec();
        self.values = values.to_vec();

        let mut non_zero_size = self.masks.len() * size_of::<u32>();
        let mut tmp = *self.masks.last().unwrap();
        while (tmp & 0xff) == 0 {
            non_zero_size -= 1;
            tmp >>= 8;
        }
        self.non_zero_size = Some(non_zero_size);
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        let mut masks = Vec::new();
        let mut values = Vec::new();

        for input in input.children().filter(xml::Node::is_element) {
            masks.push(input.attribute_int("mask")?);
            values.push(input.attribute_int("val")?);
        }

        let mut slf = Self {
            offset: input.attribute_int("offset")?,
            non_zero_size: Some(input.attribute_int("nonzero")?),
            masks,
            values,
        };
        slf.normalise();
        Ok(slf)
    }
}
