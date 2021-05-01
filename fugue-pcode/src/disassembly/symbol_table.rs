use crate::disassembly::ParserWalker;
use crate::error::deserialisation as de;
use crate::error::disassembly as di;
use crate::parse::XmlExt;
use crate::pattern::PatternExpression;
use crate::space::AddressSpace;
use crate::space_manager::SpaceManager;
use crate::subtable::{Constructor, DecisionNode};

use std::collections::BTreeSet as Set;
use std::fmt;
use std::mem::take;
use std::sync::Arc;
use snafu::OptionExt;

#[derive(Debug, Clone)]
pub struct FixedHandle<'a> {
    pub space: &'a AddressSpace,
    pub size: usize,
    pub offset_space: Option<&'a AddressSpace>,
    pub offset_offset: u64,
    pub offset_size: usize,
    pub temporary_space: Option<&'a AddressSpace>,
    pub temporary_offset: u64,
}

impl<'a> FixedHandle<'a> {
    pub fn new(space: &'a AddressSpace) -> Self {
        Self {
            space,
            size: 0,
            offset_space: None,
            offset_offset: 0,
            offset_size: 0,
            temporary_space: None,
            temporary_offset: 0,
        }
    }
}


#[derive(Debug, Clone, Default)]
pub struct SymbolScope {
    id: usize,
    parent: usize,
    tree: Set<usize>,
}

impl SymbolScope {
    pub fn add_symbol(&mut self, symbol: usize) {
        self.tree.insert(symbol);
    }

    pub fn iter(&self) -> impl Iterator<Item=&usize> {
        self.tree.iter()
    }

