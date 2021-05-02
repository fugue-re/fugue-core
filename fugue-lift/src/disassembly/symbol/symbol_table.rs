//use crate::disassembly::ParserWalker;
use crate::deserialise::parse::XmlExt;
use crate::deserialise::Error as DeserialiseError;

use crate::disassembly::symbol::{Constructor, Symbol, SymbolBuilder, SymbolKind, SymbolScope};

//use crate::error::disassembly as di;

use crate::space_manager::SpaceManager;

use std::mem::take;

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

    /*
    pub (crate) fn resolve(&'a self, id: usize, walker: &mut ParserWalker) -> Result<&'a Constructor, di::Error> {
        if let Symbol::Subtable { decision_tree, constructors, .. } = &self.symbols[id] {
            decision_tree.resolve(walker, constructors)
        } else {
            di::InvalidSymbol.fail()
        }
    }
    */

    pub fn from_xml(spaces: &'a SpaceManager, input: xml::Node) -> Result<Self, DeserialiseError> {
        if input.tag_name().name() != "symbol_table" {
            return Err(DeserialiseError::TagUnexpected(
                input.tag_name().name().to_owned(),
            ));
        }

        let scope_size = input.attribute_int("scopesize")?;
        let symbol_size = input.attribute_int("symbolsize")?;

        let mut children = input.children().filter(xml::Node::is_element);

        let mut scopes = vec![SymbolScope::default(); scope_size];
        for _ in 0..scope_size {
            let input = children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("incorrect number of scopes"))?;

            let id = input.attribute_int::<usize>("id")?;
            let parent = input.attribute_int::<usize>("parent")?;

            scopes[id].id = id;
            scopes[id].parent = if id == parent { 0 } else { parent };
        }

        let mut builders = vec![Some(SymbolBuilder::default()); symbol_size];
        for _ in 0..symbol_size {
            let input = children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("incorrect number of scopes"))?;

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
                name => return Err(DeserialiseError::TagUnexpected(name.to_owned())),
            };
            let id = input.attribute_int::<usize>("id")?;
            let scope = input.attribute_int("scope")?;
            let name = input
                .attribute("name")
                .ok_or_else(|| DeserialiseError::AttributeExpected("name"))?;

            let builder = builders[id]
                .as_mut()
                .ok_or_else(|| DeserialiseError::Invariant("inconsistent symbol ID"))?;

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

        Ok(Self { scopes, symbols })
    }
}
