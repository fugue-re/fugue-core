use crate::bits;
//use crate::disassembly::ParserWalker;
use crate::deserialise::Error as DeserialiseError;
use crate::deserialise::parse::XmlExt;
use crate::disassembly::symbol::{Symbol, SymbolTable};
use crate::disassembly::walker::ParserWalker;
use crate::disassembly::error::Error;

use std::mem::size_of;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
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
            Self::TokenField { .. } |
            Self::ContextField { .. } |
            Self::StartInstruction |
            Self::EndInstruction => Some(0),
            Self::Constant { value, .. } => Some(*value),
            _ => None,
        }
    }

    pub fn max_value(&self) -> Option<i64> {
        match self {
            Self::TokenField { bit_start, bit_end, .. } |
            Self::ContextField { bit_start, bit_end, .. } => {
                //Some(!(!0i64).checked_shl((bit_end - bit_start) as u32 + 1).unwrap_or(0))
                Some(bits::zero_extend(!0i64, bit_end - bit_start))
            },
            Self::StartInstruction |
            Self::EndInstruction => Some(0),
            Self::Constant { value, .. } => Some(*value),
            _ => None,
        }
    }

    pub fn value_with<'b, 'c, 'z>(&'b self, walker: &mut ParserWalker<'b, 'c, 'z>, symbols: &'b SymbolTable) -> Result<(i64, Option<Range<u32>>), Error> {
        Ok(match self {
            Self::TokenField {
                big_endian,
                sign_bit,
                bit_start,
                bit_end,
                byte_start,
                byte_end,
                shift,
            } => {
                let size = byte_end - byte_start + 1;
                let mut res = 0i64;
                let mut start = *byte_start as isize;
                let mut tsize = size as isize;

                while tsize >= size_of::<u32>() as isize {
                    let tmp = walker.instruction_bytes(start as usize, size_of::<u32>())?;
                    res = res.checked_shl(8 * size_of::<u32>() as u32).unwrap_or(0);
                    res = (res as u64 | tmp as u64) as i64;
                    start += size_of::<u32>() as isize;
                    tsize = (*byte_end as isize) - start + 1;
                }
                if tsize > 0 {
                    let tmp = walker.instruction_bytes(start as usize, tsize as usize)?;
                    res = res.checked_shl(8 * tsize as u32).unwrap_or(0);
                    res = (res as u64 | tmp as u64) as i64;
                }

                res = if !big_endian { bits::byte_swap(res, size) } else { res };
                res = res.checked_shr(*shift).unwrap_or(if res < 0 { -1 } else { 0 });

                let value = if *sign_bit {
                    bits::sign_extend(res, bit_end - bit_start)
                } else {
                    bits::zero_extend(res, bit_end - bit_start)
                };

                let offset = (*byte_start + walker.offset(None)) as u32 * 8;
                let end_offset = size as u32 * 8;

                (value, Some(offset + end_offset - (*bit_end as u32 + 1)..offset + end_offset - *bit_start as u32))
            },
            Self::ContextField {
                sign_bit,
                bit_start,
                bit_end,
                byte_start,
                byte_end,
                shift,
            } => {
                let mut res = 0i64;
                let mut size = (*byte_end as isize) - (*byte_start as isize) + 1;
                let mut start = *byte_start as isize;

                while size >= size_of::<u32>() as isize {
                    let tmp = walker.context_bytes(start as usize, size_of::<u32>());
                    res = res.checked_shl(8 * size_of::<u32>() as u32).unwrap_or(0);
                    res = (res as u64 | tmp as u64) as i64;
                    start += size_of::<u32>() as isize;
                    size = (*byte_end as isize) - start + 1;
                }
                if size > 0 {
                    let tmp = walker.context_bytes(start as usize, size as usize);
                    res = res.checked_shl(8 * size as u32).unwrap_or(0);
                    res = (res as u64 | tmp as u64) as i64;
                }

                res = res.checked_shr(*shift).unwrap_or(if res < 0 { -1 } else { 0 });

                let value = if *sign_bit {
                    bits::sign_extend(res, bit_end - bit_start)
                } else {
                    bits::zero_extend(res, bit_end - bit_start)
                };

                let offset = (*byte_start + walker.offset(None)) as u32 * 8;
                let end_offset = size as u32 * 8;

                (value, Some(offset + end_offset - (*bit_end as u32 + 1)..offset + end_offset - *bit_start as u32))
            },
            Self::Constant { value } => (*value, None),
            Self::Operand {
                table_id,
                constructor_id,
                index,
            } => {
                let table = symbols.unchecked_symbol(*table_id); //.ok_or_else(|| Error::InvalidSymbol)?;
                let ctor = if let Symbol::Subtable { constructors, .. } = table {
                    unsafe { constructors.get_unchecked(*constructor_id) }
                } else {
                    unreachable!()
                    //return Err(Error::InconsistentState)
                };

                let pexp = if let Symbol::Operand {
                    def_expr,
                    subsym_id,
                    ..
                } = symbols.unchecked_symbol(ctor.operand(*index)) /* .ok_or_else(|| Error::InvalidSymbol)? */ {
                    if let Some(def_expr) = def_expr {
                        def_expr
                    } else if let Some(subsym_id) = subsym_id {
                        let sym = symbols.unchecked_symbol(*subsym_id); /* .ok_or_else(|| Error::InvalidSymbol)?; */
                        sym.pattern_value()
                    } else {
                        return Ok((0, None))
                    }
                } else {
                    unreachable!()
                    //return Err(Error::InconsistentState)
                };

                let (value, bits) = walker.resolve_with_bits(pexp, ctor, *index, symbols)?;
                (value, bits.map(|r| {
                    let offset = 0; //walker.offset(None) as u32 * 8;
                    r.start + offset..r.end + offset
                }))
            },
            Self::StartInstruction => (walker.address().offset() as i64, None),
            Self::EndInstruction => (walker.next_address().map(|a| a.offset() as i64).unwrap_or(0), None),
            Self::Plus(ref lhs, ref rhs) => {
                let (l, m) = lhs.value_with(walker, symbols)?;
                let (r, n) = rhs.value_with(walker, symbols)?;

                (l.wrapping_add(r), match (m, n) {
                    (None, v) | (v, None) => v,
                    (Some(r1), Some(r2)) => Some(r1.start.min(r2.start)..r1.end.max(r2.end))
                })
            },
            Self::Sub(ref lhs, ref rhs) => {
                let (l, m) = lhs.value_with(walker, symbols)?;
                let (r, n) = rhs.value_with(walker, symbols)?;

                (l.wrapping_sub(r), match (m, n) {
                    (None, v) | (v, None) => v,
                    (Some(r1), Some(r2)) => Some(r1.start.min(r2.start)..r1.end.max(r2.end))
                })
            },
            Self::Mult(ref lhs, ref rhs) => {
                let (l, m) = lhs.value_with(walker, symbols)?;
                let (r, n) = rhs.value_with(walker, symbols)?;

                (l.wrapping_mul(r), match (m, n) {
                    (None, v) | (v, None) => v,
                    (Some(r1), Some(r2)) => Some(r1.start.min(r2.start)..r1.end.max(r2.end))
                })
            },
            Self::LeftShift(ref lhs, ref rhs) => {
                let (l, m) = lhs.value_with(walker, symbols)?;
                let (r, n) = rhs.value_with(walker, symbols)?;

                (l.checked_shl(r as u8 as u32).unwrap_or(0), match (m, n) {
                    (None, v) | (v, None) => v,
                    (Some(r1), Some(r2)) => Some(r1.start.min(r2.start)..r1.end.max(r2.end))
                })
            },
            Self::RightShift(ref lhs, ref rhs) => {
                let (l, m) = lhs.value_with(walker, symbols)?;
                let (r, n) = rhs.value_with(walker, symbols)?;

                (l.checked_shr(r as u8 as u32)
                    .unwrap_or(if l < 0 { -1 } else { 0 }), match (m, n) {
                    (None, v) | (v, None) => v,
                    (Some(r1), Some(r2)) => Some(r1.start.min(r2.start)..r1.end.max(r2.end))
                })
            },
            Self::And(ref lhs, ref rhs) => {
                let (l, m) = lhs.value_with(walker, symbols)?;
                let (r, n) = rhs.value_with(walker, symbols)?;

                (l & r, match (m, n) {
                    (None, v) | (v, None) => v,
                    (Some(r1), Some(r2)) => Some(r1.start.min(r2.start)..r1.end.max(r2.end))
                })
            },
            Self::Or(ref lhs, ref rhs) => {
                let (l, m) = lhs.value_with(walker, symbols)?;
                let (r, n) = rhs.value_with(walker, symbols)?;

                (l | r, match (m, n) {
                    (None, v) | (v, None) => v,
                    (Some(r1), Some(r2)) => Some(r1.start.min(r2.start)..r1.end.max(r2.end))
                })
            },
            Self::Xor(ref lhs, ref rhs) => {
                let (l, m) = lhs.value_with(walker, symbols)?;
                let (r, n) = rhs.value_with(walker, symbols)?;

                (l ^ r, match (m, n) {
                    (None, v) | (v, None) => v,
                    (Some(r1), Some(r2)) => Some(r1.start.min(r2.start)..r1.end.max(r2.end))
                })
            },
            Self::Div(ref lhs, ref rhs) => {
                let (l, m) = lhs.value_with(walker, symbols)?;
                let (r, n) = rhs.value_with(walker, symbols)?;

                (l.wrapping_div(r), match (m, n) {
                    (None, v) | (v, None) => v,
                    (Some(r1), Some(r2)) => Some(r1.start.min(r2.start)..r1.end.max(r2.end))
                })
            },
            Self::Minus(ref operand) => {
                let (operand, m) = operand.value_with(walker, symbols)?;
                (-operand, m)
            },
            Self::Not(ref operand) => {
                let (operand, m) = operand.value_with(walker, symbols)?;
                (!operand, m)
            },
        })
    }

    pub fn value<'b, 'c, 'z>(&'b self, walker: &mut ParserWalker<'b, 'c, 'z>, symbols: &'b SymbolTable) -> Result<i64, Error> {
        Ok(match self {
            Self::TokenField {
                big_endian,
                sign_bit,
                bit_start,
                bit_end,
                byte_start,
                byte_end,
                shift,
            } => {
                let size = byte_end - byte_start + 1;
                let mut res = 0i64;
                let mut start = *byte_start as isize;
                let mut tsize = size as isize;

                while tsize >= size_of::<u32>() as isize {
                    let tmp = walker.instruction_bytes(start as usize, size_of::<u32>())?;
                    res = res.checked_shl(8 * size_of::<u32>() as u32).unwrap_or(0);
                    res = (res as u64 | tmp as u64) as i64;
                    start += size_of::<u32>() as isize;
                    tsize = (*byte_end as isize) - start + 1;
                }
                if tsize > 0 {
                    let tmp = walker.instruction_bytes(start as usize, tsize as usize)?;
                    res = res.checked_shl(8 * tsize as u32).unwrap_or(0);
                    res = (res as u64 | tmp as u64) as i64;
                }

                res = if !big_endian { bits::byte_swap(res, size) } else { res };
                res = res.checked_shr(*shift).unwrap_or(if res < 0 { -1 } else { 0 });

                if *sign_bit {
                    bits::sign_extend(res, bit_end - bit_start)
                } else {
                    bits::zero_extend(res, bit_end - bit_start)
                }
            },
            Self::ContextField {
                sign_bit,
                bit_start,
                bit_end,
                byte_start,
                byte_end,
                shift,
            } => {
                let mut res = 0i64;
                let mut size = (*byte_end as isize) - (*byte_start as isize) + 1;
                let mut start = *byte_start as isize;

                while size >= size_of::<u32>() as isize {
                    let tmp = walker.context_bytes(start as usize, size_of::<u32>());
                    res = res.checked_shl(8 * size_of::<u32>() as u32).unwrap_or(0);
                    res = (res as u64 | tmp as u64) as i64;
                    start += size_of::<u32>() as isize;
                    size = (*byte_end as isize) - start + 1;
                }
                if size > 0 {
                    let tmp = walker.context_bytes(start as usize, size as usize);
                    res = res.checked_shl(8 * size as u32).unwrap_or(0);
                    res = (res as u64 | tmp as u64) as i64;
                }

                res = res.checked_shr(*shift).unwrap_or(if res < 0 { -1 } else { 0 });

                if *sign_bit {
                    bits::sign_extend(res, bit_end - bit_start)
                } else {
                    bits::zero_extend(res, bit_end - bit_start)
                }
            },
            Self::Constant { value } => *value,
            Self::Operand {
                table_id,
                constructor_id,
                index,
            } => {
                let table = symbols.unchecked_symbol(*table_id); //.ok_or_else(|| Error::InvalidSymbol)?;
                let ctor = if let Symbol::Subtable { constructors, .. } = table {
                    unsafe { constructors.get_unchecked(*constructor_id) }
                } else {
                    unreachable!()
                    //return Err(Error::InconsistentState)
                };

                let pexp = if let Symbol::Operand {
                    def_expr,
                    subsym_id,
                    ..
                } = symbols.unchecked_symbol(ctor.operand(*index)) /* .ok_or_else(|| Error::InvalidSymbol)? */ {
                    if let Some(def_expr) = def_expr {
                        def_expr
                    } else if let Some(subsym_id) = subsym_id {
                        let sym = symbols.unchecked_symbol(*subsym_id); /* .ok_or_else(|| Error::InvalidSymbol)?; */
                        sym.pattern_value()
                    } else {
                        return Ok(0)
                    }
                } else {
                    unreachable!()
                    //return Err(Error::InconsistentState)
                };

                walker.resolve_with(pexp, ctor, *index, symbols)?
            },
            Self::StartInstruction => walker.address().offset() as i64,
            Self::EndInstruction => walker.next_address().map(|a| a.offset() as i64).unwrap_or(0),
            Self::Plus(ref lhs, ref rhs) => {
                lhs.value(walker, symbols)?
                    .wrapping_add(rhs.value(walker, symbols)?)
            },
            Self::Sub(ref lhs, ref rhs) => {
                lhs.value(walker, symbols)?
                    .wrapping_sub(rhs.value(walker, symbols)?)
            },
            Self::Mult(ref lhs, ref rhs) => {
                lhs.value(walker, symbols)?
                    .wrapping_mul(rhs.value(walker, symbols)?)
            },
            Self::LeftShift(ref lhs, ref rhs) => {
                let l = lhs.value(walker, symbols)?;
                let r = rhs.value(walker, symbols)?;

                l.checked_shl(r as u8 as u32).unwrap_or(0)
            },
            Self::RightShift(ref lhs, ref rhs) => {
                let l = lhs.value(walker, symbols)?;
                let r = rhs.value(walker, symbols)?;

                l.checked_shr(r as u8 as u32)
                    .unwrap_or(if l < 0 { -1 } else { 0 })
            },
            Self::And(ref lhs, ref rhs) => {
                lhs.value(walker, symbols)? & rhs.value(walker, symbols)?
            },
            Self::Or(ref lhs, ref rhs) => {
                lhs.value(walker, symbols)? | rhs.value(walker, symbols)?
            },
            Self::Xor(ref lhs, ref rhs) => {
                lhs.value(walker, symbols)? ^ rhs.value(walker, symbols)?
            },
            Self::Div(ref lhs, ref rhs) => {
                lhs.value(walker, symbols)?
                    .wrapping_div(rhs.value(walker, symbols)?)
            },
            Self::Minus(ref operand) => -operand.value(walker, symbols)?,
            Self::Not(ref operand) => !operand.value(walker, symbols)?,
        })
    }

    pub fn from_xml(input: xml::Node) -> Result<Self, DeserialiseError> {
        Ok(match input.tag_name().name() {
            "tokenfield" => Self::TokenField {
                big_endian: input.attribute_bool("bigendian")?,
                sign_bit: input.attribute_bool("signbit")?,
                bit_start: input.attribute_int("bitstart")?,
                bit_end: input.attribute_int("bitend")?,
                byte_start: input.attribute_int("bytestart")?,
                byte_end: input.attribute_int("byteend")?,
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
            "plus_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Plus(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing lhs of binary expression"))?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing rhs of binary expression"))?)?),
                )
            },
            "sub_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Sub(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing lhs of binary expression"))?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing rhs of binary expression"))?)?),
                )
            },
            "mult_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Mult(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing lhs of binary expression"))?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing rhs of binary expression"))?)?),
                )
            },
            "lshift_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::LeftShift(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing lhs of binary expression"))?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing rhs of binary expression"))?)?),
                )
            },
            "rshift_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::RightShift(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing lhs of binary expression"))?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing rhs of binary expression"))?)?),
                )
            },
            "and_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::And(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing lhs of binary expression"))?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing rhs of binary expression"))?)?),
                )
            },
            "or_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Or(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing lhs of binary expression"))?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing rhs of binary expression"))?)?),
                )
            },
            "xor_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Xor(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing lhs of binary expression"))?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing rhs of binary expression"))?)?),
                )
            },
            "div_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Div(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing lhs of binary expression"))?)?),
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing rhs of binary expression"))?)?),
                )
            },
            "minus_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Minus(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing operand of unary expression"))?)?),
                )
            },
            "not_exp" => {
                let mut children = input.children().filter(xml::Node::is_element);
                Self::Not(
                    Box::new(Self::from_xml(children.next().ok_or_else(|| DeserialiseError::Invariant("missing operand of unary expression"))?)?),
                )
            },
            name => {
                return Err(DeserialiseError::TagUnexpected(name.to_owned()))
            },
        })
    }
}
