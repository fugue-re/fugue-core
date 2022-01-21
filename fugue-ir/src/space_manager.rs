use std::fmt::Debug;
use std::sync::Arc;

use crate::address::AddressValue;
use crate::deserialise::Error;
use crate::disassembly::IRBuilderArena;
use crate::space::{AddressSpace, AddressSpaceId, Space, SpaceKind, SpaceProperty};

#[derive(Debug, Clone)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct SpaceManager {
    spaces: Vec<Arc<AddressSpace>>,
    constant_space: usize,
    default_space: usize,
    register_space: usize,
    unique_space: usize,
}

pub trait FromSpace<'z, T> {
    fn from_space(t: T, manager: &SpaceManager) -> Self;
    fn from_space_with(t: T, arena: &'z IRBuilderArena, manager: &SpaceManager) -> Self;
}

pub trait IntoSpace<'z, T> {
    fn into_space(self, manager: &SpaceManager) -> T;
    fn into_space_with(self, arena: &'z IRBuilderArena, manager: &SpaceManager) -> T;
}

impl<'z, T, U> IntoSpace<'z, T> for U where T: FromSpace<'z, U> {
    fn into_space(self, manager: &SpaceManager) -> T {
        T::from_space(self, manager)
    }

    fn into_space_with(self, arena: &'z IRBuilderArena, manager: &SpaceManager) -> T {
        T::from_space_with(self, arena, manager)
    }
}

impl SpaceManager {
    pub fn address_from<S: AsRef<str>>(&self, space: S, offset: u64) -> Option<AddressValue> {
        let space = self.space_by_name(space)?;
        Some(AddressValue::new(space, offset))
    }

    pub fn address_size(&self) -> usize {
        unsafe { self.spaces.get_unchecked(self.default_space) }.address_size()
    }

    pub fn spaces(&self) -> &[Arc<AddressSpace>] {
        self.spaces.as_ref()
    }

    pub fn space_by_name<S: AsRef<str>>(&self, name: S) -> Option<Arc<AddressSpace>> {
        let name = name.as_ref();
        self.spaces.iter().find_map(|space| if space.name() == name { Some(space.clone()) } else { None })
    }

    pub fn space_by_id(&self, id: AddressSpaceId) -> &AddressSpace {
        &self.spaces[id.index()]
    }

    pub fn unchecked_space_by_id(&self, id: AddressSpaceId) -> &AddressSpace {
        &* unsafe { self.spaces.get_unchecked(id.index()) }
    }

    pub fn constant_space(&self) -> Arc<AddressSpace> {
        unsafe { self.spaces.get_unchecked(self.constant_space) }.clone()
    }

    pub fn constant_space_ref(&self) -> &AddressSpace {
        &*unsafe { self.spaces.get_unchecked(self.constant_space) }
    }

    pub fn constant_space_id(&self) -> AddressSpaceId {
        AddressSpaceId::constant_id(self.constant_space)
    }

    pub fn default_space(&self) -> Arc<AddressSpace> {
        unsafe { self.spaces.get_unchecked(self.default_space) }.clone()
    }

    pub fn default_space_ref(&self) -> &AddressSpace {
        &*unsafe { self.spaces.get_unchecked(self.default_space) }
    }

    pub fn default_space_id(&self) -> AddressSpaceId {
        AddressSpaceId::default_id(self.default_space)
    }

    pub fn register_space(&self) -> Arc<AddressSpace> {
        unsafe { self.spaces.get_unchecked(self.register_space) }.clone()
    }

    pub fn register_space_ref(&self) -> &AddressSpace {
        &*unsafe { self.spaces.get_unchecked(self.register_space) }
    }

    pub fn register_space_id(&self) -> AddressSpaceId {
        AddressSpaceId::register_id(self.register_space)
    }

    pub fn unique_space(&self) -> Arc<AddressSpace> {
        unsafe { self.spaces.get_unchecked(self.unique_space) }.clone()
    }

    pub fn unique_space_ref(&self) -> &AddressSpace {
        &*unsafe { self.spaces.get_unchecked(self.unique_space) }
    }

    pub fn unique_space_id(&self) -> AddressSpaceId {
        AddressSpaceId::unique_id(self.unique_space)
    }

    pub fn add_space<S: AsRef<str>>(
        &mut self,
        kind: SpaceKind,
        name: S,
        address_size: usize,
        word_size: usize,
        properties: Option<SpaceProperty>,
        delay: usize,
    ) -> Arc<AddressSpace> {
        let index = self.spaces.len();
        let space = Arc::new(AddressSpace::Space(Space::new(
            kind,
            name,
            address_size,
            word_size,
            index,
            properties,
            delay,
        )));
        self.spaces.push(space.clone());
        space
    }

    pub fn add_space_like<S: AsRef<str>>(
        &mut self,
        name: S,
        space: &AddressSpace,
    ) -> Arc<AddressSpace> {
        self.add_space(
            space.kind(),
            name,
            space.address_size(),
            space.word_size(),
            Some(space.properties()),
            space.delay(),
        )
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
            let mut space = AddressSpace::from_xml(child)?;

            if space.index() != index {
                return Err(Error::Invariant("space index mismatch"));
            }

            if space.name() == default_name {
                default_space = index;
                space.kind = SpaceKind::Default;
            }

            if space.name() == "register" {
                register_space = index;
                space.kind = SpaceKind::Register;
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
