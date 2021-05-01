use std::fmt::Debug;

use crate::address::Address;
use crate::space::AddressSpace;

use crate::error::deserialisation as de;
use snafu::OptionExt;

#[derive(Debug, Clone)]
pub struct SpaceManager {
    spaces: Vec<AddressSpace>,
    constant_space: usize,
    default_space: usize,
    unique_space: usize,
}

impl SpaceManager {
    pub fn address_from<S: AsRef<str>>(&self, space: S, offset: u64) -> Option<Address> {
        let space = self.space_by_name(space)?;
        Some(Address::new(space, offset))
    }

    pub fn address_size(&self) -> usize {
        self.spaces[self.default_space].address_size()
    }

    pub fn spaces(&self) -> &[AddressSpace] {
        self.spaces.as_ref()
    }

    pub fn space_by_name<S: AsRef<str>>(&self, name: S) -> Option<&AddressSpace> {
        let name = name.as_ref();
        self.spaces.iter().find(|space| space.name() == name)
    }

    pub fn constant_space(&self) -> Option<&AddressSpace> {
        self.spaces.get(self.constant_space)
    }

    pub fn default_space(&self) -> Option<&AddressSpace> {
        self.spaces.get(self.default_space)
    }

    pub fn unique_space(&self) -> Option<&AddressSpace> {
        self.spaces.get(self.unique_space)
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, de::Error> {
        if input.tag_name().name() != "spaces" {
            return de::TagUnexpected { name: input.tag_name().name().to_owned() }.fail()
        }

        let mut spaces = vec![AddressSpace::constant("const", 0)];
        let mut default_space = 0;
        let mut unique_space = 0;

        let default_name = input.attribute("defaultspace")
            .with_context(|| de::AttributeExpected { name: "defaultspace" })?;

        for (index, child) in input.children()
            .filter(xml::Node::is_element)
            .enumerate()
            .map(|(i, c)| (i + 1, c))
        {
            let space = AddressSpace::from_xml(child)?;

            if space.index() != index {
                return de::Invariant { reason: "space index mismatch" }.fail()
            }

            if space.name() == default_name {
                default_space = index;
            }

            if space.name() == "unique" {
                unique_space = index;
            }

            spaces.push(space);
        }

        if unique_space == 0 {
            return de::Invariant { reason: "unique space not defined" }.fail()
        }

        Ok(Self {
            spaces,
            constant_space: 0,
            default_space,
            unique_space,
        })
    }
}