    pub fn find(&self, name: &str, table: &SymbolTable) -> Option<usize> {
        self.tree.iter().find_map(|id| table.symbol(*id).and_then(|sym| {
            if sym.name() == name {
                Some(sym.id())
            } else {
                None
            }
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Subtable,
    FlowDest,
    FlowRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Symbol<'a> {
    UserOp {
        id: usize,
        scope: usize,
        name: String,
        index: usize,
    },
    Epsilon {
        id: usize,
        scope: usize,
        name: String,
    },
    Value {
        id: usize,
        scope: usize,
        name: String,
        pattern_value: PatternExpression,
    },
    ValueMap {
        id: usize,
        scope: usize,
        name: String,
        pattern_value: PatternExpression,
        value_table: Vec<i64>,
        table_is_filled: bool,
    },
    Name {
        id: usize,
        scope: usize,
        name: String,
        pattern_value: PatternExpression,
        name_table: Vec<String>,
        table_is_filled: bool,
    },
    Varnode {
        id: usize,
        scope: usize,
        name: String,
        space: &'a AddressSpace,
        offset: u64,
        size: usize,
    },
    Context {
        id: usize,
        scope: usize,
        name: String,
        pattern_value: PatternExpression,
        varnode_id: usize,
        high: usize,
        low: usize,
        flow: bool,
    },
    VarnodeList {
        id: usize,
        scope: usize,
        name: String,
        pattern_value: PatternExpression,
        varnode_table: Vec<Option<usize>>,
        table_is_filled: bool,
    },
    Operand {
        id: usize,
        scope: usize,
        name: String,
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
        name: String,
        pattern_value: PatternExpression,
    },
    End {
        id: usize,
        scope: usize,
        name: String,
        pattern_value: PatternExpression,
    },
    Subtable {
        id: usize,
        scope: usize,
        name: String,
        constructors: Vec<Constructor>,
        decision_tree: DecisionNode,
    },
    FlowDest {
        id: usize,
        scope: usize,
        name: String,
    },
    FlowRef {
        id: usize,
        scope: usize,
        name: String,
    },
}

impl<'a> Symbol<'a> {
    pub fn id(&self) -> usize {
        match self {
            Self::UserOp { id, .. } |
            Self::Epsilon { id, .. } |
            Self::Value { id, .. } |
            Self::ValueMap { id, .. } |
            Self::Name { id, .. } |
            Self::Varnode { id, .. } |
            Self::Context { id, .. } |
            Self::VarnodeList { id, .. } |
            Self::Operand { id, .. } |
            Self::Start { id, .. } |
            Self::End { id, .. } |
            Self::Subtable { id, .. } |
            Self::FlowDest { id, .. } |
            Self::FlowRef { id, .. } => *id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::UserOp { ref name, .. } |
            Self::Epsilon { ref name, .. } |
            Self::Value { ref name, .. } |
            Self::ValueMap { ref name, .. } |
            Self::Name { ref name, .. } |
            Self::Varnode { ref name, .. } |
            Self::Context { ref name, .. } |
            Self::VarnodeList { ref name, .. } |
            Self::Operand { ref name, .. } |
            Self::Start { ref name, .. } |
            Self::End { ref name, .. } |
            Self::Subtable { ref name, .. } |
            Self::FlowDest { ref name, .. } |
            Self::FlowRef { ref name, .. } => name,
        }
    }

    pub fn minimum_length(&self) -> Result<usize, di::Error> {
        if let Self::Operand { min_length, .. } = self {
            Ok(*min_length)
        } else {
            di::InvalidSymbol.fail()
        }
    }

    pub fn offset_base(&self) -> Result<Option<usize>, di::Error> {
        if let Self::Operand { base, .. } = self {
            Ok(*base)
        } else {
            di::InvalidSymbol.fail()
        }
    }

    pub fn relative_offset(&self) -> Result<usize, di::Error> {
        if let Self::Operand { offset, .. } = self {
            Ok(*offset)
        } else {
            di::InvalidSymbol.fail()
        }
    }

    pub fn defining_expression(&self) -> Result<Option<&PatternExpression>, di::Error> {
        if let Self::Operand { def_expr, .. } = self {
            Ok(def_expr.as_ref())
        } else {
            di::InvalidSymbol.fail()
        }
    }

    pub fn defining_symbol(&self, symbols: &'a SymbolTable) -> Result<Option<&'a Symbol>, di::Error> {
        if let Self::Operand { subsym_id, .. } = self {
            if let Some(id) = subsym_id {
                Ok(Some(symbols.symbol(*id).with_context(|| di::InvalidSymbol)?))
            } else {
                Ok(None)
            }
        } else {
            di::InvalidSymbol.fail()
        }
    }

    pub fn resolve<'b>(&'a self, walker: &mut ParserWalker<'a, 'b>) -> Result<Option<&'a Constructor>, di::Error> {
        match self {
            Self::Subtable { decision_tree, constructors, .. } => {
                Ok(Some(decision_tree.resolve(walker, constructors)?))
            },
            Self::ValueMap { table_is_filled, .. } |
            Self::VarnodeList { table_is_filled, .. } |
            Self::Name { table_is_filled, .. } => if *table_is_filled {
                Ok(None)
            } else {
                di::InvalidSymbol.fail()
            },
            _ => Ok(None)
            // FIXME: p => unreachable!("{:?}", p) // di::InvalidSymbol.fail(),
        }
    }

    pub fn is_subtable(&self) -> bool {
        if let Self::Subtable { .. } = self {
            true
        } else {
            false
        }
    }

    pub fn is_operand(&self) -> bool {
        if let Self::Operand { .. } = self {
            true
        } else {
            false
        }
    }

    pub fn fixed_handle<'b>(&'a self, walker: &'a mut ParserWalker<'a, 'b>, manager: &'a SpaceManager, symbols: &'a SymbolTable) -> Result<FixedHandle<'a>, di::Error> {
        Ok(match self {
            Self::Epsilon { .. } => {
                FixedHandle {
                    space: manager.constant_space().with_context(|| di::InvalidSpace)?,
                    size: 0,
                    offset_space: None,
                    offset_offset: 0,
                    offset_size: 0,
                    temporary_space: None,
                    temporary_offset: 0,
                }
            },
            Self::Value { pattern_value, .. } => {
                FixedHandle {
                    space: manager.constant_space().with_context(|| di::InvalidSpace)?,
                    size: 0,
                    offset_space: None,
                    offset_offset: pattern_value.value(walker, symbols)? as u64,
                    offset_size: 0,
                    temporary_space: None,
                    temporary_offset: 0,
                }
            },
            Self::Varnode { space, offset, size, .. } => {
                FixedHandle {
                    space,
                    size: *size,
                    offset_space: None,
                    offset_offset: *offset,
                    offset_size: 0,
                    temporary_space: None,
                    temporary_offset: 0,
                }
            },
            Self::Operand { handle_index, .. } => {
                walker.handle(*handle_index)?
                    .with_context(|| di::InconsistentState)?
            },
            Self::Start { .. } => {
                let space = walker.address().space();
                let size = space.address_size();
                FixedHandle {
                    space,
                    size,
                    offset_space: None,
                    offset_offset: walker.address().offset(),
                    offset_size: 0,
                    temporary_space: None,
                    temporary_offset: 0,
                }
            },
            Self::End { .. } => {
                let space = walker.address().space();
                let size = space.address_size();
                FixedHandle {
                    space,
                    size,
                    offset_space: None,
                    offset_offset: walker.next_address().with_context(|| di::InvalidNextAddress)?.offset(),
                    offset_size: 0,
                    temporary_space: None,
                    temporary_offset: 0,
                }
            }
            Self::VarnodeList { pattern_value, varnode_table, .. } => {
                let index = pattern_value.value(walker, symbols)?;
                let varnode = symbols.symbol(
                    varnode_table[index as usize].with_context(|| di::InvalidSymbol)?
                ).with_context(|| di::InvalidSymbol)?;
                varnode.fixed_handle(walker, manager, symbols)?
            },
            Self::ValueMap { pattern_value, value_table, .. } => {
                FixedHandle {
                    space: manager.constant_space().with_context(|| di::InvalidSpace)?,
                    size: 0,
                    offset_space: None,
                    offset_offset: value_table[pattern_value.value(walker, symbols)? as usize] as u64,
                    offset_size: 0,
                    temporary_space: None,
                    temporary_offset: 0,
                }
            },
            _ => return di::InvalidHandle.fail()
        })
    }

    pub fn pattern_value(&self) -> Result<&PatternExpression, di::Error> {
        Ok(match self {
            Self::Value { pattern_value, .. } |
            Self::ValueMap { pattern_value, .. } |
            Self::Name { pattern_value, .. } |
            Self::Context { pattern_value, .. } |
            Self::VarnodeList { pattern_value, .. } |
            Self::Start { pattern_value, .. } |
            Self::End { pattern_value, .. } => pattern_value,
            Self::Operand { local_expr, .. } => local_expr,
            _ => return di::InvalidPattern.fail(),
        })
    }

    pub fn format<'b>(&self, fmt: &mut fmt::Formatter, walker: &mut ParserWalker<'a, 'b>, symbols: &'a SymbolTable) -> Result<(), fmt::Error> {
        match self {
            Self::Operand { subsym_id, handle_index, def_expr, .. } => {
                walker.push_operand(*handle_index).expect("push operand");
                if let Some(id) = subsym_id {
                    let sym = symbols.symbol(*id).expect("symbol should exist for sub-table");
                    if sym.is_subtable() {
                        walker.constructor()
                            .expect("state is consistent")
                            .expect("constructor should exist for ParserWalker in formatting context")
                            .format(fmt, walker, symbols)?;
                    } else {
                        sym.format(fmt, walker, symbols)?;
                    }
                } else {
                    let value = def_expr.as_ref()
                        .expect("expression should exist for operand in formatting context")
                        .value(walker, symbols).expect("value");
                    if value < 0 {
                        write!(fmt, "-{:#x}", -value)?;
                    } else {
                        write!(fmt, "{:#x}", value)?;
                    }
                }
                walker.pop_operand().expect("pop operand");
                Ok(())
            },
            Self::Varnode { name, .. } => {
                write!(fmt, "{}", name)
            },
            Self::VarnodeList { pattern_value, varnode_table, .. } => {
                let index = pattern_value.value(walker, symbols).expect("value");
                if index >= 0 && (index as usize) < varnode_table.len() {
                    write!(fmt, "{}",
                           symbols.symbol(
                               varnode_table[index as usize]
                                   .expect("varnode should exist at index")
                           )
                           .expect("symbol for varnode should exist")
                           .name())?;
                }
                Ok(())
            },
            Self::Name { pattern_value, name_table, .. } => {
                let index = pattern_value.value(walker, symbols).expect("value") as usize;
                write!(fmt, "{}", name_table[index])
            },
            Self::Epsilon { .. } => {
                write!(fmt, "0")
            }
            Self::Value { pattern_value, .. } => {
                let value = pattern_value.value(walker, symbols).expect("value");
                if value < 0 {
                    write!(fmt, "-{:#x}", -value)
                } else {
                    write!(fmt, "{:#x}", value)
                }
            },
            Self::ValueMap { pattern_value, value_table, .. } => {
                let index = pattern_value.value(walker, symbols).expect("value") as usize;
                let value = value_table[index];
                if value < 0 {
                    write!(fmt, "-{:#x}", -value)
                } else {
                    write!(fmt, "{:#x}", value)
                }
            },
            Self::Start { .. } => {
                write!(fmt, "{:#x}", walker.address().offset())
            }
            Self::End { .. } => {
                write!(fmt, "{:#x}", walker.next_address().expect("next address should exist").offset())
            }
            s => panic!("`{:?}` should never be used in a formatting context", s),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolBuilder {
    kind: SymbolKind,
    id: usize,
    scope: usize,
    name: String,
}

impl Default for SymbolBuilder {
    fn default() -> Self {
        Self {
            kind: SymbolKind::UserOp,
            id: 0,
            scope: 0,
            name: String::default(),
        }
    }
}

impl SymbolBuilder {
    pub fn build_from_xml<'a>(self, spaces: &'a SpaceManager, input: xml::Node) -> Result<Symbol<'a>, de::Error> {
        Ok(match self.kind {
            SymbolKind::UserOp => {
                if input.tag_name().name() != "userop" {
                    return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
                }
                Symbol::UserOp {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    index: input.attribute_int("index")?,
                }
            },
            SymbolKind::Epsilon => {
                if input.tag_name().name() != "epsilon" {
                    return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
                }
                Symbol::Epsilon {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                }
            },
            SymbolKind::Value => {
                if input.tag_name().name() != "value_sym" {
                    return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
                }
                let pattern_value = input.children()
                    .filter(xml::Node::is_element)
                    .next()
                    .map(PatternExpression::from_xml)
                    .with_context(|| de::Invariant { reason: "missing pattern expression for value" })??;

                Symbol::Value {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    pattern_value,
                }
            },
            SymbolKind::ValueMap => {
                if input.tag_name().name() != "valuemap_sym" {
                    return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
                }
                let mut children = input.children().filter(xml::Node::is_element);
                let pattern_value = children.next()
                    .map(PatternExpression::from_xml)
                    .with_context(|| de::Invariant { reason: "missing pattern expression for name" })??;

                let value_table = children
                    .map(|v| v.attribute_int("val"))
                    .collect::<Result<Vec<i64>, _>>()?;

                let min = pattern_value.min_value().with_context(|| de::Invariant { reason: "invalid pattern" })?;
                let max = pattern_value.max_value().with_context(|| de::Invariant { reason: "invalid pattern" })?;

                let table_is_filled =
                    min >= 0 &&
                    (max as i64) < value_table.len() as i64 &&
                    !value_table.iter().any(|v| *v == 0xbadbeef);

                Symbol::ValueMap {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    pattern_value,
                    value_table,
                    table_is_filled,
                }
            },
            SymbolKind::Name => {
                if input.tag_name().name() != "name_sym" {
                    return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
                }
                let mut children = input.children().filter(xml::Node::is_element);
                let pattern_value = children.next()
                    .map(PatternExpression::from_xml)
                    .with_context(|| de::Invariant { reason: "missing pattern expression for value" })??;

                let name_table = children
                    .map(|v| {
                        let mut s = v.attribute_string_opt("name", "\t");
                        if s == "_" { s = "\t".to_string() };
                        s
                    })
                    .collect::<Vec<String>>();

                let min = pattern_value.min_value().with_context(|| de::Invariant { reason: "invalid pattern" })?;
                let max = pattern_value.max_value().with_context(|| de::Invariant { reason: "invalid pattern" })?;

                let table_is_filled =
                    min >= 0 &&
                    (max as i64) < name_table.len() as i64 &&
                    !name_table.iter().any(|v| v == "\t");

                Symbol::Name {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    pattern_value,
                    name_table,
                    table_is_filled,
                }

            },
            SymbolKind::Varnode => {
                if input.tag_name().name() != "varnode_sym" {
                    return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
                }

                let space_name = input.attribute("space")
                    .with_context(|| de::AttributeExpected { name: "space" })?;

                let space = spaces.space_by_name(space_name)
                    .with_context(|| de::Invariant { reason: "varnode space not defined" })?;

                let offset = input.attribute_int("offset")?;
                let size = input.attribute_int("size")?;

                Symbol::Varnode {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    space,
                    offset,
                    size,
                }
            },
            SymbolKind::Context => {
                if input.tag_name().name() != "context_sym" {
                    return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
                }

                let pattern_value = input.children()
                    .filter(xml::Node::is_element)
                    .next()
                    .map(PatternExpression::from_xml)
                    .with_context(|| de::Invariant { reason: "missing pattern expression for context" })??;

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
                    return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
                }
                let mut children = input.children().filter(xml::Node::is_element);
                let pattern_value = children.next()
                    .map(PatternExpression::from_xml)
                    .with_context(|| de::Invariant { reason: "missing pattern expression for varnodelist" })??;

                let varnode_table = children
                    .map(|input| {
                        Ok(if input.tag_name().name() == "var" {
                            Some(input.attribute_int("id")?)
                        } else {
                            None
                        })
                    })
                    .collect::<Result<Vec<Option<usize>>, _>>()?;

                let min = pattern_value.min_value().with_context(|| de::Invariant { reason: "invalid pattern" })?;
                let max = pattern_value.max_value().with_context(|| de::Invariant { reason: "invalid pattern" })?;

                let table_is_filled =
                    min >= 0 &&
                    (max as i64) < varnode_table.len() as i64 &&
                    !varnode_table.iter().any(Option::is_none);

                Symbol::VarnodeList {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    pattern_value,
                    varnode_table,
                    table_is_filled,
                }
            },
            SymbolKind::Operand => {
                if input.tag_name().name() != "operand_sym" {
                    return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
                }

                let handle_index = input.attribute_int("index")?;
                let offset = input.attribute_int("off")?;
                let base = input.attribute_int::<i64>("base")
                    .map(|v| if v < 0 { None } else { Some(v as usize) })?;

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
                let local_expr = children.next()
                    .map(PatternExpression::from_xml)
                    .with_context(|| de::Invariant { reason: "missing local expression for operand" })??;

                let def_expr = children.next()
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
            },
            SymbolKind::Start => {
                if input.tag_name().name() != "start_sym" {
                    return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
                }

                Symbol::Start {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    pattern_value: PatternExpression::StartInstruction,
                }
            },
            SymbolKind::End => {
                if input.tag_name().name() != "end_sym" {
                    return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
                }

                Symbol::End {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    pattern_value: PatternExpression::EndInstruction,
                }
            },
            SymbolKind::FlowDest => {
                if input.tag_name().name() != "flowdest_sym" {
                    return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
                }

                Symbol::FlowDest {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                }
            },
            SymbolKind::FlowRef => {
                if input.tag_name().name() != "flowref_sym" {
                    return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
                }

                Symbol::FlowDest {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                }
            },
            SymbolKind::Subtable => {
                if input.tag_name().name() != "subtable_sym" {
                    return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
                }

                // let ctor_count = input.attribute_int::<usize>("numct")?;

                let mut constructors = Vec::new();
                let mut decision_root = None;
                for input in input.children().filter(xml::Node::is_element) {
                    match input.tag_name().name() {
                        "constructor" => constructors.push(Constructor::from_xml(input)?),
                        "decision" => if decision_root.is_none() {
                            decision_root = Some(DecisionNode::from_xml(input)?);
                        } else {
                            return de::Invariant { reason: "redefintion of root decision tree node" }.fail()
                        },
                        _ => (),
                    }
                }

                Symbol::Subtable {
                    id: self.id,
                    scope: self.scope,
                    name: self.name,
                    constructors,
                    decision_tree: decision_root.with_context(||
                        de::Invariant { reason: "missing decision tree for subtable" }
                    )?,
                }
            }
        })
    }
}

#[derive(Clone)]
pub struct SymbolTable<'a> {
    scopes: Vec<SymbolScope>,
    symbols: Vec<Symbol<'a>>,
}

impl<'a> SymbolTable<'a> {
    pub fn global_scope(&self) -> Option<&SymbolScope> {
        self.scopes.get(0)
    }

