use std::io::{self, Read, Write};
use std::vec::Vec;

use thiserror::Error;

pub mod sla;

pub const ATTRIB_UNKNOWN: AttributeId = AttributeId::new("XMLunknown", 151);
pub const ATTRIB_CONTENT: AttributeId = AttributeId::new("XMLcontent", 1);
pub const ATTRIB_ALIGN: AttributeId = AttributeId::new("align", 2);
pub const ATTRIB_BIGENDIAN: AttributeId = AttributeId::new("bigendian", 3);
pub const ATTRIB_CONSTRUCTOR: AttributeId = AttributeId::new("constructor", 4);
pub const ATTRIB_DESTRUCTOR: AttributeId = AttributeId::new("destructor", 5);
pub const ATTRIB_EXTRAPOP: AttributeId = AttributeId::new("extrapop", 6);
pub const ATTRIB_FORMAT: AttributeId = AttributeId::new("format", 7);
pub const ATTRIB_HIDDENRETPARM: AttributeId = AttributeId::new("hiddenretparm", 8);
pub const ATTRIB_ID: AttributeId = AttributeId::new("id", 9);
pub const ATTRIB_INDEX: AttributeId = AttributeId::new("index", 10);
pub const ATTRIB_INDIRECTSTORAGE: AttributeId = AttributeId::new("indirectstorage", 11);
pub const ATTRIB_METATYPE: AttributeId = AttributeId::new("metatype", 12);
pub const ATTRIB_MODEL: AttributeId = AttributeId::new("model", 13);
pub const ATTRIB_NAME: AttributeId = AttributeId::new("name", 14);
pub const ATTRIB_NAMELOCK: AttributeId = AttributeId::new("namelock", 15);
pub const ATTRIB_OFFSET: AttributeId = AttributeId::new("offset", 16);
pub const ATTRIB_READONLY: AttributeId = AttributeId::new("readonly", 17);
pub const ATTRIB_REF: AttributeId = AttributeId::new("ref", 18);
pub const ATTRIB_SIZE: AttributeId = AttributeId::new("size", 19);
pub const ATTRIB_SPACE: AttributeId = AttributeId::new("space", 20);
pub const ATTRIB_THISPTR: AttributeId = AttributeId::new("thisptr", 21);
pub const ATTRIB_TYPE: AttributeId = AttributeId::new("type", 22);
pub const ATTRIB_TYPELOCK: AttributeId = AttributeId::new("typelock", 23);
pub const ATTRIB_VAL: AttributeId = AttributeId::new("val", 24);
pub const ATTRIB_VALUE: AttributeId = AttributeId::new("value", 25);
pub const ATTRIB_WORDSIZE: AttributeId = AttributeId::new("wordsize", 26);
pub const ATTRIB_STORAGE: AttributeId = AttributeId::new("storage", 149);
pub const ATTRIB_STACKSPILL: AttributeId = AttributeId::new("stackspill", 150);

pub const ELEM_UNKNOWN_NAME: &'static str = "XMLunknown";
pub const ELEM_UNKNOWN_ID: u32 = 287;
pub const ELEM_UNKNOWN: ElementId = ElementId::new(ELEM_UNKNOWN_NAME, ELEM_UNKNOWN_ID);
pub const ELEM_DATA_NAME: &'static str = "data";
pub const ELEM_DATA_ID: u32 = 1;
pub const ELEM_DATA: ElementId = ElementId::new(ELEM_DATA_NAME, ELEM_DATA_ID);
pub const ELEM_INPUT_NAME: &'static str = "input";
pub const ELEM_INPUT_ID: u32 = 2;
pub const ELEM_INPUT: ElementId = ElementId::new(ELEM_INPUT_NAME, ELEM_INPUT_ID);
pub const ELEM_OFF_NAME: &'static str = "off";
pub const ELEM_OFF_ID: u32 = 3;
pub const ELEM_OFF: ElementId = ElementId::new(ELEM_OFF_NAME, ELEM_OFF_ID);
pub const ELEM_OUTPUT_NAME: &'static str = "output";
pub const ELEM_OUTPUT_ID: u32 = 4;
pub const ELEM_OUTPUT: ElementId = ElementId::new(ELEM_OUTPUT_NAME, ELEM_OUTPUT_ID);
pub const ELEM_RETURNADDRESS_NAME: &'static str = "returnaddress";
pub const ELEM_RETURNADDRESS_ID: u32 = 5;
pub const ELEM_RETURNADDRESS: ElementId =
    ElementId::new(ELEM_RETURNADDRESS_NAME, ELEM_RETURNADDRESS_ID);
pub const ELEM_SYMBOL_NAME: &'static str = "symbol";
pub const ELEM_SYMBOL_ID: u32 = 6;
pub const ELEM_SYMBOL: ElementId = ElementId::new(ELEM_SYMBOL_NAME, ELEM_SYMBOL_ID);
pub const ELEM_TARGET_NAME: &'static str = "target";
pub const ELEM_TARGET_ID: u32 = 7;
pub const ELEM_TARGET: ElementId = ElementId::new(ELEM_TARGET_NAME, ELEM_TARGET_ID);
pub const ELEM_VAL_NAME: &'static str = "val";
pub const ELEM_VAL_ID: u32 = 8;
pub const ELEM_VAL: ElementId = ElementId::new(ELEM_VAL_NAME, ELEM_VAL_ID);
pub const ELEM_VALUE_NAME: &'static str = "value";
pub const ELEM_VALUE_ID: u32 = 9;
pub const ELEM_VALUE: ElementId = ElementId::new(ELEM_VALUE_NAME, ELEM_VALUE_ID);
pub const ELEM_VOID_NAME: &'static str = "void";
pub const ELEM_VOID_ID: u32 = 10;
pub const ELEM_VOID: ElementId = ElementId::new(ELEM_VOID_NAME, ELEM_VOID_ID);

