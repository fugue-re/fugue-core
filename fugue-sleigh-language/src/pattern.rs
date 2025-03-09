use crate::deserialise::{DeserialiseError, XmlExt};
use crate::util;

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum PatternExpression {
    TokenField {
        big_endian: bool,
        sign_bit: bool,
        bit_start: usize,
        bit_end: usize,
        byte_start: usize,
        byte_end: usize,
        shift: u32,
    },
    ContextField {
        sign_bit: bool,
        bit_start: usize,
        bit_end: usize,
        byte_start: usize,
        byte_end: usize,
        shift: u32,
    },
    Constant {
        value: i64,
    },
    Operand {
        index: usize,
        table_id: usize,
        constructor_id: usize,
    },
    StartInstruction,
    EndInstruction,
    Next2Instruction,
    Plus(Box<Self>, Box<Self>),
    Sub(Box<Self>, Box<Self>),
    Mult(Box<Self>, Box<Self>),
    LeftShift(Box<Self>, Box<Self>),
    RightShift(Box<Self>, Box<Self>),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Xor(Box<Self>, Box<Self>),
    Div(Box<Self>, Box<Self>),
    Minus(Box<Self>),
    Not(Box<Self>),
}

impl PatternExpression {
    pub fn min_value(&self) -> Option<i64> {
        match self {
            Self::TokenField { .. }
            | Self::ContextField { .. }
            | Self::StartInstruction
            | Self::EndInstruction
            | Self::Next2Instruction => Some(0),
            Self::Constant { value, .. } => Some(*value),
            _ => None,
        }
    }

    pub fn max_value(&self) -> Option<i64> {
        match self {
            Self::TokenField {
                bit_start, bit_end, ..
            }
            | Self::ContextField {
                bit_start, bit_end, ..
            } => Some(util::zero_extend(!0i64, bit_end - bit_start)),
            Self::StartInstruction | Self::EndInstruction | Self::Next2Instruction => Some(0),
            Self::Constant { value, .. } => Some(*value),
            _ => None,
        }
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        Ok(match input.tag_name().name() {
            "tokenfield" => Self::TokenField {
                big_endian: input.attribute_bool("bigendian")?,
                sign_bit: input.attribute_bool("signbit")?,
                bit_start: input.attribute_int_or("bitstart", "startbit")?,
                bit_end: input.attribute_int_or("bitend", "endbit")?,
                byte_start: input.attribute_int_or("bytestart", "startbyte")?,
                byte_end: input.attribute_int_or("byteend", "endbyte")?,
                shift: input.attribute_int("shift")?,
            },
            "contextfield" => Self::ContextField {
                sign_bit: input.attribute_bool("signbit")?,
                bit_start: input.attribute_int("startbit")?,
                bit_end: input.attribute_int("endbit")?,
                byte_start: input.attribute_int("startbyte")?,
                byte_end: input.attribute_int("endbyte")?,
                shift: input.attribute_int("shift")?,
            },
            "intb" => Self::Constant {
                value: input.attribute_int("val")?,
            },
            "operand_exp" => Self::Operand {
                index: input.attribute_int("index")?,
                table_id: input.attribute_int("table")?,
                constructor_id: input.attribute_int("ct")?,
            },
            "start_exp" => Self::StartInstruction,
            "end_exp" => Self::EndInstruction,
            "next2_exp" => Self::Next2Instruction,
            "plus_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Plus(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing lhs of binary expression")
                    })?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing rhs of binary expression")
                    })?)?),
                )
            }
            "sub_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Sub(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing lhs of binary expression")
                    })?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing rhs of binary expression")
                    })?)?),
                )
            }
            "mult_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Mult(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing lhs of binary expression")
                    })?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing rhs of binary expression")
                    })?)?),
                )
            }
            "lshift_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::LeftShift(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing lhs of binary expression")
                    })?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing rhs of binary expression")
                    })?)?),
                )
            }
            "rshift_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::RightShift(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing lhs of binary expression")
                    })?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing rhs of binary expression")
                    })?)?),
                )
            }
            "and_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::And(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing lhs of binary expression")
                    })?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing rhs of binary expression")
                    })?)?),
                )
            }
            "or_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Or(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing lhs of binary expression")
                    })?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing rhs of binary expression")
                    })?)?),
                )
            }
            "xor_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Xor(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing lhs of binary expression")
                    })?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing rhs of binary expression")
                    })?)?),
                )
            }
            "div_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Div(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing lhs of binary expression")
                    })?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| {
                        DeserialiseError::Invariant("missing rhs of binary expression")
                    })?)?),
                )
            }
            "minus_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Minus(Box::new(Self::from_xml(children.next().ok_or_else(
                    || DeserialiseError::Invariant("missing operand of unary expression"),
                )?)?))
            }
            "not_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Not(Box::new(Self::from_xml(children.next().ok_or_else(
                    || DeserialiseError::Invariant("missing operand of unary expression"),
                )?)?))
            }
            name => return Err(DeserialiseError::TagUnexpected(name.to_owned())),
        })
    }
}
