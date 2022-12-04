use crate::bits::calculate_mask;
use crate::deserialise::parse::XmlExt;
use crate::deserialise::Error;

use std::fmt;
use std::mem;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Deref, DerefMut, Not};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum SpaceKind {
    Constant,
    Default,
    Processor,
    Internal,
    Register,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
#[repr(transparent)]
pub struct SpaceProperty(usize);

impl Not for SpaceProperty {
    type Output = Self;

    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl BitAnd for SpaceProperty {
    type Output = Self;

    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

impl BitAndAssign for SpaceProperty {
    fn bitand_assign(&mut self, other: Self) {
        *self = Self(self.0 & other.0)
    }
}

impl BitOr for SpaceProperty {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl BitOrAssign for SpaceProperty {
    fn bitor_assign(&mut self, other: Self) {
        *self = Self(self.0 | other.0)
    }
}

impl fmt::Debug for SpaceProperty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        if !self.is_set() {
            write!(f, "SpaceProperty::default()")?;
            return Ok(());
        }

        let pr = [
            "BigEndian",
            "Heritaged",
            "DoesDeadcode",
            "ProgramSpecific",
            "ReverseJustification",
            "Overlay",
            "OverlayBase",
            "Truncated",
            "HasPhysical",
        ];

        write!(f, "SpaceProperty(")?;

        let mut kinds = Self::iter().zip(pr.iter()).filter_map(|(o, v)| {
            if (o & *self).is_set() {
                Some(v)
            } else {
                None
            }
        });

        write!(f, "{}", kinds.next().unwrap())?;

        for &kind in kinds {
            write!(f, " | {}", kind)?;
        }

        write!(f, ")")
    }
}

impl Default for SpaceProperty {
    fn default() -> Self {
        Self(0)
    }
}

pub mod property {
    #![allow(non_upper_case_globals)]

    use super::SpaceProperty;

    pub const BigEndian: SpaceProperty = SpaceProperty(1);
    pub const Heritaged: SpaceProperty = SpaceProperty(2);
    pub const DoesDeadcode: SpaceProperty = SpaceProperty(4);
    pub const ProgramSpecific: SpaceProperty = SpaceProperty(8);
    pub const ReverseJustification: SpaceProperty = SpaceProperty(16);
    pub const Overlay: SpaceProperty = SpaceProperty(32);
    pub const OverlayBase: SpaceProperty = SpaceProperty(64);
    pub const Truncated: SpaceProperty = SpaceProperty(128);
    pub const HasPhysical: SpaceProperty = SpaceProperty(256);
}

impl SpaceProperty {
    pub fn is_set(&self) -> bool {
        self.0 != 0
    }

    pub fn is_big_endian(&self) -> bool {
        (*self & property::BigEndian).is_set()
    }

    pub fn is_heritaged(&self) -> bool {
        (*self & property::Heritaged).is_set()
    }

    pub fn does_deadcode(&self) -> bool {
        (*self & property::DoesDeadcode).is_set()
    }

    pub fn is_program_specific(&self) -> bool {
        (*self & property::ProgramSpecific).is_set()
    }

    pub fn is_reverse_justified(&self) -> bool {
        (*self & property::ReverseJustification).is_set()
    }

    pub fn is_overlay(&self) -> bool {
        (*self & property::Overlay).is_set()
    }

    pub fn is_overlay_base(&self) -> bool {
        (*self & property::OverlayBase).is_set()
    }

    pub fn is_truncated(&self) -> bool {
        (*self & property::Truncated).is_set()
    }

    pub fn has_physical(&self) -> bool {
        (*self & property::HasPhysical).is_set()
    }

    pub fn iter() -> impl Iterator<Item = Self> {
        (0..)
            .map(|i| Self(1 << i))
            .take_while(|v| *v <= property::HasPhysical)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Space {
    index: usize,
    pub(crate) kind: SpaceKind,
    properties: SpaceProperty,
    highest: u64,
    name: String,
    address_size: usize,
    word_size: usize,
    delay: usize,
    deadcode_delay: usize,
}

impl Space {
    pub fn new<S: AsRef<str>>(
        kind: SpaceKind,
        name: S,
        address_size: usize,
        word_size: usize,
        index: usize,
        properties: Option<SpaceProperty>,
        delay: usize,
    ) -> Self {
        let properties = properties
            .map(|v| v & property::HasPhysical)
            .unwrap_or(SpaceProperty::default());

        let highest = calculate_mask(address_size) * (word_size as u64) + (word_size as u64 - 1);

        Space {
            kind: if name.as_ref() == "register" { SpaceKind::Register } else { kind },
            properties: properties | property::Heritaged | property::DoesDeadcode,
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

#[derive(Debug, Clone, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum AddressSpace {
    Constant(Space),
    Unique(Space),
    Space(Space),
}

impl PartialEq for AddressSpace {
    fn eq(&self, other: &Self) -> bool {
        self.index() == other.index()
    }
}
impl Eq for AddressSpace { }

impl Deref for AddressSpace {
    type Target = Space;

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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Deserialize, serde::Serialize)]
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
        matches!(self.kind(), SpaceKind::Register)
    }

    pub fn is_default(&self) -> bool {
        matches!(self.kind(), SpaceKind::Default)
    }

    pub fn constant<S: AsRef<str>>(name: S, index: usize) -> Self {
        let mut space = Self::Constant(Space::new(
            SpaceKind::Constant,
            name,
            mem::size_of::<u64>(),
            1,
            index,
            None,
            0,
        ));

        space.properties &= !(property::Heritaged | property::DoesDeadcode | property::BigEndian);
        if cfg!(target_endian = "big") {
            space.properties |= property::BigEndian;
        }
        space
    }

    pub fn unique<S: AsRef<str>>(name: S, index: usize, properties: Option<SpaceProperty>) -> Self {
        Self::Unique(Space::new(
            SpaceKind::Internal,
            name,
            mem::size_of::<usize>(),
            1,
            index,
            properties
                .map(|p| p | property::HasPhysical)
                .or(Some(property::HasPhysical)),
            0,
        ))
    }

    pub fn kind(&self) -> SpaceKind {
        self.kind
    }

    pub fn properties(&self) -> SpaceProperty {
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
            Self::Space(s) if s.kind == SpaceKind::Register => AddressSpaceId::register_id(s.index),
            Self::Space(s) if s.kind == SpaceKind::Default => AddressSpaceId::default_id(s.index),
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
        self.properties |= property::Truncated;
        self.address_size = size;
        self.highest = calculate_mask(self.address_size) * (self.word_size as u64)
            + (self.word_size as u64 - 1);
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, Error> {
        let name = input.attribute_string("name")?;
        let index = input.attribute_int("index")?;
        let address_size = input.attribute_int("size")?;
        let delay = input.attribute_int("delay")?;
        let word_size = input.attribute_int_opt("wordsize", 1)?;
        let deadcode_delay = input.attribute_int_opt("deadcodedelay", delay)?;

        let mut properties = property::Heritaged | property::DoesDeadcode;
        if input.attribute_bool("bigendian")? {
            properties |= property::BigEndian;
        }

        if input.attribute_bool("physical")? {
            properties |= property::HasPhysical;
        }

        let highest = calculate_mask(address_size) * (word_size as u64) + (word_size as u64 - 1);

        match input.tag_name().name() {
            /* These are not used in any .sla distributed with Ghidra:
            "space_base"
            "space_overlay"
            */
            "space_unique" => Ok(Self::Unique(Space {
                kind: SpaceKind::Internal,
                properties,
                name,
                highest,
                address_size,
                word_size,
                index,
                delay,
                deadcode_delay,
            })),
            "space" | "space_other" => Ok(Self::Space(Space {
                kind: SpaceKind::Processor,
                properties,
                name,
                highest,
                address_size,
                word_size,
                index,
                delay,
                deadcode_delay,
            })),
            tag => Err(Error::TagUnexpected(tag.to_owned())),
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
