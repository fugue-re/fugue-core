use std::fmt::Debug;
use std::sync::Arc;

use crate::address::AddressValue;
use crate::deserialise::Error;
use crate::space::AddressSpace;

#[derive(Debug, Clone)]
pub struct SpaceManager {
    spaces: Vec<Arc<AddressSpace>>,
    constant_space: usize,
    default_space: usize,
    register_space: usize,
    unique_space: usize,
}

impl SpaceManager {
    pub fn address_from<S: AsRef<str>>(&self, space: S, offset: u64) -> Option<AddressValue> {
        let space = self.space_by_name(space)?;
        Some(AddressValue::new(space, offset))
    }

    pub fn address_size(&self) -> usize {
        self.spaces[self.default_space].address_size()
    }

    pub fn spaces(&self) -> &[Arc<AddressSpace>] {
        self.spaces.as_ref()
    }

    pub fn space_by_name<S: AsRef<str>>(&self, name: S) -> Option<Arc<AddressSpace>> {
        let name = name.as_ref();
        self.spaces.iter().find_map(|space| if space.name() == name { Some(space.clone()) } else { None })
    }

    pub fn constant_space(&self) -> Arc<AddressSpace> {
        self.spaces[self.constant_space].clone()
    }

    pub fn default_space(&self) -> Arc<AddressSpace> {
        self.spaces[self.default_space].clone()
    }

    pub fn register_space(&self) -> Arc<AddressSpace> {
        self.spaces[self.register_space].clone()
    }

    pub fn unique_space(&self) -> Arc<AddressSpace> {
        self.spaces[self.unique_space].clone()
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, Error> {
        if input.tag_name().name() != "spaces" {
            return Err(Error::TagUnexpected(input.tag_name().name().to_owned()));
        }

        let mut spaces = vec![Arc::new(AddressSpace::constant("const", 0))];
        let mut default_space = 0;
        let mut register_space = 0;
        let mut unique_space = 0;

        let default_name = input
            .attribute("defaultspace")
            .ok_or_else(|| Error::AttributeExpected("defaultspace"))?;

        for (index, child) in input
            .children()
            .filter(xml::Node::is_element)
            .enumerate()
            .map(|(i, c)| (i + 1, c))
        {
            let space = AddressSpace::from_xml(child)?;

            if space.index() != index {
                return Err(Error::Invariant("space index mismatch"));
            }

            if space.name() == default_name {
                default_space = index;
            }

            if space.name() == "register" {
                register_space = index;
            }

            if space.name() == "unique" {
                unique_space = index;
            }

            spaces.push(Arc::new(space));
        }

        if default_space == 0 {
            return Err(Error::Invariant("non-constant default space not defined"));
        }

        if register_space == 0 {
            return Err(Error::Invariant("register space not defined"));
        }

        if unique_space == 0 {
            return Err(Error::Invariant("unique space not defined"));
        }

        Ok(Self {
            spaces,
            constant_space: 0,
            default_space,
            register_space,
            unique_space,
        })
    }
}
