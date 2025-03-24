use std::str::FromStr;

use fugue_ghidra_marshal::sla::*;
use fugue_ghidra_marshal::Decoder;

use crate::deserialise::{DeserialiseError, XmlExt};
use crate::opcode::Opcode;
use crate::spaces::{AddressSpaceId, AddressSpaces};

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum HandleKind {
    Space,
    Offset,
    Size,
    OffsetPlus(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum ConstTpl {
    Real(u64),
    Handle(usize, HandleKind),
    Start,
    Next,
    Next2,
    CurrentSpace,
    CurrentSpaceSize,
    SpaceId(AddressSpaceId),
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

    pub fn from_decoder<D: Decoder>(
        spaces: &AddressSpaces,
        input: &mut D,
        kind: u32,
    ) -> Result<Self, DeserialiseError> {
        // NOTE: we assume the element is opened
        #[cfg(feature = "tracing")]
        tracing::trace!("decoding constant template ({})", kind);

        let val = match kind {
            ELEM_CONST_REAL_ID => Self::Real(input.read_unsigned_integer_with_id(&ATTRIB_VAL)?),
            ELEM_CONST_HANDLE_ID => Self::Handle(
                input.read_signed_integer_with_id(&ATTRIB_VAL)? as usize,
                match input.read_signed_integer_with_id(&ATTRIB_S)? {
                    0 => HandleKind::Space,
                    1 => HandleKind::Offset,
                    2 => HandleKind::Size,
                    3 => HandleKind::OffsetPlus(input.read_unsigned_integer_with_id(&ATTRIB_PLUS)?),
                    _ => return Err(DeserialiseError::Invariant("invalid handle kind")),
                },
            ),
            ELEM_CONST_START_ID => Self::Start,
            ELEM_CONST_NEXT_ID => Self::Next,
            ELEM_CONST_NEXT2_ID => Self::Next2,
            ELEM_CONST_CURSPACE_ID => Self::CurrentSpace,
            ELEM_CONST_CURSPACE_SIZE_ID => Self::CurrentSpaceSize,
            ELEM_CONST_SPACEID_ID => Self::SpaceId(
                input
                    .read_space_with_id(&ATTRIB_SPACE)?
                    .index()
                    .and_then(|index| spaces.get(index).map(|spc| spc.id()))
                    .ok_or_else(|| DeserialiseError::Invariant("invalid space"))?,
            ),
            ELEM_CONST_RELATIVE_ID => {
                Self::Relative(input.read_unsigned_integer_with_id(&ATTRIB_VAL)?)
            }
            ELEM_CONST_FLOWREF_ID => Self::FlowRef,
            ELEM_CONST_FLOWREF_SIZE_ID => Self::FlowRefSize,
            ELEM_CONST_FLOWDEST_ID => Self::FlowDest,
            ELEM_CONST_FLOWDEST_SIZE_ID => Self::FlowDestSize,
            _ => return Err(DeserialiseError::Invariant("invalid ConstTpl type")),
        };

        Ok(val)
    }

    pub fn from_xml(input: xml::Node, spaces: &AddressSpaces) -> Result<Self, DeserialiseError> {
        Ok(
            match input
                .attribute("type")
                .or_else(|| input.tag_name().name().strip_prefix("const_"))
                .ok_or_else(|| DeserialiseError::AttributeExpected("type"))?
            {
                "real" => Self::Real(input.attribute_int("val")?),
                "handle" => Self::Handle(
                    input.attribute_int("val")?,
                    match input
                        .attribute("s")
                        .ok_or_else(|| DeserialiseError::AttributeExpected("s"))?
                    {
                        "space" | "0" => HandleKind::Space,
                        "offset" | "1" => HandleKind::Offset,
                        "size" | "2" => HandleKind::Size,
                        "offset_plus" | "3" => HandleKind::OffsetPlus(input.attribute_int("plus")?),
                        _ => return Err(DeserialiseError::Invariant("invalid handle kind")),
                    },
                ),
                "start" => Self::Start,
                "next" => Self::Next,
                "next2" => Self::Next2,
                "curspace" => Self::CurrentSpace,
                "curspace_size" => Self::CurrentSpaceSize,
                "spaceid" => Self::SpaceId(
                    spaces
                        .space_by_name(input.attribute_string_or("name", "space")?)
                        .unwrap()
                        .id(),
                ),
                "relative" => Self::Relative(input.attribute_int("val")?),
                "flowref" => Self::FlowRef,
                "flowref_size" => Self::FlowRefSize,
                "flowdest" => Self::FlowDest,
                "flowdest_size" => Self::FlowDestSize,
                _ => return Err(DeserialiseError::Invariant("invalid ConstTpl type")),
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
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
    pub fn space(&self) -> &ConstTpl {
        &self.space
    }

    pub fn size(&self) -> &ConstTpl {
        &self.size
    }

    pub fn ptr_space(&self) -> &ConstTpl {
        &self.ptr_space
    }

    pub fn ptr_offset(&self) -> &ConstTpl {
        &self.ptr_offset
    }

    pub fn ptr_size(&self) -> &ConstTpl {
        &self.ptr_size
    }

    pub fn tmp_space(&self) -> &ConstTpl {
        &self.tmp_space
    }

    pub fn tmp_offset(&self) -> &ConstTpl {
        &self.tmp_offset
    }

    pub fn from_decoder<D: Decoder>(
        spaces: &AddressSpaces,
        input: &mut D,
    ) -> Result<Self, DeserialiseError> {
        // NOTE: assumes element is open
        #[cfg(feature = "tracing")]
        tracing::trace!("decoding handle template");

        let mut const_tpl = || {
            let elem = input.open_element()?;
            let tpl = ConstTpl::from_decoder(spaces, input, elem)?;
            input.close_element(elem)?;
            Ok::<ConstTpl, DeserialiseError>(tpl)
        };

        Ok(Self {
            space: const_tpl()?,
            size: const_tpl()?,
            ptr_space: const_tpl()?,
            ptr_offset: const_tpl()?,
            ptr_size: const_tpl()?,
            tmp_space: const_tpl()?,
            tmp_offset: const_tpl()?,
        })
    }

    pub fn from_xml(input: xml::Node, spaces: &AddressSpaces) -> Result<Self, DeserialiseError> {
        let mut children = input
            .children()
            .filter(xml::Node::is_element)
            .map(|c| ConstTpl::from_xml(c, spaces));

        Ok(Self {
            space: children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("space missing for HandleTpl"))??,
            size: children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("size missing for HandleTpl"))??,
            ptr_space: children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("ptr_space missing for HandleTpl"))??,
            ptr_offset: children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("ptr_offset missing for HandleTpl"))??,
            ptr_size: children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("ptr_size missing for HandleTpl"))??,
            tmp_space: children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("tmp_space missing for HandleTpl"))??,
            tmp_offset: children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("tmp_offset missing for HandleTpl"))??,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct VarnodeTpl {
    space: ConstTpl,
    offset: ConstTpl,
    size: ConstTpl,
}

impl VarnodeTpl {
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

    pub fn from_decoder<D: Decoder>(
        spaces: &AddressSpaces,
        input: &mut D,
    ) -> Result<Self, DeserialiseError> {
        // NOTE: we assume the element is opened
        #[cfg(feature = "tracing")]
        tracing::trace!("decoding varnode template");

        let mut const_tpl = || {
            let elem = input.open_element()?;
            let tpl = ConstTpl::from_decoder(spaces, input, elem)?;
            input.close_element(elem)?;
            Ok::<ConstTpl, DeserialiseError>(tpl)
        };

        Ok(Self {
            space: const_tpl()?,
            offset: const_tpl()?,
            size: const_tpl()?,
        })
    }

    pub fn from_xml(input: xml::Node, spaces: &AddressSpaces) -> Result<Self, DeserialiseError> {
        let mut children = input
            .children()
            .filter(xml::Node::is_element)
            .map(|c| ConstTpl::from_xml(c, spaces));

        Ok(Self {
            space: children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("space missing for VarnodeTpl"))??,
            offset: children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("offset missing for VarnodeTpl"))??,
            size: children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("size missing for VarnodeTpl"))??,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
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

    pub fn inputs(&self) -> &[VarnodeTpl] {
        &self.inputs
    }

    pub fn output(&self) -> Option<&VarnodeTpl> {
        self.output.as_ref()
    }

    pub fn input_count(&self) -> usize {
        self.inputs.len()
    }

    pub fn from_decoder<D: Decoder>(
        spaces: &AddressSpaces,
        input: &mut D,
    ) -> Result<Self, DeserialiseError> {
        // NOTE: we assume the element is opened
        #[cfg(feature = "tracing")]
        tracing::trace!("decoding operation template");

        let opcode = input
            .read_signed_integer_with_id(&ATTRIB_CODE)
            .map_err(DeserialiseError::from)
            .and_then(Opcode::try_from)?;

        let elem = input.open_element()?;
        let output = if elem == ELEM_NULL_ID {
            None
        } else {
            Some(VarnodeTpl::from_decoder(spaces, input)?)
        };
        input.close_element(elem)?;

        let mut inputs = Vec::new();
        while input.peek_element()? != 0 {
            let elem = input.open_element_with_id(&ELEM_VARNODE_TPL)?;
            inputs.push(VarnodeTpl::from_decoder(spaces, input)?);
            input.close_element(elem)?;
        }

        Ok(Self {
            opcode,
            inputs,
            output,
        })
    }

    pub fn from_xml(input: xml::Node, spaces: &AddressSpaces) -> Result<Self, DeserialiseError> {
        let opcode = input
            .attribute("code")
            .map(Opcode::from_str)
            .ok_or_else(|| DeserialiseError::AttributeExpected("code"))??;

        let mut children = input.children().filter(xml::Node::is_element);

        let output = children
            .next()
            .map(|input| {
                if input.tag_name().name() == "null" {
                    None
                } else {
                    Some(VarnodeTpl::from_xml(input, spaces))
                }
                .transpose()
            })
            .ok_or_else(|| DeserialiseError::Invariant("output missing for OpTpl"))??;

        let inputs = children
            .map(|v| VarnodeTpl::from_xml(v, spaces))
            .collect::<Result<Vec<VarnodeTpl>, _>>()?;

        Ok(OpTpl {
            opcode,
            inputs,
            output,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
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

    pub fn from_decoder<D: Decoder>(
        spaces: &AddressSpaces,
        input: &mut D,
    ) -> Result<Self, DeserialiseError> {
        // NOTE: assumes element is open
        #[cfg(feature = "tracing")]
        tracing::trace!("decoding constructor template");

        let delay_slot = input
            .try_read_signed_integer_with_id(&ATTRIB_DELAY)?
            .unwrap_or(0) as usize;
        let labels = input
            .try_read_signed_integer_with_id(&ATTRIB_LABELS)?
            .unwrap_or(0) as usize;
        let section_id = input
            .try_read_signed_integer_with_id(&ATTRIB_SECTION)?
            .and_then(|id| if id < 0 { None } else { Some(id as usize) });

        let elem = input.open_element()?;
        let result = if elem == ELEM_NULL_ID {
            None
        } else {
            Some(HandleTpl::from_decoder(spaces, input)?)
        };
        input.close_element(elem)?;

        let mut operations = Vec::new();
        while input.peek_element()? != 0 {
            let elem = input.open_element_with_id(&ELEM_OP_TPL)?;
            operations.push(OpTpl::from_decoder(spaces, input)?);
            input.close_element(elem)?;
        }

        Ok(Self {
            delay_slot,
            labels,
            section_id,
            result,
            operations,
        })
    }

    pub fn from_xml(input: xml::Node, spaces: &AddressSpaces) -> Result<Self, DeserialiseError> {
        let delay_slot = input.attribute_int_opt("delay", 0)?;
        let labels = input.attribute_int_opt("labels", 0)?;
        let section_id = input.attribute_int_opt::<i64>("section", -1).map(|i| {
            if i < 0 {
                None
            } else {
                Some(i as usize)
            }
        })?;
        let mut children = input.children().filter(xml::Node::is_element);

        let result = children
            .next()
            .map(|input| {
                if input.tag_name().name() == "null" {
                    None
                } else {
                    Some(HandleTpl::from_xml(input, spaces))
                }
                .transpose()
            })
            .ok_or_else(|| DeserialiseError::Invariant("result missing for ConstructTpl"))??;

        let operations = children
            .map(|o| OpTpl::from_xml(o, spaces))
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
