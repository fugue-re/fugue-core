use crate::disassembly::{Error, ParserWalker};
use crate::disassembly::symbol::FixedHandle;
use crate::deserialise::Error as DeserialiseError;
use crate::deserialise::parse::XmlExt;

use crate::space::AddressSpace;
use crate::space_manager::SpaceManager;
//use crate::error::disassembly as di;
use crate::opcode::Opcode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandleKind {
    Space,
    Offset,
    Size,
    OffsetPlus(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstTpl {
    Real(u64),
    Handle(usize, HandleKind),
    Start,
    Next,
    CurrentSpace,
    CurrentSpaceSize,
    SpaceId(String),
    Relative(u64),
    FlowRef,
    FlowRefSize,
    FlowDest,
    FlowDestSize,
}

impl ConstTpl {
    pub fn is_handle(&self) -> bool {
        matches!(self, Self::Handle(_, _))
    }

    pub fn is_real(&self) -> bool {
        matches!(self, Self::Real(_))
    }

    pub fn real(&self) -> u64 {
        match self {
            Self::Real(value) => *value,
            _ => 0,
        }
    }

    pub fn handle_index(&self) -> Option<usize> {
        match self {
            Self::Handle(index, _) => Some(*index),
            _ => None,
        }
    }

    pub fn is_relative(&self) -> bool {
        matches!(self, Self::Relative { .. })
    }

    pub fn fix<'a, 'b, 'c>(&'b self, walker: &mut ParserWalker<'a, 'b, 'c>, manager: &'a SpaceManager) -> Result<u64, Error> {
        Ok(match self {
            Self::Start => walker.address().offset(),
            Self::Next => walker.next_address().ok_or_else(|| Error::InvalidNextAddress)?.offset(),
            Self::CurrentSpaceSize => walker.address().space().address_size() as u64,
            Self::CurrentSpace => walker.address().space().index() as u64,
            Self::Relative(value) | Self::Real(value) => *value,
            Self::SpaceId(name) => manager.space_by_name(name)
                .ok_or_else(|| Error::InvalidSpace)?
                .index() as u64,
            Self::Handle(index, kind) => {
                let handle = walker.handle(*index)?.ok_or_else(|| Error::InvalidHandle)?;
                match kind {
                    HandleKind::Space => {
                        if handle.offset_space.is_none() {
                            handle.space.index() as u64
                        } else {
                            handle.temporary_space.ok_or_else(|| Error::InvalidSpace)?.index() as u64
                        }
                    },
                    HandleKind::Offset => {
                        if handle.offset_space.is_none() {
                            handle.offset_offset
                        } else {
                            handle.temporary_offset
                        }
                    },
                    HandleKind::Size => {
                        handle.size as u64
                    },
                    HandleKind::OffsetPlus(value) => {
                        if manager.constant_space().ok_or_else(|| Error::InvalidSpace)? != handle.space {
                            if handle.offset_space.is_none() {
                                handle.offset_offset + (*value & 0xffff)
                            } else {
                                handle.temporary_offset + (*value & 0xffff)
                            }
                        } else {
                            let val = if handle.offset_space.is_none() {
                                handle.offset_offset
                            } else {
                                handle.temporary_offset
                            };
                            val.checked_shr(8 * value.checked_shr(16).unwrap_or(0) as u32)
                                .unwrap_or(0)
                        }
                    }
                }
            },
            f => unimplemented!("flow {:?}.. not seen", f),
        })
    }

    pub fn offset<'a, 'b, 'c>(&'b self, handle: &mut FixedHandle<'a>, walker: &mut ParserWalker<'a, 'b, 'c>, manager: &'a SpaceManager) -> Result<(), Error> {
        Ok(match self {
            Self::Handle(index, _) => {
                let h = walker.handle(*index)?
                    .ok_or_else(|| Error::InvalidHandle)?;
                handle.offset_space = h.offset_space;
                handle.offset_offset = h.offset_offset;
                handle.offset_size = h.offset_size;
                handle.temporary_space = h.temporary_space.clone();
                handle.temporary_offset = h.temporary_offset;
            },
            _ => {
                handle.offset_space = None;
                handle.offset_offset = handle.space
                    .wrap_offset(self.fix(walker, manager)?);
            }
        })
    }

    pub fn space<'a, 'b, 'c>(&'b self, walker: &mut ParserWalker<'a, 'b, 'c>, manager: &'a SpaceManager) -> Result<&'a AddressSpace, Error> {
        Ok(match self {
            Self::CurrentSpace => walker.address().space(),
            Self::Handle(index, kind) => {
                if *kind == HandleKind::Space {
                    walker.handle(*index)?.ok_or_else(|| Error::InvalidHandle)?.space
                } else {
                    return Err(Error::InconsistentState)
                }
            },
            Self::SpaceId(name) => {
                manager.space_by_name(name).unwrap()
            },
            _ => return Err(Error::InconsistentState)
        })
    }

    pub fn fix_space<'a, 'b, 'c>(&'b self, walker: &mut ParserWalker<'a, 'b, 'c>, manager: &'a SpaceManager) -> Result<Option<&'a AddressSpace>, Error> {
        Ok(match self {
            Self::CurrentSpace => Some(walker.address().space()),
            Self::Handle(index, kind) => {
                if *kind == HandleKind::Space {
                    let h = walker.handle(*index)?.ok_or_else(|| Error::InvalidHandle)?;
                    if h.offset_space.is_none() {
                        Some(h.space)
                    } else {
                        h.temporary_space.clone()
                    }
                } else {
                    return Err(Error::InconsistentState)
                }
            },
            Self::SpaceId(name) => {
                manager.space_by_name(name)
            },
            _ => return Err(Error::InconsistentState)
        })
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        Ok(match input.attribute("type").ok_or_else(|| DeserialiseError::AttributeExpected("type"))? {
            "real" => Self::Real(input.attribute_int("val")?),
            "handle" => Self::Handle(
                input.attribute_int("val")?,
                match input.attribute("s").ok_or_else(|| DeserialiseError::AttributeExpected("s"))? {
                    "space" => HandleKind::Space,
                    "offset" => HandleKind::Offset,
                    "size" => HandleKind::Size,
                    "offset_plus" => HandleKind::OffsetPlus(input.attribute_int("plus")?),
                    _ => return Err(DeserialiseError::Invariant("invalid handle kind")),
                },
            ),
            "start" => Self::Start,
            "next" => Self::Next,
            "curspace" => Self::CurrentSpace,
            "curspace_size" => Self::CurrentSpaceSize,
            "spaceid" => Self::SpaceId(input.attribute_string("name")?),
            "relative" => Self::Relative(input.attribute_int("val")?),
            "flowref" => Self::FlowRef,
            "flowref_size" => Self::FlowRefSize,
            "flowdest" => Self::FlowDest,
            "flowdest_size" => Self::FlowDestSize,
            _ => return Err(DeserialiseError::Invariant("invalid ConstTpl type"))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandleTpl {
    space: ConstTpl,
    size: ConstTpl,
    ptr_space: ConstTpl,
    ptr_offset: ConstTpl,
    ptr_size: ConstTpl,
    tmp_space: ConstTpl,
    tmp_offset: ConstTpl,
}

impl HandleTpl {
    pub fn fix<'a, 'b, 'c>(&'b self, walker: &mut ParserWalker<'a, 'b, 'c>, manager: &'a SpaceManager) -> Result<FixedHandle<'a>, Error> {
        if self.ptr_space.is_real() {
            let mut handle = FixedHandle::new(self.space.space(walker, manager)?);
            handle.size = self.size.fix(walker, manager)? as usize;
            self.ptr_offset.offset(&mut handle, walker, manager)?;
            Ok(handle)
        } else {
            let mut handle = FixedHandle::new(self.space.fix_space(walker, manager)?
                                              .ok_or_else(|| Error::InconsistentState)?);
            handle.size = self.size.fix(walker, manager)? as usize;
            handle.offset_offset = self.ptr_offset.fix(walker, manager)?;
            handle.offset_space = self.ptr_space.fix_space(walker, manager)?;

            if handle.offset_space.as_ref().ok_or_else(|| Error::InconsistentState)?.is_constant() {
                handle.offset_space = None;
                handle.offset_offset = handle.offset_offset * handle.space.word_size() as u64;
                handle.offset_offset = handle.space.wrap_offset(handle.offset_offset);
            } else {
                handle.offset_size = self.ptr_size.fix(walker, manager)? as usize;
                handle.temporary_space = self.tmp_space.fix_space(walker, manager)?;
                handle.temporary_offset = self.tmp_offset.fix(walker, manager)?;
            }

            Ok(handle)
        }
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        let mut children = input.children()
            .filter(xml::Node::is_element)
            .map(ConstTpl::from_xml);

        Ok(Self {
            space: children.next().ok_or_else(|| DeserialiseError::Invariant("space missing for HandleTpl"))??,
            size: children.next().ok_or_else(|| DeserialiseError::Invariant("size missing for HandleTpl"))??,
            ptr_space: children.next().ok_or_else(|| DeserialiseError::Invariant("ptr_space missing for HandleTpl"))??,
            ptr_offset: children.next().ok_or_else(|| DeserialiseError::Invariant("ptr_offset missing for HandleTpl"))??,
            ptr_size: children.next().ok_or_else(|| DeserialiseError::Invariant("ptr_size missing for HandleTpl"))??,
            tmp_space: children.next().ok_or_else(|| DeserialiseError::Invariant("tmp_space missing for HandleTpl"))??,
            tmp_offset: children.next().ok_or_else(|| DeserialiseError::Invariant("tmp_offset missing for HandleTpl"))??,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarnodeTpl {
    space: ConstTpl,
    offset: ConstTpl,
    size: ConstTpl,
}

impl VarnodeTpl {
    /*
    pub fn is_dynamic(&self, walker: &mut ParserWalker) -> Result<bool, di::Error> {
        if let ConstTpl::Handle(index, _) = self.offset {
            Ok(walker.handle(index)?
                .ok_or_else(|| Error::InvalidHandle)?
                .offset_space
                .is_some())
        } else {
            Ok(false)
        }
    }
    */

    pub fn is_relative(&self) -> bool {
        self.offset.is_relative()
    }

    pub fn space(&self) -> &ConstTpl {
        &self.space
    }

    pub fn offset(&self) -> &ConstTpl {
        &self.offset
    }

    pub fn size(&self) -> &ConstTpl {
        &self.size
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        let mut children = input.children()
            .filter(xml::Node::is_element)
            .map(ConstTpl::from_xml);

        Ok(Self {
            space: children.next().ok_or_else(|| DeserialiseError::Invariant("space missing for VarnodeTpl"))??,
            offset: children.next().ok_or_else(|| DeserialiseError::Invariant("offset missing for VarnodeTpl"))??,
            size: children.next().ok_or_else(|| DeserialiseError::Invariant("size missing for VarnodeTpl"))??,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpTpl {
    opcode: Opcode,
    inputs: Vec<VarnodeTpl>,
    output: Option<VarnodeTpl>,
}

impl OpTpl {
    pub fn opcode(&self) -> Opcode {
        self.opcode
    }

    pub fn input(&self, index: usize) -> &VarnodeTpl {
        &self.inputs[index]
    }

    pub fn output(&self) -> Option<&VarnodeTpl> {
        self.output.as_ref()
    }

    pub fn input_count(&self) -> usize {
        self.inputs.len()
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        let opcode = input.attribute("code")
            .map(Opcode::from_str)
            .ok_or_else(|| DeserialiseError::AttributeExpected("code"))??;

        let mut children = input.children()
            .filter(xml::Node::is_element);

        let output = children.next()
            .map(|input| if input.tag_name().name() == "null" {
                None
            } else {
                Some(VarnodeTpl::from_xml(input))
            }.transpose())
            .ok_or_else(|| DeserialiseError::Invariant("output missing for OpTpl"))??;

        let inputs = children.map(VarnodeTpl::from_xml)
            .collect::<Result<Vec<VarnodeTpl>, _>>()?;

        Ok(OpTpl {
            opcode,
            inputs,
            output,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructTpl {
    delay_slot: usize,
    labels: usize,
    section_id: Option<usize>,
    result: Option<HandleTpl>,
    operations: Vec<OpTpl>,
}

impl ConstructTpl {
    pub fn section_id(&self) -> Option<usize> {
        self.section_id
    }

    pub fn delay_slot(&self) -> usize {
        self.delay_slot
    }

    pub fn labels(&self) -> usize {
        self.labels
    }

    pub fn operations(&self) -> &[OpTpl] {
        self.operations.as_ref()
    }

    pub fn result(&self) -> Option<&HandleTpl> {
        self.result.as_ref()
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        let delay_slot = input.attribute_int_opt("delay", 0)?;
        let labels = input.attribute_int_opt("labels", 0)?;
        let section_id = input.attribute_int_opt::<i64>("section", -1)
            .map(|i| if i < 0 { None } else { Some(i as usize) })?;
        let mut children = input.children().filter(xml::Node::is_element);

        let result = children.next()
            .map(|input| if input.tag_name().name() == "null" {
                None
            } else {
                Some(HandleTpl::from_xml(input))
            }.transpose())
            .ok_or_else(|| DeserialiseError::Invariant("result missing for ConstructTpl"))??;

        let operations = children
            .map(OpTpl::from_xml)
            .collect::<Result<Vec<OpTpl>, _>>()?;

        Ok(Self {
            delay_slot,
            labels,
            section_id,
            result,
            operations,
        })
    }
}
