use std::mem;
use std::ops::{Deref, DerefMut};

use bitflags::bitflags;

use crate::deserialise::{DeserialiseError, XmlExt};
use crate::util::calculate_mask;

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub enum AddressSpaceKind {
    Constant,
    Default,
    Processor,
    Internal,
    Register,
}

bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize)]
    pub struct AddressSpaceProperty: u32 {
        const BigEndian            = 1;
        const Heritaged            = 2;
        const DoesDeadcode         = 4;
        const ProgramSpecific      = 8;
        const ReverseJustification = 16;
        const Overlay              = 32;
        const OverlayBase          = 64;
        const Truncated            = 128;
        const HasPhysical          = 256;
    }
}

impl AddressSpaceProperty {
    pub fn is_set(&self) -> bool {
        !self.is_empty()
    }

    pub fn is_big_endian(&self) -> bool {
        self.contains(Self::BigEndian)
    }

    pub fn is_heritaged(&self) -> bool {
        self.contains(Self::Heritaged)
    }

    pub fn does_deadcode(&self) -> bool {
        self.contains(Self::DoesDeadcode)
    }

    pub fn is_program_specific(&self) -> bool {
        self.contains(Self::ProgramSpecific)
    }

    pub fn is_reverse_justified(&self) -> bool {
        self.contains(Self::ReverseJustification)
    }

    pub fn is_overlay(&self) -> bool {
        self.contains(Self::Overlay)
    }

    pub fn is_overlay_base(&self) -> bool {
        self.contains(Self::OverlayBase)
    }

    pub fn is_truncated(&self) -> bool {
        self.contains(Self::Truncated)
    }

    pub fn has_physical(&self) -> bool {
        self.contains(Self::HasPhysical)
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub struct AddressSpaceDef {
    index: usize,
    pub(crate) kind: AddressSpaceKind,
    properties: AddressSpaceProperty,
    highest: u64,
    name: String,
    address_size: usize,
    word_size: usize,
    delay: usize,
    deadcode_delay: usize,
}

impl AddressSpaceDef {
    pub fn new<S: AsRef<str>>(
        kind: AddressSpaceKind,
        name: S,
        address_size: usize,
        word_size: usize,
        index: usize,
        properties: Option<AddressSpaceProperty>,
        delay: usize,
    ) -> Self {
        let properties = properties
            .map(|v| v & AddressSpaceProperty::HasPhysical)
            .unwrap_or(AddressSpaceProperty::default());

        let highest = calculate_mask(address_size) * (word_size as u64) + (word_size as u64 - 1);

        Self {
            kind: if name.as_ref() == "register" {
                AddressSpaceKind::Register
            } else {
                kind
            },
            properties: properties
                | AddressSpaceProperty::Heritaged
                | AddressSpaceProperty::DoesDeadcode,
            highest,
            name: name.as_ref().to_owned(),
            address_size,
            word_size,
            index,
            delay,
            deadcode_delay: delay,
        }
    }
}

#[derive(Debug, Clone, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize)]
pub enum AddressSpace {
    Constant(AddressSpaceDef),
    Unique(AddressSpaceDef),
    Space(AddressSpaceDef),
}

impl PartialEq for AddressSpace {
    fn eq(&self, other: &Self) -> bool {
        self.index() == other.index()
    }
}
impl Eq for AddressSpace {}

impl Deref for AddressSpace {
    type Target = AddressSpaceDef;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Constant(space) | Self::Space(space) | Self::Unique(space) => space,
        }
    }
}

impl DerefMut for AddressSpace {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Constant(space) | Self::Space(space) | Self::Unique(space) => space,
        }
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
#[repr(transparent)]
pub struct AddressSpaceId(pub(crate) u32);

const ID_CONSTANT_SPACE: u32 = 0x8000_0000;
const ID_DEFAULT_SPACE: u32 = 0x4000_0000;
const ID_REGISTER_SPACE: u32 = 0x2000_0000;
const ID_UNIQUE_SPACE: u32 = 0x1000_0000;

