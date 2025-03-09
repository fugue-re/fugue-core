use std::fmt::Debug;
use std::ops::Deref;
use std::sync::Arc;

use crate::deserialise::DeserialiseError;

pub mod space;
pub use space::{AddressSpace, AddressSpaceId, AddressSpaceKind, AddressSpaceProperty};

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct AddressSpaces {
    spaces: Vec<Arc<AddressSpace>>,
    constant_space: usize,
    default_space: usize,
    register_space: usize,
    unique_space: usize,
}

impl Deref for AddressSpaces {
    type Target = [Arc<AddressSpace>];

    fn deref(&self) -> &Self::Target {
        &self.spaces
    }
}

impl AddressSpaces {
    pub fn address_size(&self) -> usize {
        self.spaces
            .get(self.default_space)
            .expect("valid space")
            .address_size()
    }

    pub fn space_by_name<S: AsRef<str>>(&self, name: S) -> Option<Arc<AddressSpace>> {
        let name = name.as_ref();
        self.spaces.iter().find_map(|space| {
            if space.name() == name {
                Some(space.clone())
            } else {
                None
            }
        })
    }

    pub fn space_by_id(&self, id: AddressSpaceId) -> &AddressSpace {
        &self.spaces[id.index()]
    }

    pub fn constant_space(&self) -> Arc<AddressSpace> {
        self.spaces
            .get(self.constant_space)
            .cloned()
            .expect("valid space")
    }

    pub fn constant_space_ref(&self) -> &AddressSpace {
        self.spaces.get(self.constant_space).expect("valid space")
    }

    pub fn constant_space_id(&self) -> AddressSpaceId {
        AddressSpaceId::constant_id(self.constant_space)
    }

    pub fn default_space(&self) -> Arc<AddressSpace> {
        self.spaces
            .get(self.default_space)
            .cloned()
            .expect("valid space")
    }

    pub fn default_space_ref(&self) -> &AddressSpace {
        self.spaces.get(self.default_space).expect("valid space")
    }

    pub fn default_space_id(&self) -> AddressSpaceId {
        AddressSpaceId::default_id(self.default_space)
    }

    pub fn register_space(&self) -> Arc<AddressSpace> {
        self.spaces
            .get(self.register_space)
            .cloned()
            .expect("valid space")
    }

    pub fn register_space_ref(&self) -> &AddressSpace {
        self.spaces.get(self.register_space).expect("valid space")
    }

    pub fn register_space_id(&self) -> AddressSpaceId {
        AddressSpaceId::register_id(self.register_space)
    }

    pub fn unique_space(&self) -> Arc<AddressSpace> {
        self.spaces
            .get(self.unique_space)
            .cloned()
            .expect("valid space")
    }

    pub fn unique_space_ref(&self) -> &AddressSpace {
        self.spaces.get(self.unique_space).expect("valid space")
    }

    pub fn unique_space_id(&self) -> AddressSpaceId {
        AddressSpaceId::unique_id(self.unique_space)
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        if input.tag_name().name() != "spaces" {
            return Err(DeserialiseError::TagUnexpected(
                input.tag_name().name().to_owned(),
            ));
        }

        let mut spaces = vec![Arc::new(AddressSpace::constant("const", 0))];
        let mut default_space = 0;
        let mut register_space = 0;
        let mut unique_space = 0;

        let default_name = input
            .attribute("defaultspace")
            .ok_or_else(|| DeserialiseError::AttributeExpected("defaultspace"))?;

        for (index, child) in input
            .children()
            .filter(xml::Node::is_element)
            .enumerate()
            .map(|(i, c)| (i + 1, c))
        {
            let mut space = AddressSpace::from_xml(child)?;

            if space.index() != index {
                return Err(DeserialiseError::Invariant("space index mismatch"));
            }

            if space.name() == default_name {
                default_space = index;
                space.kind = AddressSpaceKind::Default;
            }

            if space.name() == "register" {
                register_space = index;
                space.kind = AddressSpaceKind::Register;
            }

            if space.name() == "unique" {
                unique_space = index;
            }

            spaces.push(Arc::new(space));
        }

        if default_space == 0 {
            return Err(DeserialiseError::Invariant(
                "non-constant default space not defined",
            ));
        }

        if register_space == 0 {
            return Err(DeserialiseError::Invariant("register space not defined"));
        }

        if unique_space == 0 {
            return Err(DeserialiseError::Invariant("unique space not defined"));
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
