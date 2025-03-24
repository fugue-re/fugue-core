use std::mem::size_of;

use fugue_ghidra_marshal::sla::*;
use fugue_ghidra_marshal::Decoder;

use crate::construct::ConstructTpl;
use crate::deserialise::{DeserialiseError, XmlExt};
use crate::pattern::PatternExpression;
use crate::spaces::AddressSpaces;

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
    pub fn from_decoder<D: Decoder>(input: &mut D, kind: u32) -> Result<Self, DeserialiseError> {
        // NOTE: we assume the element is opened
        #[cfg(feature = "tracing")]
        tracing::trace!("decoding context operation");

        Ok(match kind {
            ELEM_CONTEXT_OP_ID => {
                let num = input.read_signed_integer_with_id(&ATTRIB_I)? as usize;
                let shift = input.read_signed_integer_with_id(&ATTRIB_SHIFT)? as u32;
                let mask = input.read_unsigned_integer_with_id(&ATTRIB_MASK)? as u32;
                let pattern_value = PatternExpression::from_decoder(input)?;
                Self::Operator {
                    num,
                    shift,
                    mask,
                    pattern_value,
                }
            }
            ELEM_COMMIT_ID => {
                let symbol_id = input.read_unsigned_integer_with_id(&ATTRIB_ID)? as usize;
                let num = input.read_signed_integer_with_id(&ATTRIB_NUMBER)? as usize;
                let mask = input.read_unsigned_integer_with_id(&ATTRIB_MASK)? as u32;
                let flow = input.read_bool_with_id(&ATTRIB_FLOW)?;
                Self::Commit {
                    symbol_id,
                    num,
                    mask,
                    flow,
                }
            }
            _ => {
                return Err(DeserialiseError::ElementUnexpected(kind));
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
                num: input.attribute_int_or("num", "number")?,
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
    pub fn id(&self) -> (usize, usize) {
        self.id
    }

    pub fn context(&self) -> &[Context] {
        &self.context
    }

    pub fn minimum_length(&self) -> usize {
        self.min_length
    }

    pub fn print_pieces(&self) -> &[String] {
        &self.print_pieces
    }

    pub fn first_whitespace(&self) -> Option<usize> {
        self.first_whitespace
    }

    pub fn flow_through_index(&self) -> Option<usize> {
        self.flow_through_index
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

    pub unsafe fn unchecked_template(&self) -> &ConstructTpl {
        if let Some(ref templ) = self.template {
            templ
        } else {
            unreachable!()
        }
    }

    pub fn named_template(&self, index: usize) -> Option<&ConstructTpl> {
        self.named_template.get(index).and_then(|v| v.as_ref())
    }

    pub unsafe fn unchecked_named_template(&self, index: usize) -> &ConstructTpl {
        if let Some(ref named) = self.named_template.get_unchecked(index) {
            named
        } else {
            unreachable!()
        }
    }

    pub fn from_decoder<D: Decoder>(
        spaces: &AddressSpaces,
        input: &mut D,
        id: (usize, usize),
    ) -> Result<Self, DeserialiseError> {
        // NOTE: we assume the element is opened
        #[cfg(feature = "tracing")]
        tracing::trace!("decoding constructor {id:?}");

        let mut operands = Vec::new();
        let mut print_pieces = Vec::new();
        let mut context = Vec::new();
        let mut template = None;
        let mut named_template = Vec::<Option<ConstructTpl>>::new();

        let parent = input.read_unsigned_integer_with_id(&ATTRIB_PARENT)? as usize;
        let first_whitespace = input.read_signed_integer_with_id(&ATTRIB_FIRST)?;
        let min_length = input.read_signed_integer_with_id(&ATTRIB_LENGTH)? as usize;
        let source_file_index = input.read_signed_integer_with_id(&ATTRIB_SOURCE)? as usize;
        let line_number = input.read_signed_integer_with_id(&ATTRIB_LINE)? as usize;

        while input.peek_element()? != 0 {
            let elem = input.open_element()?;

            match elem {
                ELEM_OPER_ID => {
                    operands.push(input.read_unsigned_integer_with_id(&ATTRIB_ID)? as usize);
                }
                ELEM_PRINT_ID => {
                    print_pieces.push(input.read_string_with_id(&ATTRIB_PIECE)?);
                }
                ELEM_OPPRINT_ID => {
                    let index = input.read_signed_integer_with_id(&ATTRIB_ID)? as u8;
                    print_pieces.push(format!("\n{}", char::from(index + b'A')));
                }
                ELEM_CONTEXT_OP_ID | ELEM_COMMIT_ID => {
                    context.push(Context::from_decoder(input, elem)?);
                }
                ELEM_CONSTRUCT_TPL_ID => {
                    let cur = ConstructTpl::from_decoder(spaces, input)?;
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
                _ => {
                    return Err(DeserialiseError::ElementUnexpected(elem));
                }
            }

            input.close_element(elem)?;
        }

        let flow_through_index =
            if print_pieces.len() == 1 && print_pieces[0].chars().nth(0).unwrap() == '\n' {
                Some((print_pieces[0].chars().nth(1).unwrap() as u8 - b'A') as usize)
            } else {
                None
            };

        Ok(Self {
            id,
            parent_id: parent,
            first_whitespace: if first_whitespace < 0 {
                None
            } else {
                Some(first_whitespace as usize)
            },
            min_length,
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

    pub fn from_xml(
        input: xml::Node,
        id: (usize, usize),
        spaces: &AddressSpaces,
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
                    let cur = ConstructTpl::from_xml(input, spaces)?;
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

        // for v4 and v3
        let (source_file_index, line_number) = input
            .attribute_int("source")
            .and_then(|index| input.attribute_int("line").map(|line| (index, line)))
            .or_else(|_| input.attribute_line_number("line"))?;

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
    pub fn number(&self) -> usize {
        self.number
    }

    pub fn context_decision(&self) -> bool {
        self.context_decision
    }

    pub fn start_bit(&self) -> usize {
        self.start_bit
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn patterns(&self) -> &[DecisionPair] {
        &self.patterns
    }

    pub fn children(&self) -> &[DecisionNode] {
        &self.children
    }

    pub fn from_decoder<D: Decoder>(
        input: &mut D,
    ) -> Result<Self, DeserialiseError> {
        // NOTE: we assume the element is opened
        #[cfg(feature = "tracing")]
        tracing::trace!("decoding decision node");

        let number = input.read_signed_integer_with_id(&ATTRIB_NUMBER)? as usize;
        let context_decision = input.read_bool_with_id(&ATTRIB_CONTEXT)?;
        let start_bit = input.read_signed_integer_with_id(&ATTRIB_STARTBIT)? as usize;
        let size = input.read_signed_integer_with_id(&ATTRIB_SIZE)? as usize;

        let mut patterns = Vec::new();
        let mut children = Vec::new();

        while input.peek_element()? != 0 {
            let elem = input.open_element()?;

            match elem {
                ELEM_PAIR_ID => {
                    let id = input.read_signed_integer_with_id(&ATTRIB_ID)? as usize;
                    let pattern = DisjointPattern::from_decoder(input)?;
                    patterns.push(DecisionPair { id, pattern });
                }
                ELEM_DECISION_ID => {
                    children.push(Self::from_decoder(input)?);
                }
                _ => (),
            }

            input.close_element(elem)?;
        }

        Ok(Self {
            number,
            context_decision,
            start_bit,
            size,
            patterns,
            children,
        })
    }

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
            start_bit: input.attribute_int_or("start", "startbit")?,
            size: input.attribute_int("size")?,
            patterns,
            children,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct DecisionPair {
    id: usize,
    pattern: DisjointPattern,
}

impl DecisionPair {
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn pattern(&self) -> &DisjointPattern {
        &self.pattern
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
    pub fn from_decoder<D: Decoder>(
        input: &mut D,
    ) -> Result<Self, DeserialiseError> {
        #[cfg(feature = "tracing")]
        tracing::trace!("decoding decision node");

        let elem = input.peek_element()?;

        let pattern = match elem {
            ELEM_INSTRUCT_PAT_ID => {
                Self::Instruction(InstructionPattern::from_decoder(input)?)
            }
            ELEM_CONTEXT_PAT_ID => Self::Context(ContextPattern::from_decoder(input)?),
            _ => {
                let sub_elem = input.open_element_with_id(&ELEM_COMBINE_PAT)?;
                let pattern = Self::Combine {
                    context: ContextPattern::from_decoder(input)?,
                    instruction: InstructionPattern::from_decoder(input)?,
                };
                input.close_element(sub_elem)?;
                pattern
            }
        };

        Ok(pattern)
    }

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
    pub fn mask_value(&self) -> &PatternBlock {
        &self.mask_value
    }

    pub fn from_decoder<D: Decoder>(
        input: &mut D,
    ) -> Result<Self, DeserialiseError> {
        #[cfg(feature = "tracing")]
        tracing::trace!("decoding instruction pattern");

        let elem = input.open_element_with_id(&ELEM_INSTRUCT_PAT)?;

        let mask_value = PatternBlock::from_decoder(input)?;

        input.close_element(elem)?;

        Ok(Self { mask_value })
    }

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
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ContextPattern {
    mask_value: PatternBlock,
}

impl ContextPattern {
    pub fn mask_value(&self) -> &PatternBlock {
        &self.mask_value
    }

    pub fn from_decoder<D: Decoder>(
        input: &mut D,
    ) -> Result<Self, DeserialiseError> {
        #[cfg(feature = "tracing")]
        tracing::trace!("decoding context pattern");

        let elem = input.open_element_with_id(&ELEM_CONTEXT_PAT)?;

        let mask_value = PatternBlock::from_decoder(input)?;

        input.close_element(elem)?;

        Ok(Self { mask_value })
    }

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

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn non_zero_size(&self) -> Option<usize> {
        self.non_zero_size
    }

    pub fn masks(&self) -> &[u32] {
        &self.masks
    }

    pub fn values(&self) -> &[u32] {
        &self.values
    }

    fn normalise(&mut self) {
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

    pub fn from_decoder<D: Decoder>(
        input: &mut D,
    ) -> Result<Self, DeserialiseError> {
        #[cfg(feature = "tracing")]
        tracing::trace!("decoding pattern block");

        let elem = input.open_element_with_id(&ELEM_PAT_BLOCK)?;

        let offset = input.read_signed_integer_with_id(&ATTRIB_OFF)? as usize;
        let non_zero_size = input.read_signed_integer_with_id(&ATTRIB_NONZERO)? as usize;

        let mut masks = Vec::new();
        let mut values = Vec::new();

        while input.peek_element()? != 0 {
            let sub_elem = input.open_element_with_id(&ELEM_MASK_WORD)?;

            masks.push(input.read_unsigned_integer_with_id(&ATTRIB_MASK)? as u32);
            values.push(input.read_unsigned_integer_with_id(&ATTRIB_VAL)? as u32);

            input.close_element(sub_elem)?;
        }

        input.close_element(elem)?;

        let mut slf = Self {
            offset,
            non_zero_size: Some(non_zero_size),
            masks,
            values,
        };
        slf.normalise();
        Ok(slf)
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        let mut masks = Vec::new();
        let mut values = Vec::new();

        for input in input.children().filter(xml::Node::is_element) {
            masks.push(input.attribute_int("mask")?);
            values.push(input.attribute_int("val")?);
        }

        let mut slf = Self {
            offset: input.attribute_int_or("offset", "off")?,
            non_zero_size: Some(input.attribute_int("nonzero")?),
            masks,
            values,
        };
        slf.normalise();
        Ok(slf)
    }
}
