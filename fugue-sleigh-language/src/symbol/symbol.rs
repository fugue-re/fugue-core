use ustr::Ustr;

use crate::deserialise::{DeserialiseError, XmlExt};
use crate::pattern::PatternExpression;
use crate::symbol::{Constructor, DecisionNode, SymbolTable};
use crate::spaces::{AddressSpaceId, AddressSpaces};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum SymbolKind {
    UserOp,
    Epsilon,
    Value,
    ValueMap,
    Name,
    Varnode,
    Context,
    VarnodeList,
    Operand,
    Start,
    End,
    Next2,
    Subtable,
    FlowDest,
    FlowRef,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Symbol {
    UserOp {
        id: usize,
        scope: usize,
        name: Ustr,
        index: usize,
    },
    Epsilon {
        id: usize,
        scope: usize,
        name: Ustr,
    },
    Value {
        id: usize,
        scope: usize,
        name: Ustr,
        pattern_value: PatternExpression,
    },
    ValueMap {
        id: usize,
        scope: usize,
        name: Ustr,
        pattern_value: PatternExpression,
        value_table: Vec<i64>,
        table_is_filled: bool,
    },
    Name {
        id: usize,
        scope: usize,
        name: Ustr,
        pattern_value: PatternExpression,
        name_table: Vec<String>,
        table_is_filled: bool,
    },
    Varnode {
        id: usize,
        scope: usize,
        name: Ustr,
        space: AddressSpaceId,
        offset: u64,
        size: usize,
    },
    Context {
        id: usize,
        scope: usize,
        name: Ustr,
        pattern_value: PatternExpression,
        varnode_id: usize,
        high: usize,
        low: usize,
        flow: bool,
    },
    VarnodeList {
        id: usize,
        scope: usize,
        name: Ustr,
        pattern_value: PatternExpression,
        varnode_table: Vec<Option<usize>>,
        table_is_filled: bool,
    },
    Operand {
        id: usize,
        scope: usize,
        name: Ustr,
        handle_index: usize,
        offset: usize,
        base: Option<usize>,
        min_length: usize,
        subsym_id: Option<usize>,
        is_code: bool,
        local_expr: PatternExpression,
        def_expr: Option<PatternExpression>,
    },
    Start {
        id: usize,
        scope: usize,
        name: Ustr,
        pattern_value: PatternExpression,
    },
    End {
        id: usize,
        scope: usize,
        name: Ustr,
        pattern_value: PatternExpression,
    },
    Next2 {
        id: usize,
        scope: usize,
        name: Ustr,
        pattern_value: PatternExpression,
    },
    Subtable {
        id: usize,
        scope: usize,
        name: Ustr,
        constructors: Vec<Constructor>,
        decision_tree: DecisionNode,
    },
    FlowDest {
        id: usize,
        scope: usize,
        name: Ustr,
    },
    FlowRef {
        id: usize,
        scope: usize,
        name: Ustr,
    },
}

impl Symbol {
    pub fn id(&self) -> usize {
        match self {
            Self::UserOp { id, .. }
            | Self::Epsilon { id, .. }
            | Self::Value { id, .. }
            | Self::ValueMap { id, .. }
            | Self::Name { id, .. }
            | Self::Varnode { id, .. }
            | Self::Context { id, .. }
            | Self::VarnodeList { id, .. }
            | Self::Operand { id, .. }
            | Self::Start { id, .. }
            | Self::End { id, .. }
            | Self::Next2 { id, .. }
            | Self::Subtable { id, .. }
            | Self::FlowDest { id, .. }
            | Self::FlowRef { id, .. } => *id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::UserOp { ref name, .. }
            | Self::Epsilon { ref name, .. }
            | Self::Value { ref name, .. }
            | Self::ValueMap { ref name, .. }
            | Self::Name { ref name, .. }
            | Self::Varnode { ref name, .. }
            | Self::Context { ref name, .. }
            | Self::VarnodeList { ref name, .. }
            | Self::Operand { ref name, .. }
            | Self::Start { ref name, .. }
            | Self::End { ref name, .. }
            | Self::Next2 { ref name, .. }
            | Self::Subtable { ref name, .. }
            | Self::FlowDest { ref name, .. }
            | Self::FlowRef { ref name, .. } => name,
        }
    }

    pub fn minimum_length(&self) -> Option<usize> {
        if let Self::Operand { min_length, .. } = self {
            Some(*min_length)
        } else {
            None
        }
    }

    pub fn offset_base(&self) -> Option<usize> {
        if let Self::Operand { base, .. } = self {
            *base
        } else {
            unreachable!()
        }
    }

    pub fn relative_offset(&self) -> usize {
        if let Self::Operand { offset, .. } = self {
            *offset
        } else {
            unreachable!()
        }
    }

    pub fn defining_expression(&self) -> Option<&PatternExpression> {
        if let Self::Operand { def_expr, .. } = self {
            def_expr.as_ref()
        } else {
            unreachable!()
        }
    }

    pub fn defining_symbol<'b>(&self, symbols: &'b SymbolTable) -> Option<&'b Symbol> {
        if let Self::Operand { subsym_id, .. } = self {
            symbols.symbol(subsym_id.as_ref().copied()?)
        } else {
            unreachable!()
        }
    }

    pub fn is_subtable(&self) -> bool {
        matches!(self, Self::Subtable { .. })
    }

    pub fn is_operand(&self) -> bool {
        matches!(self, Self::Operand { .. })
    }

    pub fn pattern_value(&self) -> &PatternExpression {
        match self {
            Self::Value { pattern_value, .. }
            | Self::ValueMap { pattern_value, .. }
            | Self::Name { pattern_value, .. }
            | Self::Context { pattern_value, .. }
            | Self::VarnodeList { pattern_value, .. }
            | Self::Start { pattern_value, .. }
            | Self::End { pattern_value, .. }
            | Self::Next2 { pattern_value, .. } => pattern_value,
            Self::Operand { local_expr, .. } => local_expr,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolBuilder {
    pub(super) kind: SymbolKind,
    pub(super) id: usize,
    pub(super) scope: usize,
    pub(super) name: Ustr,
}

impl Default for SymbolBuilder {
    fn default() -> Self {
        Self {
            kind: SymbolKind::UserOp,
            id: 0,
            scope: 0,
            name: Ustr::from(""),
        }
    }
}

impl SymbolBuilder {
    pub fn build_from_xml<'a>(
        self,
        spaces: &'a AddressSpaces,
        input: xml::Node,
    ) -> Result<Symbol, DeserialiseError> {
        Ok(match self.kind {
            SymbolKind::UserOp => {
                if input.tag_name().name() != "userop" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }
                Symbol::UserOp {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    index: input.attribute_int("index")?,
                }
            }
            SymbolKind::Epsilon => {
                if input.tag_name().name() != "epsilon" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }
                Symbol::Epsilon {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                }
            }
            SymbolKind::Value => {
                if input.tag_name().name() != "value_sym" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }
                let pattern_value = PatternExpression::from_xml(
                    input
                        .children()
                        .filter(xml::Node::is_element)
                        .next()
                        .ok_or_else(|| {
                            DeserialiseError::Invariant("missing pattern expression for value")
                        })?,
                )?;

                Symbol::Value {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    pattern_value,
                }
            }
            SymbolKind::ValueMap => {
                if input.tag_name().name() != "valuemap_sym" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }
                let mut children = input.children().filter(xml::Node::is_element);
                let pattern_value =
                    PatternExpression::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing pattern expression for name")
                    })?)?;

                let value_table = children
                    .map(|v| v.attribute_int("val"))
                    .collect::<Result<Vec<i64>, _>>()?;

                let min = pattern_value
                    .min_value()
                    .ok_or_else(|| DeserialiseError::Invariant("invalid pattern"))?;
                let max = pattern_value
                    .max_value()
                    .ok_or_else(|| DeserialiseError::Invariant("invalid pattern"))?;

                let table_is_filled = min >= 0
                    && (max as i64) < value_table.len() as i64
                    && !value_table.iter().any(|v| *v == 0xbadbeef);

                Symbol::ValueMap {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    pattern_value,
                    value_table,
                    table_is_filled,
                }
            }
            SymbolKind::Name => {
                if input.tag_name().name() != "name_sym" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }
                let mut children = input.children().filter(xml::Node::is_element);
                let pattern_value =
                    PatternExpression::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing pattern expression for value")
                    })?)?;

                let name_table = children
                    .map(|v| {
                        let mut s = v.attribute_string_opt("name", "\t");
                        if s == "_" {
                            s = "\t".to_string()
                        };
                        s
                    })
                    .collect::<Vec<String>>();

                let min = pattern_value
                    .min_value()
                    .ok_or_else(|| DeserialiseError::Invariant("invalid pattern"))?;
                let max = pattern_value
                    .max_value()
                    .ok_or_else(|| DeserialiseError::Invariant("invalid pattern"))?;

                let table_is_filled = min >= 0
                    && (max as i64) < name_table.len() as i64
                    && !name_table.iter().any(|v| v == "\t");

                Symbol::Name {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    pattern_value,
                    name_table,
                    table_is_filled,
                }
            }
            SymbolKind::Varnode => {
                if input.tag_name().name() != "varnode_sym" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }

                let space_name = input
                    .attribute("space")
                    .ok_or_else(|| DeserialiseError::AttributeExpected("space"))?;

                let space = spaces
                    .space_by_name(space_name)
                    .ok_or_else(|| DeserialiseError::Invariant("varnode space not defined"))?;

                let offset = input.attribute_int_or("offset", "off")?;
                let size = input.attribute_int("size")?;

                Symbol::Varnode {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    space: space.id(),
                    offset,
                    size,
                }
            }
            SymbolKind::Context => {
                if input.tag_name().name() != "context_sym" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }

                let pattern_value = PatternExpression::from_xml(
                    input
                        .children()
                        .filter(xml::Node::is_element)
                        .next()
                        .ok_or_else(|| {
                            DeserialiseError::Invariant("missing pattern expression for context")
                        })?,
                )?;

                let varnode_id = input.attribute_int("varnode")?;
                let high = input.attribute_int("high")?;
                let low = input.attribute_int("low")?;
                let flow = input.attribute_bool("flow")?;

                Symbol::Context {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    pattern_value,
                    varnode_id,
                    high,
                    low,
                    flow,
                }
            }
            SymbolKind::VarnodeList => {
                if input.tag_name().name() != "varlist_sym" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }
                let mut children = input.children().filter(xml::Node::is_element);
                let pattern_value =
                    PatternExpression::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing pattern expression for varnodelist")
                    })?)?;

                let varnode_table = children
                    .map(|input| {
                        Ok(if input.tag_name().name() == "var" {
                            Some(input.attribute_int("id")?)
                        } else {
                            None
                        })
                    })
                    .collect::<Result<Vec<Option<usize>>, DeserialiseError>>()?;

                let min = pattern_value
                    .min_value()
                    .ok_or_else(|| DeserialiseError::Invariant("invalid pattern"))?;
                let max = pattern_value
                    .max_value()
                    .ok_or_else(|| DeserialiseError::Invariant("invalid pattern"))?;

                let table_is_filled = min >= 0
                    && (max as i64) < varnode_table.len() as i64
                    && !varnode_table.iter().any(Option::is_none);

                Symbol::VarnodeList {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    pattern_value,
                    varnode_table,
                    table_is_filled,
                }
            }
            SymbolKind::Operand => {
                if input.tag_name().name() != "operand_sym" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }

                let handle_index = input.attribute_int("index")?;
                let offset = input.attribute_int("off")?;
                let base = input.attribute_int::<i64>("base").map(|v| {
                    if v < 0 {
                        None
                    } else {
                        Some(v as usize)
                    }
                })?;

                let min_length = input.attribute_int("minlen")?;

                let subsym_id = if input.attribute("subsym").is_none() {
                    None
                } else {
                    Some(input.attribute_int("subsym")?)
                };

                let is_code = if input.attribute("code").is_none() {
                    false
                } else {
                    input.attribute_bool("code")?
                };

                let mut children = input.children().filter(xml::Node::is_element);
                let local_expr =
                    PatternExpression::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing local expression for operand")
                    })?)?;

                let def_expr = children
                    .next()
                    .map(PatternExpression::from_xml)
                    .transpose()?;

                Symbol::Operand {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    handle_index,
                    offset,
                    base,
                    min_length,
                    subsym_id,
                    is_code,
                    local_expr,
                    def_expr,
                }
            }
            SymbolKind::Start => {
                if input.tag_name().name() != "start_sym" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }

                Symbol::Start {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    pattern_value: PatternExpression::StartInstruction,
                }
            }
            SymbolKind::End => {
                if input.tag_name().name() != "end_sym" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }

                Symbol::End {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    pattern_value: PatternExpression::EndInstruction,
                }
            }
            SymbolKind::Next2 => {
                if input.tag_name().name() != "next2_sym" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }

                Symbol::Next2 {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    pattern_value: PatternExpression::Next2Instruction,
                }
            }
            SymbolKind::FlowDest => {
                if input.tag_name().name() != "flowdest_sym" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }

                Symbol::FlowDest {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                }
            }
            SymbolKind::FlowRef => {
                if input.tag_name().name() != "flowref_sym" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }

                Symbol::FlowDest {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                }
            }
            SymbolKind::Subtable => {
                if input.tag_name().name() != "subtable_sym" {
                    return Err(DeserialiseError::TagUnexpected(
                        input.tag_name().name().to_owned(),
                    ));
                }

                let mut constructors = Vec::new();
                let mut decision_root = None;
                for (id, input) in input.children().filter(xml::Node::is_element).enumerate() {
                    match input.tag_name().name() {
                        "constructor" => {
                            constructors.push(Constructor::from_xml(input, (self.id, id), spaces)?)
                        }
                        "decision" => {
                            if decision_root.is_none() {
                                decision_root = Some(DecisionNode::from_xml(input)?);
                            } else {
                                return Err(DeserialiseError::Invariant(
                                    "redefintion of root decision tree node",
                                ));
                            }
                        }
                        _ => (),
                    }
                }

                Symbol::Subtable {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    constructors,
                    decision_tree: decision_root.ok_or_else(|| {
                        DeserialiseError::Invariant("missing decision tree for subtable")
                    })?,
                }
            }
        })
    }
}