#[derive(Debug, Error)]
pub enum MarshalError {
    #[error("cannot decode: {0}")]
    Decoder(#[from] DecoderError),
    #[error("cannot encode: {0}")]
    Encoder(#[from] EncoderError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, Error)]
pub enum EncoderError {
    #[error(transparent)]
    Compress(io::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, Error)]
pub enum DecoderError {
    #[error("address space unsupported")]
    AddressSpaceUnsupported,
    #[error("attribute `{0}` not found")]
    AttributeNotFound(&'static str),
    #[error("stream is corrupt")]
    CorruptStream,
    #[error(transparent)]
    Decompress(io::Error),
    #[error("element `{0}` not found")]
    ElementNotFound(&'static str),
    #[error("element not closed")]
    ElementNotClosed,
    #[error("invalid header format")]
    HeaderFormat,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("position out of bounds")]
    PositionOutOfBounds,
    #[error("string cannot be decoded: {0}")]
    StringEncoding(#[from] std::str::Utf8Error),
    #[error("unexpected attribute; expected: {0}")]
    UnexpectedAttribute(&'static str),
    #[error("unexpected end of stream")]
    UnexpectedEndOfStream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttributeId {
    name: &'static str,
    id: u32,
}

impl AttributeId {
    pub const fn new(name: &'static str, id: u32) -> Self {
        Self { name, id }
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub const fn id(&self) -> u32 {
        self.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementId {
    name: &'static str,
    id: u32,
}

impl ElementId {
    pub const fn new(name: &'static str, id: u32) -> Self {
        Self { name, id }
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub const fn id(&self) -> u32 {
        self.id
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AddressSpaceRef {
    Fspec,
    Iop,
    Join,
    Spacebase(bool),
    Other(u32),
}

impl AddressSpaceRef {
    pub fn index(&self) -> Option<usize> {
        let Self::Other(index) = self else {
            return None;
        };
        Some(*index as _)
    }

    pub fn is_formal_stack_space(&self) -> bool {
        *self == Self::Spacebase(true)
    }
}

pub trait Decoder {
    /// Prepare to decode a given stream
    fn ingest_stream<R: Read>(&mut self, stream: R) -> Result<(), MarshalError>;

    /// Peek at the next child element without traversing into it
    fn peek_element(&mut self) -> Result<u32, MarshalError>;

    /// Open the next child element
    fn open_element(&mut self) -> Result<u32, MarshalError>;

    /// Open the next child element, which must be of a specific type
    fn open_element_with_id(&mut self, elem_id: &ElementId) -> Result<u32, MarshalError>;

    /// Close the current element
    fn close_element(&mut self, id: u32) -> Result<(), MarshalError>;

    /// Close the current element, skipping any child elements
    fn close_element_skipping(&mut self, id: u32) -> Result<(), MarshalError>;

    /// Get the next attribute id for the current element
    fn get_next_attribute_id(&mut self) -> Result<u32, MarshalError>;

    /// Get the id for the current attribute, assuming it is indexed
    fn get_indexed_attribute_id(&mut self, attrib_id: &AttributeId) -> Result<u32, MarshalError>;

    /// Reset attribute traversal for the current element
    fn rewind_attributes(&mut self) -> Result<(), MarshalError>;

    /// Parse the current attribute as a boolean value
    fn read_bool(&mut self) -> Result<bool, MarshalError>;

    /// Find and parse a specific attribute as a boolean value
    fn read_bool_with_id(&mut self, attrib_id: &AttributeId) -> Result<bool, MarshalError>;

    /// Find and parse a specific attribute as a boolean value
    fn try_read_bool_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<Option<bool>, MarshalError>;

    /// Parse the current attribute as a signed integer value
    fn read_signed_integer(&mut self) -> Result<i64, MarshalError>;

    /// Find and parse a specific attribute as a signed integer
    fn read_signed_integer_with_id(&mut self, attrib_id: &AttributeId)
        -> Result<i64, MarshalError>;

    /// Find and parse a specific attribute as a signed integer
    fn try_read_signed_integer_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<Option<i64>, MarshalError>;

    /// Parse the current attribute as either a signed integer or a string
    fn read_signed_integer_expect_string(
        &mut self,
        expect: &str,
        expect_val: i64,
    ) -> Result<i64, MarshalError>;

    /// Find and parse a specific attribute as either a signed integer or a string
    fn read_signed_integer_expect_string_with_id(
        &mut self,
        attrib_id: &AttributeId,
        expect: &str,
        expect_val: i64,
    ) -> Result<i64, MarshalError>;

    /// Parse the current attribute as an unsigned integer value
    fn read_unsigned_integer(&mut self) -> Result<u64, MarshalError>;

    /// Find and parse a specific attribute as an unsigned integer
    fn read_unsigned_integer_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<u64, MarshalError>;

    /// Find and parse a specific attribute as an unsigned integer
    fn try_read_unsigned_integer_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<Option<u64>, MarshalError>;

    /// Parse the current attribute as a string
    fn read_string(&mut self) -> Result<String, MarshalError>;

    /// Find and parse a specific attribute as a string
    fn read_string_with_id(&mut self, attrib_id: &AttributeId) -> Result<String, MarshalError>;

    /// Find and parse a specific attribute as a string
    fn try_read_string_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<Option<String>, MarshalError>;

    /// Parse the current attribute as an address space
    fn read_space(&mut self) -> Result<AddressSpaceRef, MarshalError>;

    /// Find and parse a specific attribute as an address space
    fn read_space_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<AddressSpaceRef, MarshalError>;

    /// Skip parsing of the next element
    fn skip_element(&mut self) -> Result<(), MarshalError> {
        let elem_id = self.open_element()?;
        self.close_element_skipping(elem_id)
    }
}

pub trait Encoder {
    /// Begin a new element in the encoding
    fn open_element(&mut self, elem_id: &ElementId) -> Result<(), MarshalError>;

    /// End the current element in the encoding
    fn close_element(&mut self, elem_id: &ElementId) -> Result<(), MarshalError>;

    /// Write an annotated boolean value
    fn write_bool(&mut self, attrib_id: &AttributeId, val: bool) -> Result<(), MarshalError>;

    /// Write an annotated signed integer value
    fn write_signed_integer(
        &mut self,
        attrib_id: &AttributeId,
        val: i64,
    ) -> Result<(), MarshalError>;

    /// Write an annotated unsigned integer value
    fn write_unsigned_integer(
        &mut self,
        attrib_id: &AttributeId,
        val: u64,
    ) -> Result<(), MarshalError>;

    /// Write an annotated string
    fn write_string(&mut self, attrib_id: &AttributeId, val: &str) -> Result<(), MarshalError>;

    /// Write an annotated string, using an indexed attribute
    fn write_string_indexed(
        &mut self,
        attrib_id: &AttributeId,
        index: u32,
        val: &str,
    ) -> Result<(), MarshalError>;

    /// Write an address space reference
    fn write_space(
        &mut self,
        attrib_id: &AttributeId,
        spc: AddressSpaceRef,
    ) -> Result<(), MarshalError>;
}

mod packed_format {
    pub const HEADER_MASK: u8 = 0xc0;
    pub const ELEMENT_START: u8 = 0x40;
    pub const ELEMENT_END: u8 = 0x80;
    pub const ATTRIBUTE: u8 = 0xc0;
    pub const HEADEREXTEND_MASK: u8 = 0x20;
    pub const ELEMENTID_MASK: u8 = 0x1f;
    pub const RAWDATA_MASK: u8 = 0x7f;
    pub const RAWDATA_BITSPERBYTE: u32 = 7;
    pub const RAWDATA_MARKER: u8 = 0x80;
    pub const TYPECODE_SHIFT: u32 = 4;
    pub const LENGTHCODE_MASK: u8 = 0x0f;
    pub const TYPECODE_BOOLEAN: u8 = 1;
    pub const TYPECODE_SIGNEDINT_POSITIVE: u8 = 2;
    pub const TYPECODE_SIGNEDINT_NEGATIVE: u8 = 3;
    pub const TYPECODE_UNSIGNEDINT: u8 = 4;
    pub const TYPECODE_ADDRESSSPACE: u8 = 5;
    pub const TYPECODE_SPECIALSPACE: u8 = 6;
    pub const TYPECODE_STRING: u8 = 7;
    pub const SPECIALSPACE_STACK: u32 = 0;
    pub const SPECIALSPACE_JOIN: u32 = 1;
    pub const SPECIALSPACE_FSPEC: u32 = 2;
    pub const SPECIALSPACE_IOP: u32 = 3;
    pub const SPECIALSPACE_SPACEBASE: u32 = 4;
}

const BUFFER_SIZE: usize = 1024;

pub struct PackedDecoder {
    in_stream: Vec<ByteChunk>,
    start_pos: Position,
    cur_pos: Position,
    end_pos: Position,
    attribute_read: bool,
}

#[derive(Debug, Clone)]
struct ByteChunk {
    data: Vec<u8>,
}

#[derive(Debug, Default, Clone, Copy)]
struct Position {
    chunk_index: usize,
    byte_index: usize,
}

impl Position {
    fn new() -> Self {
        Self {
            chunk_index: 0,
            byte_index: 0,
        }
    }
}

impl PackedDecoder {
    pub fn new() -> Self {
        Self {
            in_stream: Vec::new(),
            start_pos: Position::new(),
            cur_pos: Position::new(),
            end_pos: Position::new(),
            attribute_read: true,
        }
    }

    fn get_byte(&self, pos: &Position) -> Result<u8, MarshalError> {
        if pos.chunk_index >= self.in_stream.len() {
            return Err(DecoderError::PositionOutOfBounds.into());
        }
        let chunk = &self.in_stream[pos.chunk_index];
        if pos.byte_index >= chunk.data.len() {
            return Err(DecoderError::PositionOutOfBounds.into());
        }
        Ok(chunk.data[pos.byte_index])
    }

    fn get_byte_plus_1(&self, pos: &Position) -> Result<u8, MarshalError> {
        let mut next_pos = pos.clone();
        if next_pos.byte_index + 1 < self.in_stream[next_pos.chunk_index].data.len() {
            next_pos.byte_index += 1;
        } else {
            next_pos.chunk_index += 1;
            next_pos.byte_index = 0;
            if next_pos.chunk_index >= self.in_stream.len() {
                return Err(DecoderError::UnexpectedEndOfStream.into());
            }
        }
        self.get_byte(&next_pos)
    }

    fn get_next_byte(&mut self) -> Result<u8, MarshalError> {
        let byte = self.get_byte(&self.cur_pos)?;
        self.advance_position(1)?;
        Ok(byte)
    }

    fn get_next_byte_from_end(&mut self) -> Result<u8, MarshalError> {
        let byte = self.get_byte(&self.end_pos)?;
        self.advance_position_from_end(1)?;
        Ok(byte)
    }

    fn advance_position(&mut self, skip: usize) -> Result<(), MarshalError> {
        let mut remaining = skip;
        while remaining > 0 {
            if self.cur_pos.chunk_index >= self.in_stream.len() {
                return Err(DecoderError::UnexpectedEndOfStream.into());
            }

            let chunk = &self.in_stream[self.cur_pos.chunk_index];
            let bytes_left_in_chunk = chunk.data.len() - self.cur_pos.byte_index;

            if bytes_left_in_chunk > remaining {
                self.cur_pos.byte_index += remaining;
                break;
            } else {
                remaining -= bytes_left_in_chunk;
                self.cur_pos.chunk_index += 1;
                self.cur_pos.byte_index = 0;
            }
        }
        Ok(())
    }

    fn advance_position_from_end(&mut self, skip: usize) -> Result<(), MarshalError> {
        let mut remaining = skip;
        while remaining > 0 {
            if self.end_pos.chunk_index >= self.in_stream.len() {
                return Err(DecoderError::UnexpectedEndOfStream.into());
            }

            let chunk = &self.in_stream[self.end_pos.chunk_index];
            let bytes_left_in_chunk = chunk.data.len() - self.end_pos.byte_index;

            if bytes_left_in_chunk > remaining {
                self.end_pos.byte_index += remaining;
                break;
            } else {
                remaining -= bytes_left_in_chunk;
                self.end_pos.chunk_index += 1;
                self.end_pos.byte_index = 0;
            }
        }
        Ok(())
    }

    fn read_integer(&mut self, len: usize) -> Result<u64, MarshalError> {
        let mut res: u64 = 0;
        for _ in 0..len {
            res <<= packed_format::RAWDATA_BITSPERBYTE;
            let byte = self.get_next_byte()?;
            res |= (byte & packed_format::RAWDATA_MASK) as u64;
        }
        Ok(res)
    }

    fn read_length_code(&self, type_byte: u8) -> u32 {
        (type_byte & packed_format::LENGTHCODE_MASK) as u32
    }

    fn find_matching_attribute(&mut self, attrib_id: &AttributeId) -> Result<(), MarshalError> {
        self.cur_pos = self.start_pos.clone();
        loop {
            let header1 = self.get_byte(&self.cur_pos)?;
            if (header1 & packed_format::HEADER_MASK) != packed_format::ATTRIBUTE {
                break;
            }

            let mut id = (header1 & packed_format::ELEMENTID_MASK) as u32;
            if (header1 & packed_format::HEADEREXTEND_MASK) != 0 {
                id <<= packed_format::RAWDATA_BITSPERBYTE;
                id |= (self.get_byte_plus_1(&self.cur_pos)? & packed_format::RAWDATA_MASK) as u32;
            }

            if attrib_id.id() == id {
                return Ok(());
            }

            self.skip_attribute()?;
        }

        Err(DecoderError::AttributeNotFound(attrib_id.name()).into())
    }

    fn skip_attribute(&mut self) -> Result<(), MarshalError> {
        let header1 = self.get_next_byte()?;
        if (header1 & packed_format::HEADEREXTEND_MASK) != 0 {
            self.get_next_byte()?;
        }

        let type_byte = self.get_next_byte()?;
        self.skip_attribute_remaining(type_byte)
    }

    fn skip_attribute_remaining(&mut self, type_byte: u8) -> Result<(), MarshalError> {
        let attrib_type = type_byte >> packed_format::TYPECODE_SHIFT;

        if attrib_type == packed_format::TYPECODE_BOOLEAN
            || attrib_type == packed_format::TYPECODE_SPECIALSPACE
        {
            return Ok(());
        }

        let mut length = self.read_length_code(type_byte) as usize;

        if attrib_type == packed_format::TYPECODE_STRING {
            length = self.read_integer(length)? as usize;
        }

        self.advance_position(length)
    }

    #[allow(unused)]
    fn allocate_next_input_buffer(&mut self, pad: usize) -> Vec<u8> {
        let buf = vec![0u8; BUFFER_SIZE + pad];
        self.in_stream.push(ByteChunk { data: buf.clone() });
        buf
    }

    fn end_ingest(&mut self, buf_pos: usize) -> Result<(), MarshalError> {
        self.end_pos.chunk_index = 0;
        self.end_pos.byte_index = 0;

        if !self.in_stream.is_empty() {
            if buf_pos == BUFFER_SIZE {
                self.in_stream.push(ByteChunk {
                    data: vec![packed_format::ELEMENT_END],
                });
            } else {
                let last_chunk = self.in_stream.last_mut().unwrap();
                if buf_pos < last_chunk.data.len() {
                    last_chunk.data[buf_pos] = packed_format::ELEMENT_END;
                }
            }
        }
        Ok(())
    }
}

impl Decoder for PackedDecoder {
    fn ingest_stream<R: Read>(&mut self, mut stream: R) -> Result<(), MarshalError> {
        let mut buffer = [0u8; BUFFER_SIZE];
        let mut total_read = 0;

        loop {
            let n = stream.read(&mut buffer).map_err(DecoderError::from)?;
            if n == 0 {
                break;
            }
            let chunk = ByteChunk {
                data: buffer[..n].to_vec(),
            };
            self.in_stream.push(chunk);
            total_read = n;
        }

        self.end_ingest(total_read)
    }

    fn peek_element(&mut self) -> Result<u32, MarshalError> {
        let header1 = self.get_byte(&self.end_pos)?;
        if (header1 & packed_format::HEADER_MASK) != packed_format::ELEMENT_START {
            return Ok(0);
        }

        let mut id = (header1 & packed_format::ELEMENTID_MASK) as u32;
        if (header1 & packed_format::HEADEREXTEND_MASK) != 0 {
            id <<= packed_format::RAWDATA_BITSPERBYTE;
            id |= (self.get_byte_plus_1(&self.end_pos)? & packed_format::RAWDATA_MASK) as u32;
        }

        Ok(id)
    }

    fn open_element(&mut self) -> Result<u32, MarshalError> {
        let header1 = self.get_byte(&self.end_pos)?;
        if (header1 & packed_format::HEADER_MASK) != packed_format::ELEMENT_START {
            return Ok(0);
        }

        self.get_next_byte_from_end()?;
        let mut id = (header1 & packed_format::ELEMENTID_MASK) as u32;
        if (header1 & packed_format::HEADEREXTEND_MASK) != 0 {
            id <<= packed_format::RAWDATA_BITSPERBYTE;
            id |= (self.get_next_byte_from_end()? & packed_format::RAWDATA_MASK) as u32;
        }

        self.start_pos = self.end_pos.clone();
        self.cur_pos = self.end_pos.clone();

        let mut header1 = self.get_byte(&self.cur_pos)?;
        while (header1 & packed_format::HEADER_MASK) == packed_format::ATTRIBUTE {
            self.skip_attribute()?;
            header1 = self.get_byte(&self.cur_pos)?;
        }

        self.end_pos = self.cur_pos.clone();
        self.cur_pos = self.start_pos.clone();
        self.attribute_read = true; // "Last attribute was read" is vacuously true

        Ok(id)
    }

    fn open_element_with_id(&mut self, elem_id: &ElementId) -> Result<u32, MarshalError> {
        let id = self.open_element()?;
        if id != elem_id.id() {
            return Err(DecoderError::ElementNotFound(elem_id.name()).into());
        }
        Ok(id)
    }

    fn close_element(&mut self, id: u32) -> Result<(), MarshalError> {
        let header1 = self.get_next_byte_from_end()?;
        if (header1 & packed_format::HEADER_MASK) != packed_format::ELEMENT_END {
            return Err(DecoderError::ElementNotClosed.into());
        }

        let mut close_id = (header1 & packed_format::ELEMENTID_MASK) as u32;
        if (header1 & packed_format::HEADEREXTEND_MASK) != 0 {
            close_id <<= packed_format::RAWDATA_BITSPERBYTE;
            close_id |= (self.get_next_byte_from_end()? & packed_format::RAWDATA_MASK) as u32;
        }

        if id != close_id {
            return Err(DecoderError::ElementNotClosed.into());
        }

        Ok(())
    }

    fn close_element_skipping(&mut self, id: u32) -> Result<(), MarshalError> {
        let mut id_stack = vec![id];

        while !id_stack.is_empty() {
            let header1 = self.get_byte(&self.end_pos)? & packed_format::HEADER_MASK;

            if header1 == packed_format::ELEMENT_END {
                self.close_element(id_stack.pop().unwrap())?;
            } else if header1 == packed_format::ELEMENT_START {
                id_stack.push(self.open_element()?);
            } else {
                return Err(DecoderError::CorruptStream.into());
            }
        }

        Ok(())
    }

    fn rewind_attributes(&mut self) -> Result<(), MarshalError> {
        self.cur_pos = self.start_pos.clone();
        self.attribute_read = true;
        Ok(())
    }

    fn get_next_attribute_id(&mut self) -> Result<u32, MarshalError> {
        if !self.attribute_read {
            self.skip_attribute()?;
        }

        let header1 = self.get_byte(&self.cur_pos)?;
        if (header1 & packed_format::HEADER_MASK) != packed_format::ATTRIBUTE {
            return Ok(0);
        }

        let mut id = (header1 & packed_format::ELEMENTID_MASK) as u32;
        if (header1 & packed_format::HEADEREXTEND_MASK) != 0 {
            id <<= packed_format::RAWDATA_BITSPERBYTE;
            id |= (self.get_byte_plus_1(&self.cur_pos)? & packed_format::RAWDATA_MASK) as u32;
        }

        self.attribute_read = false;
        Ok(id)
    }

    fn get_indexed_attribute_id(&mut self, _attrib_id: &AttributeId) -> Result<u32, MarshalError> {
        Ok(ATTRIB_UNKNOWN.id())
    }

    fn read_bool(&mut self) -> Result<bool, MarshalError> {
        let header1 = self.get_next_byte()?;
        if (header1 & packed_format::HEADEREXTEND_MASK) != 0 {
            self.get_next_byte()?;
        }

        let type_byte = self.get_next_byte()?;
        self.attribute_read = true;

        if (type_byte >> packed_format::TYPECODE_SHIFT) != packed_format::TYPECODE_BOOLEAN {
            return Err(DecoderError::UnexpectedAttribute("boolean").into());
        }

        Ok((type_byte & packed_format::LENGTHCODE_MASK) != 0)
    }

    fn read_bool_with_id(&mut self, attrib_id: &AttributeId) -> Result<bool, MarshalError> {
        self.find_matching_attribute(attrib_id)?;
        let res = self.read_bool()?;
        self.cur_pos = self.start_pos.clone();
        Ok(res)
    }

    fn try_read_bool_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<Option<bool>, MarshalError> {
        if self.find_matching_attribute(attrib_id).is_err() {
            return Ok(None);
        }
        let res = self.read_bool()?;
        self.cur_pos = self.start_pos;
        Ok(Some(res))
    }

    fn read_signed_integer(&mut self) -> Result<i64, MarshalError> {
        let header1 = self.get_next_byte()?;
        if (header1 & packed_format::HEADEREXTEND_MASK) != 0 {
            self.get_next_byte()?;
        }

        let type_byte = self.get_next_byte()?;
        let type_code = type_byte >> packed_format::TYPECODE_SHIFT;

        let res = if type_code == packed_format::TYPECODE_SIGNEDINT_POSITIVE {
            self.read_integer(self.read_length_code(type_byte) as usize)? as i64
        } else if type_code == packed_format::TYPECODE_SIGNEDINT_NEGATIVE {
            -(self.read_integer(self.read_length_code(type_byte) as usize)? as i64)
        } else {
            self.skip_attribute_remaining(type_byte)?;
            self.attribute_read = true;
            return Err(DecoderError::UnexpectedAttribute("signed integer").into());
        };

        self.attribute_read = true;
        Ok(res)
    }

    fn read_signed_integer_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<i64, MarshalError> {
        self.find_matching_attribute(attrib_id)?;
        let res = self.read_signed_integer()?;
        self.cur_pos = self.start_pos;
        Ok(res)
    }

    fn try_read_signed_integer_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<Option<i64>, MarshalError> {
        if self.find_matching_attribute(attrib_id).is_err() {
            return Ok(None);
        }
        let res = self.read_signed_integer()?;
        self.cur_pos = self.start_pos;
        Ok(Some(res))
    }

    fn read_signed_integer_expect_string(
        &mut self,
        expect: &str,
        expect_val: i64,
    ) -> Result<i64, MarshalError> {
        let tmp_pos = self.cur_pos.clone();
        let header1 = self.get_next_byte()?;
        if (header1 & packed_format::HEADEREXTEND_MASK) != 0 {
            self.get_next_byte()?;
        }

        let type_byte = self.get_next_byte()?;
        let type_code = type_byte >> packed_format::TYPECODE_SHIFT;

        if type_code == packed_format::TYPECODE_STRING {
            self.cur_pos = tmp_pos;
            let val = self.read_string()?;

            if val != expect {
                return Err(DecoderError::UnexpectedAttribute("string").into());
            }

            Ok(expect_val)
        } else {
            self.cur_pos = tmp_pos;
            self.read_signed_integer()
        }
    }

    fn read_signed_integer_expect_string_with_id(
        &mut self,
        attrib_id: &AttributeId,
        expect: &str,
        expect_val: i64,
    ) -> Result<i64, MarshalError> {
        self.find_matching_attribute(attrib_id)?;
        let res = self.read_signed_integer_expect_string(expect, expect_val)?;
        self.cur_pos = self.start_pos.clone();
        Ok(res)
    }

    fn read_unsigned_integer(&mut self) -> Result<u64, MarshalError> {
        let header1 = self.get_next_byte()?;
        if (header1 & packed_format::HEADEREXTEND_MASK) != 0 {
            self.get_next_byte()?;
        }

        let type_byte = self.get_next_byte()?;
        let type_code = type_byte >> packed_format::TYPECODE_SHIFT;

        if type_code != packed_format::TYPECODE_UNSIGNEDINT {
            self.skip_attribute_remaining(type_byte)?;
            self.attribute_read = true;
            return Err(DecoderError::UnexpectedAttribute("unsigned integer").into());
        }

        let res = self.read_integer(self.read_length_code(type_byte) as usize)?;
        self.attribute_read = true;
        Ok(res)
    }

    fn read_unsigned_integer_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<u64, MarshalError> {
        self.find_matching_attribute(attrib_id)?;
        let res = self.read_unsigned_integer()?;
        self.cur_pos = self.start_pos;
        Ok(res)
    }

    fn try_read_unsigned_integer_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<Option<u64>, MarshalError> {
        if self.find_matching_attribute(attrib_id).is_err() {
            return Ok(None);
        }
        let res = self.read_unsigned_integer()?;
        self.cur_pos = self.start_pos;
        Ok(Some(res))
    }

    fn read_string(&mut self) -> Result<String, MarshalError> {
        let header1 = self.get_next_byte()?;
        if (header1 & packed_format::HEADEREXTEND_MASK) != 0 {
            self.get_next_byte()?;
        }

        let type_byte = self.get_next_byte()?;
        let type_code = type_byte >> packed_format::TYPECODE_SHIFT;

        if type_code != packed_format::TYPECODE_STRING {
            self.skip_attribute_remaining(type_byte)?;
            self.attribute_read = true;
            return Err(DecoderError::UnexpectedAttribute("string").into());
        }

        let length = self.read_integer(self.read_length_code(type_byte) as usize)? as usize;
        self.attribute_read = true;

        let mut result = String::with_capacity(length);
        let mut remaining = length;

        while remaining > 0 {
            if self.cur_pos.chunk_index >= self.in_stream.len() {
                return Err(DecoderError::UnexpectedEndOfStream.into());
            }

            let chunk = &self.in_stream[self.cur_pos.chunk_index];
            let bytes_available = chunk.data.len() - self.cur_pos.byte_index;
            let bytes_to_read = remaining.min(bytes_available);

            let chunk_str = std::str::from_utf8(
                &chunk.data[self.cur_pos.byte_index..self.cur_pos.byte_index + bytes_to_read],
            )
            .map_err(DecoderError::StringEncoding)?;

            result.push_str(chunk_str);
            remaining -= bytes_to_read;

            self.advance_position(bytes_to_read)?;
        }

        Ok(result)
    }

    fn read_string_with_id(&mut self, attrib_id: &AttributeId) -> Result<String, MarshalError> {
        self.find_matching_attribute(attrib_id)?;
        let res = self.read_string()?;
        self.cur_pos = self.start_pos;
        Ok(res)
    }

    fn try_read_string_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<Option<String>, MarshalError> {
        if self.find_matching_attribute(attrib_id).is_err() {
            return Ok(None);
        }
        let res = self.read_string()?;
        self.cur_pos = self.start_pos;
        Ok(Some(res))
    }

    fn read_space(&mut self) -> Result<AddressSpaceRef, MarshalError> {
        let header1 = self.get_next_byte()?;
        if (header1 & packed_format::HEADEREXTEND_MASK) != 0 {
            self.get_next_byte()?;
        }

        let type_byte = self.get_next_byte()?;
        let type_code = type_byte >> packed_format::TYPECODE_SHIFT;

        let spc = if type_code == packed_format::TYPECODE_ADDRESSSPACE {
            let index = self.read_integer(self.read_length_code(type_byte) as usize)? as u32;
            AddressSpaceRef::Other(index)
        } else if type_code == packed_format::TYPECODE_SPECIALSPACE {
            let special_code = self.read_length_code(type_byte);
            match special_code {
                packed_format::SPECIALSPACE_STACK => AddressSpaceRef::Spacebase(true),
                packed_format::SPECIALSPACE_JOIN => AddressSpaceRef::Join,
                _ => {
                    return Err(DecoderError::AddressSpaceUnsupported.into());
                }
            }
        } else {
            self.skip_attribute_remaining(type_byte)?;
            self.attribute_read = true;
            return Err(DecoderError::UnexpectedAttribute("space").into());
        };

        self.attribute_read = true;
        Ok(spc)
    }

    fn read_space_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<AddressSpaceRef, MarshalError> {
        self.find_matching_attribute(attrib_id)?;
        let res = self.read_space()?;
        self.cur_pos = self.start_pos.clone();
        Ok(res)
    }
}

pub struct PackedEncoder<W: Write> {
    out_stream: W,
}

impl<W: Write> PackedEncoder<W> {
    pub fn new(out_stream: W) -> Self {
        Self { out_stream }
    }

    fn write_header(&mut self, header: u8, id: u32) -> Result<(), MarshalError> {
        if id > 0x1f {
            let out_header = header
                | packed_format::HEADEREXTEND_MASK
                | ((id >> packed_format::RAWDATA_BITSPERBYTE) as u8);
            let extend_byte =
                (id as u8 & packed_format::RAWDATA_MASK) | packed_format::RAWDATA_MARKER;

            self.out_stream
                .write_all(&[out_header, extend_byte])
                .map_err(EncoderError::from)?;
        } else {
            let out_header = header | (id as u8);
            self.out_stream
                .write_all(&[out_header])
                .map_err(EncoderError::from)?;
        }

        Ok(())
    }

    fn write_integer(&mut self, type_byte: u8, val: u64) -> Result<(), MarshalError> {
        let (len_code, shift_amount) = if val == 0 {
            (0, None)
        } else if val < 0x80 {
            (1, Some(0))
        } else if val < 0x4000 {
            (2, Some(packed_format::RAWDATA_BITSPERBYTE))
        } else if val < 0x200000 {
            (3, Some(2 * packed_format::RAWDATA_BITSPERBYTE))
        } else if val < 0x10000000 {
            (4, Some(3 * packed_format::RAWDATA_BITSPERBYTE))
        } else if val < 0x800000000 {
            (5, Some(4 * packed_format::RAWDATA_BITSPERBYTE))
        } else if val < 0x40000000000 {
            (6, Some(5 * packed_format::RAWDATA_BITSPERBYTE))
        } else if val < 0x2000000000000 {
            (7, Some(6 * packed_format::RAWDATA_BITSPERBYTE))
        } else if val < 0x100000000000000 {
            (8, Some(7 * packed_format::RAWDATA_BITSPERBYTE))
        } else if val < 0x8000000000000000 {
            (9, Some(8 * packed_format::RAWDATA_BITSPERBYTE))
        } else {
            (10, Some(9 * packed_format::RAWDATA_BITSPERBYTE))
        };

        let out_type_byte = type_byte | len_code;
        self.out_stream
            .write_all(&[out_type_byte])
            .map_err(EncoderError::from)?;

        if let Some(sa) = shift_amount {
            let mut remaining_shift = sa as i32;
            while remaining_shift >= 0 {
                let piece = ((val >> remaining_shift) & packed_format::RAWDATA_MASK as u64) as u8
                    | packed_format::RAWDATA_MARKER;
                self.out_stream
                    .write_all(&[piece])
                    .map_err(EncoderError::from)?;
                remaining_shift -= packed_format::RAWDATA_BITSPERBYTE as i32;
            }
        }

        Ok(())
    }
}

impl<W: Write> Encoder for PackedEncoder<W> {
    fn open_element(&mut self, elem_id: &ElementId) -> Result<(), MarshalError> {
        self.write_header(packed_format::ELEMENT_START, elem_id.id())
    }

    fn close_element(&mut self, elem_id: &ElementId) -> Result<(), MarshalError> {
        self.write_header(packed_format::ELEMENT_END, elem_id.id())
    }

    fn write_bool(&mut self, attrib_id: &AttributeId, val: bool) -> Result<(), MarshalError> {
        self.write_header(packed_format::ATTRIBUTE, attrib_id.id())?;

        let type_byte = if val {
            (packed_format::TYPECODE_BOOLEAN << packed_format::TYPECODE_SHIFT) | 1
        } else {
            packed_format::TYPECODE_BOOLEAN << packed_format::TYPECODE_SHIFT
        };

        self.out_stream
            .write_all(&[type_byte])
            .map_err(EncoderError::from)?;
        Ok(())
    }

    fn write_signed_integer(
        &mut self,
        attrib_id: &AttributeId,
        val: i64,
    ) -> Result<(), MarshalError> {
        self.write_header(packed_format::ATTRIBUTE, attrib_id.id())?;

        let (type_byte, num) = if val < 0 {
            (
                packed_format::TYPECODE_SIGNEDINT_NEGATIVE << packed_format::TYPECODE_SHIFT,
                (-val) as u64,
            )
        } else {
            (
                packed_format::TYPECODE_SIGNEDINT_POSITIVE << packed_format::TYPECODE_SHIFT,
                val as u64,
            )
        };

        self.write_integer(type_byte, num)
    }

    fn write_unsigned_integer(
        &mut self,
        attrib_id: &AttributeId,
        val: u64,
    ) -> Result<(), MarshalError> {
        self.write_header(packed_format::ATTRIBUTE, attrib_id.id())?;
        self.write_integer(
            packed_format::TYPECODE_UNSIGNEDINT << packed_format::TYPECODE_SHIFT,
            val,
        )
    }

    fn write_string(&mut self, attrib_id: &AttributeId, val: &str) -> Result<(), MarshalError> {
        let length = val.len() as u64;
        self.write_header(packed_format::ATTRIBUTE, attrib_id.id())?;
        self.write_integer(
            packed_format::TYPECODE_STRING << packed_format::TYPECODE_SHIFT,
            length,
        )?;
        self.out_stream
            .write_all(val.as_bytes())
            .map_err(EncoderError::from)?;
        Ok(())
    }

    fn write_string_indexed(
        &mut self,
        attrib_id: &AttributeId,
        index: u32,
        val: &str,
    ) -> Result<(), MarshalError> {
        let length = val.len() as u64;
        self.write_header(packed_format::ATTRIBUTE, attrib_id.id() + index)?;
        self.write_integer(
            packed_format::TYPECODE_STRING << packed_format::TYPECODE_SHIFT,
            length,
        )?;
        self.out_stream
            .write_all(val.as_bytes())
            .map_err(EncoderError::from)?;
        Ok(())
    }

    fn write_space(
        &mut self,
        attrib_id: &AttributeId,
        spc: AddressSpaceRef,
    ) -> Result<(), MarshalError> {
        self.write_header(packed_format::ATTRIBUTE, attrib_id.id())?;

        match spc {
            AddressSpaceRef::Fspec => {
                let type_byte = (packed_format::TYPECODE_SPECIALSPACE
                    << packed_format::TYPECODE_SHIFT)
                    | packed_format::SPECIALSPACE_FSPEC as u8;
                self.out_stream
                    .write_all(&[type_byte])
                    .map_err(EncoderError::from)?;
            }
            AddressSpaceRef::Iop => {
                let type_byte = (packed_format::TYPECODE_SPECIALSPACE
                    << packed_format::TYPECODE_SHIFT)
                    | packed_format::SPECIALSPACE_IOP as u8;
                self.out_stream
                    .write_all(&[type_byte])
                    .map_err(EncoderError::from)?;
            }
            AddressSpaceRef::Join => {
                let type_byte = (packed_format::TYPECODE_SPECIALSPACE
                    << packed_format::TYPECODE_SHIFT)
                    | packed_format::SPECIALSPACE_JOIN as u8;
                self.out_stream
                    .write_all(&[type_byte])
                    .map_err(EncoderError::from)?;
            }
            AddressSpaceRef::Spacebase(_) => {
                if spc.is_formal_stack_space() {
                    let type_byte = (packed_format::TYPECODE_SPECIALSPACE
                        << packed_format::TYPECODE_SHIFT)
                        | packed_format::SPECIALSPACE_STACK as u8;
                    self.out_stream
                        .write_all(&[type_byte])
                        .map_err(EncoderError::from)?;
                } else {
                    let type_byte = (packed_format::TYPECODE_SPECIALSPACE
                        << packed_format::TYPECODE_SHIFT)
                        | packed_format::SPECIALSPACE_SPACEBASE as u8;
                    self.out_stream
                        .write_all(&[type_byte])
                        .map_err(EncoderError::from)?;
                }
            }
            AddressSpaceRef::Other(index) => {
                let spc_id = index as u64;
                self.write_integer(
                    packed_format::TYPECODE_ADDRESSSPACE << packed_format::TYPECODE_SHIFT,
                    spc_id,
                )?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_packed_encoder_decoder_basic() -> Result<(), MarshalError> {
        let buffer = Vec::new();
        let mut encoder = PackedEncoder::new(buffer);

        encoder.open_element(&ELEM_DATA)?;
        encoder.write_string(&ATTRIB_NAME, "test_data")?;
        encoder.write_unsigned_integer(&ATTRIB_SIZE, 42)?;
        encoder.close_element(&ELEM_DATA)?;

        let encoded_data = encoder.out_stream;
        let mut decoder = PackedDecoder::new();
        decoder.ingest_stream(Cursor::new(encoded_data))?;

        let elem_id = decoder.open_element()?;
        assert_eq!(elem_id, ELEM_DATA.id());

        let size = decoder.read_unsigned_integer_with_id(&ATTRIB_SIZE)?;
        assert_eq!(size, 42);

        let name = decoder.read_string_with_id(&ATTRIB_NAME)?;
        assert_eq!(name, "test_data");

        decoder.close_element(elem_id)?;

        Ok(())
    }

    #[test]
    fn test_packed_encoder_decoder_nested() -> Result<(), MarshalError> {
        let buffer = Vec::new();
        let mut encoder = PackedEncoder::new(buffer);

        encoder.open_element(&ELEM_DATA)?;
        encoder.write_string(&ATTRIB_NAME, "parent")?;

        encoder.open_element(&ELEM_VALUE)?;
        encoder.write_string(&ATTRIB_NAME, "child1")?;
        encoder.write_bool(&ATTRIB_READONLY, true)?;
        encoder.close_element(&ELEM_VALUE)?;

        encoder.open_element(&ELEM_VALUE)?;
        encoder.write_string(&ATTRIB_NAME, "child2")?;
        encoder.write_signed_integer(&ATTRIB_VAL, -123)?;
        encoder.close_element(&ELEM_VALUE)?;

        encoder.close_element(&ELEM_DATA)?;

        let encoded_data = encoder.out_stream;
        let mut decoder = PackedDecoder::new();
        decoder.ingest_stream(Cursor::new(encoded_data))?;

        let parent_id = decoder.open_element()?;
        assert_eq!(parent_id, ELEM_DATA.id());
        let parent_name = decoder.read_string_with_id(&ATTRIB_NAME)?;
        assert_eq!(parent_name, "parent");

        let child1_id = decoder.open_element()?;
        assert_eq!(child1_id, ELEM_VALUE.id());
        let child1_name = decoder.read_string_with_id(&ATTRIB_NAME)?;
        assert_eq!(child1_name, "child1");
        let readonly = decoder.read_bool_with_id(&ATTRIB_READONLY)?;
        assert!(readonly);
        decoder.close_element(child1_id)?;

        let child2_id = decoder.open_element()?;
        assert_eq!(child2_id, ELEM_VALUE.id());
        let child2_name = decoder.read_string_with_id(&ATTRIB_NAME)?;
        assert_eq!(child2_name, "child2");
        let val = decoder.read_signed_integer_with_id(&ATTRIB_VAL)?;
        assert_eq!(val, -123);
        decoder.close_element(child2_id)?;

        decoder.close_element(parent_id)?;

        Ok(())
    }

    #[test]
    fn test_skip_element() -> Result<(), MarshalError> {
        let buffer = Vec::new();
        let mut encoder = PackedEncoder::new(buffer);

        encoder.open_element(&ELEM_DATA)?;
        encoder.write_string(&ATTRIB_NAME, "root")?;

        encoder.open_element(&ELEM_VALUE)?;
        encoder.write_string(&ATTRIB_NAME, "skip_me")?;
        encoder.close_element(&ELEM_VALUE)?;

        encoder.open_element(&ELEM_TARGET)?;
        encoder.write_string(&ATTRIB_NAME, "want_this")?;
        encoder.close_element(&ELEM_TARGET)?;

        encoder.close_element(&ELEM_DATA)?;

        let encoded_data = encoder.out_stream;
        let mut decoder = PackedDecoder::new();
        decoder.ingest_stream(Cursor::new(encoded_data))?;

        let root_id = decoder.open_element()?;
        assert_eq!(root_id, ELEM_DATA.id());

        decoder.skip_element()?;

        let target_id = decoder.open_element()?;
        assert_eq!(target_id, ELEM_TARGET.id());
        let name = decoder.read_string_with_id(&ATTRIB_NAME)?;
        assert_eq!(name, "want_this");
        decoder.close_element(target_id)?;

        decoder.close_element(root_id)?;

        Ok(())
    }
}