const ID_STACK_HINT: u32 = 0x0100_0000;
const ID_HEAP_HINT: u32 = 0x0200_0000;
const ID_STACK_OR_HEAP: u32 = ID_STACK_HINT | ID_HEAP_HINT;
const ID_UNMAPPED_HINT: u32 = 0x0400_0000;

impl AddressSpaceId {
    pub fn index(&self) -> usize {
        (self.0 & 0xffff) as usize
    }

    pub fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub fn bits(&self) -> u32 {
        self.0
    }

    pub fn mark_heap(&mut self) {
        *self = (*self).heap();
    }

    pub fn heap(self) -> Self {
        if self.is_default() {
            Self(self.0 & !ID_STACK_OR_HEAP | ID_HEAP_HINT)
        } else {
            self
        }
    }

    pub fn mark_stack(&mut self) {
        *self = (*self).heap();
    }

    pub fn stack(self) -> Self {
        if self.is_default() {
            Self(self.0 & !ID_STACK_OR_HEAP | ID_STACK_HINT)
        } else {
            self
        }
    }

    pub const fn constant_id(index: usize) -> Self {
        Self((index & 0xffff) as u32 | ID_CONSTANT_SPACE)
    }

    pub const fn default_id(index: usize) -> Self {
        Self((index & 0xffff) as u32 | ID_DEFAULT_SPACE)
    }

    pub const fn register_id(index: usize) -> Self {
        Self((index & 0xffff) as u32 | ID_REGISTER_SPACE)
    }

    pub const fn unique_id(index: usize) -> Self {
        Self((index & 0xffff) as u32 | ID_UNIQUE_SPACE)
    }

    pub const fn other_id(index: usize) -> Self {
        Self((index & 0xffff) as u32)
    }

    pub const fn unmapped_id(index: usize) -> Self {
        Self((index & 0xffff) as u32 | ID_UNMAPPED_HINT)
    }

    pub fn is_constant(&self) -> bool {
        (ID_CONSTANT_SPACE & self.0) != 0
    }

    pub fn is_default(&self) -> bool {
        (ID_DEFAULT_SPACE & self.0) != 0
    }

    pub fn is_global(&self) -> bool {
        let mask = ID_DEFAULT_SPACE | ID_STACK_OR_HEAP;
        self.0 & mask == ID_DEFAULT_SPACE
    }

    pub fn is_stack(&self) -> bool {
        let mask = ID_DEFAULT_SPACE | ID_STACK_HINT;
        self.0 & mask == mask
    }

    pub fn is_heap(&self) -> bool {
        let mask = ID_DEFAULT_SPACE | ID_HEAP_HINT;
        self.0 & mask == mask
    }

    pub fn is_register(&self) -> bool {
        (ID_REGISTER_SPACE & self.0) != 0
    }

    pub fn is_unique(&self) -> bool {
        (ID_UNIQUE_SPACE & self.0) != 0
    }

    pub fn is_unmapped(&self) -> bool {
        (ID_UNMAPPED_HINT & self.0) != 0
    }
}

impl AddressSpace {
    pub fn is_constant(&self) -> bool {
        if let Self::Constant(..) = self {
            true
        } else {
            false
        }
    }

    pub fn is_unique(&self) -> bool {
        if let Self::Unique(..) = self {
            true
        } else {
            false
        }
    }

    pub fn is_register(&self) -> bool {
        matches!(self.kind(), AddressSpaceKind::Register)
    }

    pub fn is_default(&self) -> bool {
        matches!(self.kind(), AddressSpaceKind::Default)
    }

    pub fn constant<S: AsRef<str>>(name: S, index: usize) -> Self {
        let mut space = Self::Constant(AddressSpaceDef::new(
            AddressSpaceKind::Constant,
            name,
            mem::size_of::<u64>(),
            1,
            index,
            None,
            0,
        ));

        space.properties &= !(AddressSpaceProperty::Heritaged
            | AddressSpaceProperty::DoesDeadcode
            | AddressSpaceProperty::BigEndian);
        if cfg!(target_endian = "big") {
            space.properties |= AddressSpaceProperty::BigEndian;
        }
        space
    }