    pub fn symbol(&self, id: usize) -> Option<&Symbol> {
        self.symbols.get(id)
    }

    pub (crate) fn resolve(&'a self, id: usize, walker: &mut ParserWalker) -> Result<&'a Constructor, di::Error> {
        if let Symbol::Subtable { decision_tree, constructors, .. } = &self.symbols[id] {
            decision_tree.resolve(walker, constructors)
        } else {
            di::InvalidSymbol.fail()
        }
    }

    pub fn from_xml(spaces: &'a SpaceManager, input: xml::Node) -> Result<Self, de::Error> {
        if input.tag_name().name() != "symbol_table" {
            return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
        }

        let scope_size = input.attribute_int("scopesize")?;
        let symbol_size = input.attribute_int("symbolsize")?;

        let mut children = input.children().filter(xml::Node::is_element);

        let mut scopes = vec![SymbolScope::default(); scope_size];
        for _ in 0..scope_size {
            let input = children.next().with_context(|| de::Invariant { reason: "incorrect number of scopes" })?;

            let id = input.attribute_int::<usize>("id")?;
            let parent = input.attribute_int::<usize>("parent")?;

            scopes[id].id = id;
            scopes[id].parent = if id == parent { 0 } else { parent };
        }

        let mut builders = vec![Some(SymbolBuilder::default()); symbol_size];
        for _ in 0..symbol_size {
            let input = children.next().with_context(|| de::Invariant { reason: "incorrect number of scopes" })?;

            let kind = match input.tag_name().name() {
                "userop_head" => SymbolKind::UserOp,
                "epsilion_sym_head" => SymbolKind::Epsilon,
                "value_sym_head" => SymbolKind::Value,
                "valuemap_sym_head" => SymbolKind::ValueMap,
                "name_sym_head" => SymbolKind::Name,
                "varnode_sym_head" => SymbolKind::Varnode,
                "context_sym_head" => SymbolKind::Context,
                "varlist_sym_head" => SymbolKind::VarnodeList,
                "operand_sym_head" => SymbolKind::Operand,
                "start_sym_head" => SymbolKind::Start,
                "end_sym_head" => SymbolKind::End,
                "subtable_sym_head" => SymbolKind::Subtable,
                "flowdest_sym_head" => SymbolKind::FlowDest,
                "flowref_sym_head" => SymbolKind::FlowRef,
                name => return de::TagUnexpected { name: name.to_owned() }.fail()
            };
            let id = input.attribute_int::<usize>("id")?;
            let scope = input.attribute_int("scope")?;
            let name = input.attribute("name")
                .with_context(|| de::AttributeExpected { name: "name" })?;

            let builder = builders[id].as_mut()
                .with_context(|| de::Invariant { reason: "inconsistent symbol ID" })?;
            builder.kind = kind;
            builder.id = id;
            builder.scope = scope;
            builder.name.push_str(name);

            scopes[scope].add_symbol(id);
        }

        let mut symbols = Vec::with_capacity(symbol_size);
        for child in children {
            let id = child.attribute_int::<usize>("id")?;
            let builder = take(&mut builders[id]);
            symbols.push(builder.unwrap().build_from_xml(spaces, child)?);
        }

        symbols.sort_by_key(Symbol::id);

        Ok(Self {
            scopes,
            symbols,
        })
    }
}
