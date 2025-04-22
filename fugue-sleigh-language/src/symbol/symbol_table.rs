use std::mem::take;

use fugue_ghidra_marshal::sla::*;
use fugue_ghidra_marshal::Decoder;

use ustr::Ustr;

use crate::deserialise::{DeserialiseError, XmlExt};
use crate::spaces::AddressSpaces;
use crate::symbol::{Symbol, SymbolBuilder, SymbolKind, SymbolScope};

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct SymbolTable {
    scopes: Vec<SymbolScope>,
    symbols: Vec<Symbol>,
}

impl SymbolTable {
    pub fn global_scope(&self) -> Option<&SymbolScope> {
        self.scopes.get(0)
    }

    pub fn symbol(&self, id: usize) -> Option<&Symbol> {
        self.symbols.get(id)
    }

    pub fn scopes(&self) -> &[SymbolScope] {
        &self.scopes
    }

    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    pub fn from_decoder<D: Decoder>(
        spaces: &AddressSpaces,
        input: &mut D,
    ) -> Result<Self, DeserialiseError> {
        let id = input.open_element_with_id(&ELEM_SYMBOL_TABLE)?;

        let scope_size = input.read_signed_integer_with_id(&ATTRIB_SCOPESIZE)? as usize;
        let symbol_size = input.read_signed_integer_with_id(&ATTRIB_SYMBOLSIZE)? as usize;

        let mut scopes = vec![SymbolScope::default(); scope_size];

        for _ in 0..scope_size {
            #[cfg(feature = "tracing")]
            tracing::trace!("decoding symbol table scope");

            let scope = input.open_element_with_id(&ELEM_SCOPE)?;

            let id = input.read_unsigned_integer_with_id(&ATTRIB_ID)? as usize;
            let parent = input.read_unsigned_integer_with_id(&ATTRIB_PARENT)? as usize;

            scopes[id].id = id;
            scopes[id].parent = if id == parent { 0 } else { parent };

            input.close_element(scope)?;
        }

        let mut builders = vec![Some(SymbolBuilder::default()); symbol_size];

        for _ in 0..symbol_size {
            #[cfg(feature = "tracing")]
            tracing::trace!("decoding symbol table symbol header");

            let symbol = input.open_element()?;

            let kind = match symbol {
                ELEM_USEROP_HEAD_ID => SymbolKind::UserOp,
                ELEM_EPSILON_SYM_HEAD_ID => SymbolKind::Epsilon,
                ELEM_VALUE_SYM_HEAD_ID => SymbolKind::Value,
                ELEM_VALUEMAP_SYM_HEAD_ID => SymbolKind::ValueMap,
                ELEM_NAME_SYM_HEAD_ID => SymbolKind::Name,
                ELEM_VARNODE_SYM_HEAD_ID => SymbolKind::Varnode,
                ELEM_CONTEXT_SYM_HEAD_ID => SymbolKind::Context,
                ELEM_VARLIST_SYM_HEAD_ID => SymbolKind::VarnodeList,
                ELEM_OPERAND_SYM_HEAD_ID => SymbolKind::Operand,
                ELEM_START_SYM_HEAD_ID => SymbolKind::Start,
                ELEM_END_SYM_HEAD_ID => SymbolKind::End,
                ELEM_NEXT2_SYM_HEAD_ID => SymbolKind::Next2,
                ELEM_SUBTABLE_SYM_HEAD_ID => SymbolKind::Subtable,
                id => return Err(DeserialiseError::ElementUnexpected(id)),
            };

            let id = input.read_unsigned_integer_with_id(&ATTRIB_ID)? as usize;
            let scope = input.read_unsigned_integer_with_id(&ATTRIB_SCOPE)? as usize;
            let name = input.read_string_with_id(&ATTRIB_NAME)?;

            let builder = builders[id]
                .as_mut()
                .ok_or_else(|| DeserialiseError::Invariant("inconsistent symbol ID"))?;

            builder.kind = kind;
            builder.id = id;
            builder.scope = scope;
            builder.name = Ustr::from(&name);

            scopes[scope].add_symbol(id);

            input.close_element(symbol)?;
        }

        let mut symbols = Vec::with_capacity(symbol_size);
        while input.peek_element()? != 0 {
            #[cfg(feature = "tracing")]
            tracing::trace!("decoding symbol definition");

            let symbol = input.open_element()?;

            let id = input.read_unsigned_integer_with_id(&ATTRIB_ID)? as usize;
            let builder = take(&mut builders[id]);

            symbols.push(builder.unwrap().build_from_decoder(spaces, input, symbol)?);

            input.close_element(symbol)?;
        }

        symbols.sort_by_key(Symbol::id);

        input.close_element_skipping(id)?;

        Ok(Self { scopes, symbols })
    }

    pub fn from_xml(spaces: &AddressSpaces, input: xml::Node) -> Result<Self, DeserialiseError> {
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
                "next2_sym_head" => SymbolKind::Next2,
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
            builder.name = Ustr::from(name);

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