    pub fn unique<S: AsRef<str>>(
        name: S,
        index: usize,
        properties: Option<AddressSpaceProperty>,
    ) -> Self {
        Self::Unique(AddressSpaceDef::new(
            AddressSpaceKind::Internal,
            name,
            mem::size_of::<usize>(),
            1,
            index,
            properties
                .map(|p| p | AddressSpaceProperty::HasPhysical)
                .or(Some(AddressSpaceProperty::HasPhysical)),
            0,
        ))
    }

    pub fn kind(&self) -> AddressSpaceKind {
        self.kind
    }

    pub fn properties(&self) -> AddressSpaceProperty {
        self.properties
    }

    pub fn delay(&self) -> usize {
        self.delay
    }

    pub fn deadcode_delay(&self) -> usize {
        self.deadcode_delay
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn id(&self) -> AddressSpaceId {
        match self {
            Self::Constant(s) => AddressSpaceId::constant_id(s.index),
            Self::Unique(s) => AddressSpaceId::unique_id(s.index),
            Self::Space(s) if s.kind == AddressSpaceKind::Register => {
                AddressSpaceId::register_id(s.index)
            }
            Self::Space(s) if s.kind == AddressSpaceKind::Default => {
                AddressSpaceId::default_id(s.index)
            }
            Self::Space(s) => AddressSpaceId::other_id(s.index),
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn word_size(&self) -> usize {
        self.word_size
    }

    pub fn address_size(&self) -> usize {
        self.address_size
    }

    pub fn highest_offset(&self) -> u64 {
        self.highest
    }

    pub fn wrap_offset(&self, offset: u64) -> u64 {
        if offset <= self.highest {
            offset
        } else {
            let m = (self.highest + 1) as i64;
            let r = (offset as i64) % m;
            (if r < 0 { r + m } else { r }) as u64
        }
    }

    pub fn truncate_space(&mut self, size: usize) {
        self.properties |= AddressSpaceProperty::Truncated;
        self.address_size = size;
        self.highest = calculate_mask(self.address_size) * (self.word_size as u64)
            + (self.word_size as u64 - 1);
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        let name = input.attribute_string("name")?;
        let index = input.attribute_int("index")?;
        let address_size = input.attribute_int("size")?;
        let delay = input.attribute_int("delay")?;
        let word_size = input.attribute_int_opt("wordsize", 1)?;
        let deadcode_delay = input.attribute_int_opt("deadcodedelay", delay)?;

        let mut properties = AddressSpaceProperty::Heritaged | AddressSpaceProperty::DoesDeadcode;
        if input.attribute_bool("bigendian")? {
            properties |= AddressSpaceProperty::BigEndian;
        }

        if input.attribute_bool("physical")? {
            properties |= AddressSpaceProperty::HasPhysical;
        }

        let highest = calculate_mask(address_size) * (word_size as u64) + (word_size as u64 - 1);

        match input.tag_name().name() {
            /* These are not used in any .sla distributed with Ghidra:
            "space_base"
            "space_overlay"
            */
            "space_unique" => Ok(Self::Unique(AddressSpaceDef {
                kind: AddressSpaceKind::Internal,
                properties,
                name,
                highest,
                address_size,
                word_size,
                index,
                delay,
                deadcode_delay,
            })),
            "space" | "space_other" => Ok(Self::Space(AddressSpaceDef {
                kind: AddressSpaceKind::Processor,
                properties,
                name,
                highest,
                address_size,
                word_size,
                index,
                delay,
                deadcode_delay,
            })),
            tag => Err(DeserialiseError::TagUnexpected(tag.to_owned())),
        }
    }
}

impl From<AddressSpace> for AddressSpaceId {
    fn from(space: AddressSpace) -> Self {
        space.id()
    }
}

impl From<&'_ AddressSpace> for AddressSpaceId {
    fn from(space: &AddressSpace) -> Self {
        space.id()
    }
}
