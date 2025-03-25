use std::io::{self, Cursor, Read, Write};

use flate2::write::{ZlibDecoder, ZlibEncoder};
use flate2::Compression;

use crate::{
    AddressSpaceRef, AttributeId, Decoder, DecoderError, ElementId, Encoder, EncoderError,
    MarshalError, PackedDecoder, PackedEncoder,
};

pub const ATTRIB_VAL: AttributeId = AttributeId::new("val", 2);
pub const ATTRIB_ID: AttributeId = AttributeId::new("id", 3);
pub const ATTRIB_SPACE: AttributeId = AttributeId::new("space", 4);
pub const ATTRIB_S: AttributeId = AttributeId::new("s", 5);
pub const ATTRIB_OFF: AttributeId = AttributeId::new("off", 6);
pub const ATTRIB_CODE: AttributeId = AttributeId::new("code", 7);
pub const ATTRIB_MASK: AttributeId = AttributeId::new("mask", 8);
pub const ATTRIB_INDEX: AttributeId = AttributeId::new("index", 9);
pub const ATTRIB_NONZERO: AttributeId = AttributeId::new("nonzero", 10);
pub const ATTRIB_PIECE: AttributeId = AttributeId::new("piece", 11);
pub const ATTRIB_NAME: AttributeId = AttributeId::new("name", 12);
pub const ATTRIB_SCOPE: AttributeId = AttributeId::new("scope", 13);
pub const ATTRIB_STARTBIT: AttributeId = AttributeId::new("startbit", 14);
pub const ATTRIB_SIZE: AttributeId = AttributeId::new("size", 15);
pub const ATTRIB_TABLE: AttributeId = AttributeId::new("table", 16);
pub const ATTRIB_CT: AttributeId = AttributeId::new("ct", 17);
pub const ATTRIB_MINLEN: AttributeId = AttributeId::new("minlen", 18);
pub const ATTRIB_BASE: AttributeId = AttributeId::new("base", 19);
pub const ATTRIB_NUMBER: AttributeId = AttributeId::new("number", 20);
pub const ATTRIB_CONTEXT: AttributeId = AttributeId::new("context", 21);
pub const ATTRIB_PARENT: AttributeId = AttributeId::new("parent", 22);
pub const ATTRIB_SUBSYM: AttributeId = AttributeId::new("subsym", 23);
pub const ATTRIB_LINE: AttributeId = AttributeId::new("line", 24);
pub const ATTRIB_SOURCE: AttributeId = AttributeId::new("source", 25);
pub const ATTRIB_LENGTH: AttributeId = AttributeId::new("length", 26);
pub const ATTRIB_FIRST: AttributeId = AttributeId::new("first", 27);
pub const ATTRIB_PLUS: AttributeId = AttributeId::new("plus", 28);
pub const ATTRIB_SHIFT: AttributeId = AttributeId::new("shift", 29);
pub const ATTRIB_ENDBIT: AttributeId = AttributeId::new("endbit", 30);
pub const ATTRIB_SIGNBIT: AttributeId = AttributeId::new("signbit", 31);
pub const ATTRIB_ENDBYTE: AttributeId = AttributeId::new("endbyte", 32);
pub const ATTRIB_STARTBYTE: AttributeId = AttributeId::new("startbyte", 33);
pub const ATTRIB_VERSION: AttributeId = AttributeId::new("version", 34);
pub const ATTRIB_BIGENDIAN: AttributeId = AttributeId::new("bigendian", 35);
pub const ATTRIB_ALIGN: AttributeId = AttributeId::new("align", 36);
pub const ATTRIB_UNIQBASE: AttributeId = AttributeId::new("uniqbase", 37);
pub const ATTRIB_MAXDELAY: AttributeId = AttributeId::new("maxdelay", 38);
pub const ATTRIB_UNIQMASK: AttributeId = AttributeId::new("uniqmask", 39);
pub const ATTRIB_NUMSECTIONS: AttributeId = AttributeId::new("numsections", 40);
pub const ATTRIB_DEFAULTSPACE: AttributeId = AttributeId::new("defaultspace", 41);
pub const ATTRIB_DELAY: AttributeId = AttributeId::new("delay", 42);
pub const ATTRIB_WORDSIZE: AttributeId = AttributeId::new("wordsize", 43);
pub const ATTRIB_PHYSICAL: AttributeId = AttributeId::new("physical", 44);
pub const ATTRIB_SCOPESIZE: AttributeId = AttributeId::new("scopesize", 45);
pub const ATTRIB_SYMBOLSIZE: AttributeId = AttributeId::new("symbolsize", 46);
pub const ATTRIB_VARNODE: AttributeId = AttributeId::new("varnode", 47);
pub const ATTRIB_LOW: AttributeId = AttributeId::new("low", 48);
pub const ATTRIB_HIGH: AttributeId = AttributeId::new("high", 49);
pub const ATTRIB_FLOW: AttributeId = AttributeId::new("flow", 50);
pub const ATTRIB_CONTAIN: AttributeId = AttributeId::new("contain", 51);
pub const ATTRIB_I: AttributeId = AttributeId::new("i", 52);
pub const ATTRIB_NUMCT: AttributeId = AttributeId::new("numct", 53);
pub const ATTRIB_SECTION: AttributeId = AttributeId::new("section", 54);
pub const ATTRIB_LABELS: AttributeId = AttributeId::new("labels", 55);

pub const ELEM_CONST_REAL_NAME: &'static str = "const_real";
pub const ELEM_CONST_REAL_ID: u32 = 1;
pub const ELEM_CONST_REAL: ElementId = ElementId::new(ELEM_CONST_REAL_NAME, ELEM_CONST_REAL_ID);
pub const ELEM_VARNODE_TPL_NAME: &'static str = "varnode_tpl";
pub const ELEM_VARNODE_TPL_ID: u32 = 2;
pub const ELEM_VARNODE_TPL: ElementId = ElementId::new(ELEM_VARNODE_TPL_NAME, ELEM_VARNODE_TPL_ID);
pub const ELEM_CONST_SPACEID_NAME: &'static str = "const_spaceid";
pub const ELEM_CONST_SPACEID_ID: u32 = 3;
pub const ELEM_CONST_SPACEID: ElementId =
    ElementId::new(ELEM_CONST_SPACEID_NAME, ELEM_CONST_SPACEID_ID);
pub const ELEM_CONST_HANDLE_NAME: &'static str = "const_handle";
pub const ELEM_CONST_HANDLE_ID: u32 = 4;
pub const ELEM_CONST_HANDLE: ElementId =
    ElementId::new(ELEM_CONST_HANDLE_NAME, ELEM_CONST_HANDLE_ID);
pub const ELEM_OP_TPL_NAME: &'static str = "op_tpl";
pub const ELEM_OP_TPL_ID: u32 = 5;
pub const ELEM_OP_TPL: ElementId = ElementId::new(ELEM_OP_TPL_NAME, ELEM_OP_TPL_ID);
pub const ELEM_MASK_WORD_NAME: &'static str = "mask_word";
pub const ELEM_MASK_WORD_ID: u32 = 6;
pub const ELEM_MASK_WORD: ElementId = ElementId::new(ELEM_MASK_WORD_NAME, ELEM_MASK_WORD_ID);
pub const ELEM_PAT_BLOCK_NAME: &'static str = "pat_block";
pub const ELEM_PAT_BLOCK_ID: u32 = 7;
pub const ELEM_PAT_BLOCK: ElementId = ElementId::new(ELEM_PAT_BLOCK_NAME, ELEM_PAT_BLOCK_ID);
pub const ELEM_PRINT_NAME: &'static str = "print";
pub const ELEM_PRINT_ID: u32 = 8;
pub const ELEM_PRINT: ElementId = ElementId::new(ELEM_PRINT_NAME, ELEM_PRINT_ID);
pub const ELEM_PAIR_NAME: &'static str = "pair";
pub const ELEM_PAIR_ID: u32 = 9;
pub const ELEM_PAIR: ElementId = ElementId::new(ELEM_PAIR_NAME, ELEM_PAIR_ID);
pub const ELEM_CONTEXT_PAT_NAME: &'static str = "context_pat";
pub const ELEM_CONTEXT_PAT_ID: u32 = 10;
pub const ELEM_CONTEXT_PAT: ElementId = ElementId::new(ELEM_CONTEXT_PAT_NAME, ELEM_CONTEXT_PAT_ID);
pub const ELEM_NULL_NAME: &'static str = "null";
pub const ELEM_NULL_ID: u32 = 11;
pub const ELEM_NULL: ElementId = ElementId::new(ELEM_NULL_NAME, ELEM_NULL_ID);
pub const ELEM_OPERAND_EXP_NAME: &'static str = "operand_exp";
pub const ELEM_OPERAND_EXP_ID: u32 = 12;
pub const ELEM_OPERAND_EXP: ElementId = ElementId::new(ELEM_OPERAND_EXP_NAME, ELEM_OPERAND_EXP_ID);
pub const ELEM_OPERAND_SYM_NAME: &'static str = "operand_sym";
pub const ELEM_OPERAND_SYM_ID: u32 = 13;
pub const ELEM_OPERAND_SYM: ElementId = ElementId::new(ELEM_OPERAND_SYM_NAME, ELEM_OPERAND_SYM_ID);
pub const ELEM_OPERAND_SYM_HEAD_NAME: &'static str = "operand_sym_head";
pub const ELEM_OPERAND_SYM_HEAD_ID: u32 = 14;
pub const ELEM_OPERAND_SYM_HEAD: ElementId =
    ElementId::new(ELEM_OPERAND_SYM_HEAD_NAME, ELEM_OPERAND_SYM_HEAD_ID);
pub const ELEM_OPER_NAME: &'static str = "oper";
pub const ELEM_OPER_ID: u32 = 15;
pub const ELEM_OPER: ElementId = ElementId::new(ELEM_OPER_NAME, ELEM_OPER_ID);
pub const ELEM_DECISION_NAME: &'static str = "decision";
pub const ELEM_DECISION_ID: u32 = 16;
pub const ELEM_DECISION: ElementId = ElementId::new(ELEM_DECISION_NAME, ELEM_DECISION_ID);
pub const ELEM_OPPRINT_NAME: &'static str = "opprint";
pub const ELEM_OPPRINT_ID: u32 = 17;
pub const ELEM_OPPRINT: ElementId = ElementId::new(ELEM_OPPRINT_NAME, ELEM_OPPRINT_ID);
pub const ELEM_INSTRUCT_PAT_NAME: &'static str = "instruct_pat";
pub const ELEM_INSTRUCT_PAT_ID: u32 = 18;
pub const ELEM_INSTRUCT_PAT: ElementId =
    ElementId::new(ELEM_INSTRUCT_PAT_NAME, ELEM_INSTRUCT_PAT_ID);
pub const ELEM_COMBINE_PAT_NAME: &'static str = "combine_pat";
pub const ELEM_COMBINE_PAT_ID: u32 = 19;
pub const ELEM_COMBINE_PAT: ElementId = ElementId::new(ELEM_COMBINE_PAT_NAME, ELEM_COMBINE_PAT_ID);
pub const ELEM_CONSTRUCTOR_NAME: &'static str = "constructor";
pub const ELEM_CONSTRUCTOR_ID: u32 = 20;
pub const ELEM_CONSTRUCTOR: ElementId = ElementId::new(ELEM_CONSTRUCTOR_NAME, ELEM_CONSTRUCTOR_ID);
pub const ELEM_CONSTRUCT_TPL_NAME: &'static str = "construct_tpl";
pub const ELEM_CONSTRUCT_TPL_ID: u32 = 21;
pub const ELEM_CONSTRUCT_TPL: ElementId =
    ElementId::new(ELEM_CONSTRUCT_TPL_NAME, ELEM_CONSTRUCT_TPL_ID);
pub const ELEM_SCOPE_NAME: &'static str = "scope";
pub const ELEM_SCOPE_ID: u32 = 22;
pub const ELEM_SCOPE: ElementId = ElementId::new(ELEM_SCOPE_NAME, ELEM_SCOPE_ID);
pub const ELEM_VARNODE_SYM_NAME: &'static str = "varnode_sym";
pub const ELEM_VARNODE_SYM_ID: u32 = 23;
pub const ELEM_VARNODE_SYM: ElementId = ElementId::new(ELEM_VARNODE_SYM_NAME, ELEM_VARNODE_SYM_ID);
pub const ELEM_VARNODE_SYM_HEAD_NAME: &'static str = "varnode_sym_head";
pub const ELEM_VARNODE_SYM_HEAD_ID: u32 = 24;
pub const ELEM_VARNODE_SYM_HEAD: ElementId =
    ElementId::new(ELEM_VARNODE_SYM_HEAD_NAME, ELEM_VARNODE_SYM_HEAD_ID);
pub const ELEM_USEROP_NAME: &'static str = "userop";
pub const ELEM_USEROP_ID: u32 = 25;
pub const ELEM_USEROP: ElementId = ElementId::new(ELEM_USEROP_NAME, ELEM_USEROP_ID);
pub const ELEM_USEROP_HEAD_NAME: &'static str = "userop_head";
pub const ELEM_USEROP_HEAD_ID: u32 = 26;
pub const ELEM_USEROP_HEAD: ElementId = ElementId::new(ELEM_USEROP_HEAD_NAME, ELEM_USEROP_HEAD_ID);
pub const ELEM_TOKENFIELD_NAME: &'static str = "tokenfield";
pub const ELEM_TOKENFIELD_ID: u32 = 27;
pub const ELEM_TOKENFIELD: ElementId = ElementId::new(ELEM_TOKENFIELD_NAME, ELEM_TOKENFIELD_ID);
pub const ELEM_VAR_NAME: &'static str = "var";
pub const ELEM_VAR_ID: u32 = 28;
pub const ELEM_VAR: ElementId = ElementId::new(ELEM_VAR_NAME, ELEM_VAR_ID);
pub const ELEM_CONTEXTFIELD_NAME: &'static str = "contextfield";
pub const ELEM_CONTEXTFIELD_ID: u32 = 29;
pub const ELEM_CONTEXTFIELD: ElementId =
    ElementId::new(ELEM_CONTEXTFIELD_NAME, ELEM_CONTEXTFIELD_ID);
pub const ELEM_HANDLE_TPL_NAME: &'static str = "handle_tpl";
pub const ELEM_HANDLE_TPL_ID: u32 = 30;
pub const ELEM_HANDLE_TPL: ElementId = ElementId::new(ELEM_HANDLE_TPL_NAME, ELEM_HANDLE_TPL_ID);
pub const ELEM_CONST_RELATIVE_NAME: &'static str = "const_relative";
pub const ELEM_CONST_RELATIVE_ID: u32 = 31;
pub const ELEM_CONST_RELATIVE: ElementId =
    ElementId::new(ELEM_CONST_RELATIVE_NAME, ELEM_CONST_RELATIVE_ID);
pub const ELEM_CONTEXT_OP_NAME: &'static str = "context_op";
pub const ELEM_CONTEXT_OP_ID: u32 = 32;
pub const ELEM_CONTEXT_OP: ElementId = ElementId::new(ELEM_CONTEXT_OP_NAME, ELEM_CONTEXT_OP_ID);

pub const ELEM_SLEIGH_NAME: &'static str = "sleigh";
pub const ELEM_SLEIGH_ID: u32 = 33;
pub const ELEM_SLEIGH: ElementId = ElementId::new(ELEM_SLEIGH_NAME, ELEM_SLEIGH_ID);
pub const ELEM_SPACES_NAME: &'static str = "spaces";
pub const ELEM_SPACES_ID: u32 = 34;
pub const ELEM_SPACES: ElementId = ElementId::new(ELEM_SPACES_NAME, ELEM_SPACES_ID);
pub const ELEM_SOURCEFILES_NAME: &'static str = "sourcefiles";
pub const ELEM_SOURCEFILES_ID: u32 = 35;
pub const ELEM_SOURCEFILES: ElementId = ElementId::new(ELEM_SOURCEFILES_NAME, ELEM_SOURCEFILES_ID);
pub const ELEM_SOURCEFILE_NAME: &'static str = "sourcefile";
pub const ELEM_SOURCEFILE_ID: u32 = 36;
pub const ELEM_SOURCEFILE: ElementId = ElementId::new(ELEM_SOURCEFILE_NAME, ELEM_SOURCEFILE_ID);
pub const ELEM_SPACE_NAME: &'static str = "space";
pub const ELEM_SPACE_ID: u32 = 37;
pub const ELEM_SPACE: ElementId = ElementId::new(ELEM_SPACE_NAME, ELEM_SPACE_ID);
pub const ELEM_SYMBOL_TABLE_NAME: &'static str = "symbol_table";
pub const ELEM_SYMBOL_TABLE_ID: u32 = 38;
pub const ELEM_SYMBOL_TABLE: ElementId =
    ElementId::new(ELEM_SYMBOL_TABLE_NAME, ELEM_SYMBOL_TABLE_ID);
pub const ELEM_VALUE_SYM_NAME: &'static str = "value_sym";
pub const ELEM_VALUE_SYM_ID: u32 = 39;
pub const ELEM_VALUE_SYM: ElementId = ElementId::new(ELEM_VALUE_SYM_NAME, ELEM_VALUE_SYM_ID);
pub const ELEM_VALUE_SYM_HEAD_NAME: &'static str = "value_sym_head";
pub const ELEM_VALUE_SYM_HEAD_ID: u32 = 40;
pub const ELEM_VALUE_SYM_HEAD: ElementId =
    ElementId::new(ELEM_VALUE_SYM_HEAD_NAME, ELEM_VALUE_SYM_HEAD_ID);
pub const ELEM_CONTEXT_SYM_NAME: &'static str = "context_sym";
pub const ELEM_CONTEXT_SYM_ID: u32 = 41;
pub const ELEM_CONTEXT_SYM: ElementId = ElementId::new(ELEM_CONTEXT_SYM_NAME, ELEM_CONTEXT_SYM_ID);
pub const ELEM_CONTEXT_SYM_HEAD_NAME: &'static str = "context_sym_head";
pub const ELEM_CONTEXT_SYM_HEAD_ID: u32 = 42;
pub const ELEM_CONTEXT_SYM_HEAD: ElementId =
    ElementId::new(ELEM_CONTEXT_SYM_HEAD_NAME, ELEM_CONTEXT_SYM_HEAD_ID);
pub const ELEM_END_SYM_NAME: &'static str = "end_sym";
pub const ELEM_END_SYM_ID: u32 = 43;
pub const ELEM_END_SYM: ElementId = ElementId::new(ELEM_END_SYM_NAME, ELEM_END_SYM_ID);
pub const ELEM_END_SYM_HEAD_NAME: &'static str = "end_sym_head";
pub const ELEM_END_SYM_HEAD_ID: u32 = 44;
pub const ELEM_END_SYM_HEAD: ElementId =
    ElementId::new(ELEM_END_SYM_HEAD_NAME, ELEM_END_SYM_HEAD_ID);
pub const ELEM_SPACE_OTHER_NAME: &'static str = "space_other";
pub const ELEM_SPACE_OTHER_ID: u32 = 45;
pub const ELEM_SPACE_OTHER: ElementId = ElementId::new(ELEM_SPACE_OTHER_NAME, ELEM_SPACE_OTHER_ID);
pub const ELEM_SPACE_UNIQUE_NAME: &'static str = "space_unique";
pub const ELEM_SPACE_UNIQUE_ID: u32 = 46;
pub const ELEM_SPACE_UNIQUE: ElementId =
    ElementId::new(ELEM_SPACE_UNIQUE_NAME, ELEM_SPACE_UNIQUE_ID);
pub const ELEM_AND_EXP_NAME: &'static str = "and_exp";
pub const ELEM_AND_EXP_ID: u32 = 47;
pub const ELEM_AND_EXP: ElementId = ElementId::new(ELEM_AND_EXP_NAME, ELEM_AND_EXP_ID);
pub const ELEM_DIV_EXP_NAME: &'static str = "div_exp";
pub const ELEM_DIV_EXP_ID: u32 = 48;
pub const ELEM_DIV_EXP: ElementId = ElementId::new(ELEM_DIV_EXP_NAME, ELEM_DIV_EXP_ID);
pub const ELEM_LSHIFT_EXP_NAME: &'static str = "lshift_exp";
pub const ELEM_LSHIFT_EXP_ID: u32 = 49;
pub const ELEM_LSHIFT_EXP: ElementId = ElementId::new(ELEM_LSHIFT_EXP_NAME, ELEM_LSHIFT_EXP_ID);
pub const ELEM_MINUS_EXP_NAME: &'static str = "minus_exp";
pub const ELEM_MINUS_EXP_ID: u32 = 50;
pub const ELEM_MINUS_EXP: ElementId = ElementId::new(ELEM_MINUS_EXP_NAME, ELEM_MINUS_EXP_ID);
pub const ELEM_MULT_EXP_NAME: &'static str = "mult_exp";
pub const ELEM_MULT_EXP_ID: u32 = 51;
pub const ELEM_MULT_EXP: ElementId = ElementId::new(ELEM_MULT_EXP_NAME, ELEM_MULT_EXP_ID);
pub const ELEM_NOT_EXP_NAME: &'static str = "not_exp";
pub const ELEM_NOT_EXP_ID: u32 = 52;
pub const ELEM_NOT_EXP: ElementId = ElementId::new(ELEM_NOT_EXP_NAME, ELEM_NOT_EXP_ID);
pub const ELEM_OR_EXP_NAME: &'static str = "or_exp";
pub const ELEM_OR_EXP_ID: u32 = 53;
pub const ELEM_OR_EXP: ElementId = ElementId::new(ELEM_OR_EXP_NAME, ELEM_OR_EXP_ID);
pub const ELEM_PLUS_EXP_NAME: &'static str = "plus_exp";
pub const ELEM_PLUS_EXP_ID: u32 = 54;
pub const ELEM_PLUS_EXP: ElementId = ElementId::new(ELEM_PLUS_EXP_NAME, ELEM_PLUS_EXP_ID);
pub const ELEM_RSHIFT_EXP_NAME: &'static str = "rshift_exp";
pub const ELEM_RSHIFT_EXP_ID: u32 = 55;
pub const ELEM_RSHIFT_EXP: ElementId = ElementId::new(ELEM_RSHIFT_EXP_NAME, ELEM_RSHIFT_EXP_ID);
pub const ELEM_SUB_EXP_NAME: &'static str = "sub_exp";
pub const ELEM_SUB_EXP_ID: u32 = 56;
pub const ELEM_SUB_EXP: ElementId = ElementId::new(ELEM_SUB_EXP_NAME, ELEM_SUB_EXP_ID);
pub const ELEM_XOR_EXP_NAME: &'static str = "xor_exp";
pub const ELEM_XOR_EXP_ID: u32 = 57;
pub const ELEM_XOR_EXP: ElementId = ElementId::new(ELEM_XOR_EXP_NAME, ELEM_XOR_EXP_ID);
pub const ELEM_INTB_NAME: &'static str = "intb";
pub const ELEM_INTB_ID: u32 = 58;
pub const ELEM_INTB: ElementId = ElementId::new(ELEM_INTB_NAME, ELEM_INTB_ID);
pub const ELEM_END_EXP_NAME: &'static str = "end_exp";
pub const ELEM_END_EXP_ID: u32 = 59;
pub const ELEM_END_EXP: ElementId = ElementId::new(ELEM_END_EXP_NAME, ELEM_END_EXP_ID);
pub const ELEM_NEXT2_EXP_NAME: &'static str = "next2_exp";
pub const ELEM_NEXT2_EXP_ID: u32 = 60;
pub const ELEM_NEXT2_EXP: ElementId = ElementId::new(ELEM_NEXT2_EXP_NAME, ELEM_NEXT2_EXP_ID);
pub const ELEM_START_EXP_NAME: &'static str = "start_exp";
pub const ELEM_START_EXP_ID: u32 = 61;
pub const ELEM_START_EXP: ElementId = ElementId::new(ELEM_START_EXP_NAME, ELEM_START_EXP_ID);
pub const ELEM_EPSILON_SYM_NAME: &'static str = "epsilon_sym";
pub const ELEM_EPSILON_SYM_ID: u32 = 62;
pub const ELEM_EPSILON_SYM: ElementId = ElementId::new(ELEM_EPSILON_SYM_NAME, ELEM_EPSILON_SYM_ID);
pub const ELEM_EPSILON_SYM_HEAD_NAME: &'static str = "epsilon_sym_head";
pub const ELEM_EPSILON_SYM_HEAD_ID: u32 = 63;
pub const ELEM_EPSILON_SYM_HEAD: ElementId =
    ElementId::new(ELEM_EPSILON_SYM_HEAD_NAME, ELEM_EPSILON_SYM_HEAD_ID);
pub const ELEM_NAME_SYM_NAME: &'static str = "name_sym";
pub const ELEM_NAME_SYM_ID: u32 = 64;
pub const ELEM_NAME_SYM: ElementId = ElementId::new(ELEM_NAME_SYM_NAME, ELEM_NAME_SYM_ID);
pub const ELEM_NAME_SYM_HEAD_NAME: &'static str = "name_sym_head";
pub const ELEM_NAME_SYM_HEAD_ID: u32 = 65;
pub const ELEM_NAME_SYM_HEAD: ElementId =
    ElementId::new(ELEM_NAME_SYM_HEAD_NAME, ELEM_NAME_SYM_HEAD_ID);
pub const ELEM_NAMETAB_NAME: &'static str = "nametab";
pub const ELEM_NAMETAB_ID: u32 = 66;
pub const ELEM_NAMETAB: ElementId = ElementId::new(ELEM_NAMETAB_NAME, ELEM_NAMETAB_ID);
pub const ELEM_NEXT2_SYM_NAME: &'static str = "next2_sym";
pub const ELEM_NEXT2_SYM_ID: u32 = 67;
pub const ELEM_NEXT2_SYM: ElementId = ElementId::new(ELEM_NEXT2_SYM_NAME, ELEM_NEXT2_SYM_ID);
pub const ELEM_NEXT2_SYM_HEAD_NAME: &'static str = "next2_sym_head";
pub const ELEM_NEXT2_SYM_HEAD_ID: u32 = 68;
pub const ELEM_NEXT2_SYM_HEAD: ElementId =
    ElementId::new(ELEM_NEXT2_SYM_HEAD_NAME, ELEM_NEXT2_SYM_HEAD_ID);
pub const ELEM_START_SYM_NAME: &'static str = "start_sym";
pub const ELEM_START_SYM_ID: u32 = 69;
pub const ELEM_START_SYM: ElementId = ElementId::new(ELEM_START_SYM_NAME, ELEM_START_SYM_ID);
pub const ELEM_START_SYM_HEAD_NAME: &'static str = "start_sym_head";
pub const ELEM_START_SYM_HEAD_ID: u32 = 70;
pub const ELEM_START_SYM_HEAD: ElementId =
    ElementId::new(ELEM_START_SYM_HEAD_NAME, ELEM_START_SYM_HEAD_ID);
pub const ELEM_SUBTABLE_SYM_NAME: &'static str = "subtable_sym";
pub const ELEM_SUBTABLE_SYM_ID: u32 = 71;
pub const ELEM_SUBTABLE_SYM: ElementId =
    ElementId::new(ELEM_SUBTABLE_SYM_NAME, ELEM_SUBTABLE_SYM_ID);
pub const ELEM_SUBTABLE_SYM_HEAD_NAME: &'static str = "subtable_sym_head";
pub const ELEM_SUBTABLE_SYM_HEAD_ID: u32 = 72;
pub const ELEM_SUBTABLE_SYM_HEAD: ElementId =
    ElementId::new(ELEM_SUBTABLE_SYM_HEAD_NAME, ELEM_SUBTABLE_SYM_HEAD_ID);
pub const ELEM_VALUEMAP_SYM_NAME: &'static str = "valuemap_sym";
pub const ELEM_VALUEMAP_SYM_ID: u32 = 73;
pub const ELEM_VALUEMAP_SYM: ElementId =
    ElementId::new(ELEM_VALUEMAP_SYM_NAME, ELEM_VALUEMAP_SYM_ID);
pub const ELEM_VALUEMAP_SYM_HEAD_NAME: &'static str = "valuemap_sym_head";
pub const ELEM_VALUEMAP_SYM_HEAD_ID: u32 = 74;
pub const ELEM_VALUEMAP_SYM_HEAD: ElementId =
    ElementId::new(ELEM_VALUEMAP_SYM_HEAD_NAME, ELEM_VALUEMAP_SYM_HEAD_ID);
pub const ELEM_VALUETAB_NAME: &'static str = "valuetab";
pub const ELEM_VALUETAB_ID: u32 = 75;
pub const ELEM_VALUETAB: ElementId = ElementId::new(ELEM_VALUETAB_NAME, ELEM_VALUETAB_ID);
pub const ELEM_VARLIST_SYM_NAME: &'static str = "varlist_sym";
pub const ELEM_VARLIST_SYM_ID: u32 = 76;
pub const ELEM_VARLIST_SYM: ElementId = ElementId::new(ELEM_VARLIST_SYM_NAME, ELEM_VARLIST_SYM_ID);
pub const ELEM_VARLIST_SYM_HEAD_NAME: &'static str = "varlist_sym_head";
pub const ELEM_VARLIST_SYM_HEAD_ID: u32 = 77;
pub const ELEM_VARLIST_SYM_HEAD: ElementId =
    ElementId::new(ELEM_VARLIST_SYM_HEAD_NAME, ELEM_VARLIST_SYM_HEAD_ID);
pub const ELEM_OR_PAT_NAME: &'static str = "or_pat";
pub const ELEM_OR_PAT_ID: u32 = 78;
pub const ELEM_OR_PAT: ElementId = ElementId::new(ELEM_OR_PAT_NAME, ELEM_OR_PAT_ID);
pub const ELEM_COMMIT_NAME: &'static str = "commit";
pub const ELEM_COMMIT_ID: u32 = 79;
pub const ELEM_COMMIT: ElementId = ElementId::new(ELEM_COMMIT_NAME, ELEM_COMMIT_ID);
pub const ELEM_CONST_START_NAME: &'static str = "const_start";
pub const ELEM_CONST_START_ID: u32 = 80;
pub const ELEM_CONST_START: ElementId = ElementId::new(ELEM_CONST_START_NAME, ELEM_CONST_START_ID);
pub const ELEM_CONST_NEXT_NAME: &'static str = "const_next";
pub const ELEM_CONST_NEXT_ID: u32 = 81;
pub const ELEM_CONST_NEXT: ElementId = ElementId::new(ELEM_CONST_NEXT_NAME, ELEM_CONST_NEXT_ID);
pub const ELEM_CONST_NEXT2_NAME: &'static str = "const_next2";
pub const ELEM_CONST_NEXT2_ID: u32 = 82;
pub const ELEM_CONST_NEXT2: ElementId = ElementId::new(ELEM_CONST_NEXT2_NAME, ELEM_CONST_NEXT2_ID);
pub const ELEM_CONST_CURSPACE_NAME: &'static str = "const_curspace";
pub const ELEM_CONST_CURSPACE_ID: u32 = 83;
pub const ELEM_CONST_CURSPACE: ElementId =
    ElementId::new(ELEM_CONST_CURSPACE_NAME, ELEM_CONST_CURSPACE_ID);
pub const ELEM_CONST_CURSPACE_SIZE_NAME: &'static str = "const_curspace_size";
pub const ELEM_CONST_CURSPACE_SIZE_ID: u32 = 84;
pub const ELEM_CONST_CURSPACE_SIZE: ElementId =
    ElementId::new(ELEM_CONST_CURSPACE_SIZE_NAME, ELEM_CONST_CURSPACE_SIZE_ID);
pub const ELEM_CONST_FLOWREF_NAME: &'static str = "const_flowref";
pub const ELEM_CONST_FLOWREF_ID: u32 = 85;
pub const ELEM_CONST_FLOWREF: ElementId =
    ElementId::new(ELEM_CONST_FLOWREF_NAME, ELEM_CONST_FLOWREF_ID);
pub const ELEM_CONST_FLOWREF_SIZE_NAME: &'static str = "const_flowref_size";
pub const ELEM_CONST_FLOWREF_SIZE_ID: u32 = 86;
pub const ELEM_CONST_FLOWREF_SIZE: ElementId =
    ElementId::new(ELEM_CONST_FLOWREF_SIZE_NAME, ELEM_CONST_FLOWREF_SIZE_ID);
pub const ELEM_CONST_FLOWDEST_NAME: &'static str = "const_flowdest";
pub const ELEM_CONST_FLOWDEST_ID: u32 = 87;
pub const ELEM_CONST_FLOWDEST: ElementId =
    ElementId::new(ELEM_CONST_FLOWDEST_NAME, ELEM_CONST_FLOWDEST_ID);
pub const ELEM_CONST_FLOWDEST_SIZE_NAME: &'static str = "const_flowdest_size";
pub const ELEM_CONST_FLOWDEST_SIZE_ID: u32 = 88;
pub const ELEM_CONST_FLOWDEST_SIZE: ElementId =
    ElementId::new(ELEM_CONST_FLOWDEST_SIZE_NAME, ELEM_CONST_FLOWDEST_SIZE_ID);

pub const FORMAT_VERSION: u8 = 4;
const IN_BUFFER_SIZE: usize = 4096;

pub fn is_sla_format<R: Read>(stream: &mut R) -> Result<bool, MarshalError> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header)?;
    Ok(&header[..3] == b"sla" && header[3] == FORMAT_VERSION)
}

pub fn write_sla_header<W: Write>(stream: &mut W) -> Result<(), MarshalError> {
    let header = [b's', b'l', b'a', FORMAT_VERSION];
    stream.write_all(&header)?;
    Ok(())
}

pub struct FormatDecoder {
    inner: PackedDecoder,
    in_buffer: Vec<u8>,
}

impl FormatDecoder {
    pub fn new() -> Self {
        Self {
            inner: PackedDecoder::new(),
            in_buffer: vec![0u8; IN_BUFFER_SIZE],
        }
    }

    pub fn new_with<R: Read>(mut stream: R) -> Result<Self, MarshalError> {
        let mut slf = Self {
            inner: PackedDecoder::new(),
            in_buffer: vec![0u8; IN_BUFFER_SIZE],
        };
        slf.ingest_stream(&mut stream)?;
        Ok(slf)
    }
}

impl Decoder for FormatDecoder {
    fn ingest_stream<R: Read>(&mut self, mut stream: R) -> Result<(), MarshalError> {
        if !is_sla_format(&mut stream)? {
            return Err(DecoderError::HeaderFormat.into());
        }

        let decompressed_data = {
            let mut decoder = ZlibDecoder::new(Vec::new());

            loop {
                let n = stream
                    .read(&mut self.in_buffer)
                    .map_err(DecoderError::Decompress)?;
                if n == 0 {
                    break;
                }

                decoder
                    .write_all(&self.in_buffer[..n])
                    .map_err(DecoderError::from)?;
            }

            decoder.finish().map_err(DecoderError::from)?
        };

        self.inner.ingest_stream(Cursor::new(decompressed_data))
    }

    fn peek_element(&mut self) -> Result<u32, MarshalError> {
        self.inner.peek_element()
    }

    fn open_element(&mut self) -> Result<u32, MarshalError> {
        self.inner.open_element()
    }

    fn open_element_with_id(&mut self, elem_id: &ElementId) -> Result<u32, MarshalError> {
        self.inner.open_element_with_id(elem_id)
    }

    fn close_element(&mut self, id: u32) -> Result<(), MarshalError> {
        self.inner.close_element(id)
    }

    fn close_element_skipping(&mut self, id: u32) -> Result<(), MarshalError> {
        self.inner.close_element_skipping(id)
    }

    fn rewind_attributes(&mut self) -> Result<(), MarshalError> {
        self.inner.rewind_attributes()
    }

    fn get_next_attribute_id(&mut self) -> Result<u32, MarshalError> {
        self.inner.get_next_attribute_id()
    }

    fn get_indexed_attribute_id(&mut self, attrib_id: &AttributeId) -> Result<u32, MarshalError> {
        self.inner.get_indexed_attribute_id(attrib_id)
    }

    fn read_bool(&mut self) -> Result<bool, MarshalError> {
        self.inner.read_bool()
    }

    fn read_bool_with_id(&mut self, attrib_id: &AttributeId) -> Result<bool, MarshalError> {
        self.inner.read_bool_with_id(attrib_id)
    }

    fn try_read_bool_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<Option<bool>, MarshalError> {
        self.inner.try_read_bool_with_id(attrib_id)
    }

    fn read_signed_integer(&mut self) -> Result<i64, MarshalError> {
        self.inner.read_signed_integer()
    }

    fn read_signed_integer_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<i64, MarshalError> {
        self.inner.read_signed_integer_with_id(attrib_id)
    }

    fn try_read_signed_integer_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<Option<i64>, MarshalError> {
        self.inner.try_read_signed_integer_with_id(attrib_id)
    }

    fn read_signed_integer_expect_string(
        &mut self,
        expect: &str,
        expect_val: i64,
    ) -> Result<i64, MarshalError> {
        self.inner
            .read_signed_integer_expect_string(expect, expect_val)
    }

    fn read_signed_integer_expect_string_with_id(
        &mut self,
        attrib_id: &AttributeId,
        expect: &str,
        expect_val: i64,
    ) -> Result<i64, MarshalError> {
        self.inner
            .read_signed_integer_expect_string_with_id(attrib_id, expect, expect_val)
    }

    fn read_unsigned_integer(&mut self) -> Result<u64, MarshalError> {
        self.inner.read_unsigned_integer()
    }

    fn read_unsigned_integer_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<u64, MarshalError> {
        self.inner.read_unsigned_integer_with_id(attrib_id)
    }

    fn try_read_unsigned_integer_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<Option<u64>, MarshalError> {
        self.inner.try_read_unsigned_integer_with_id(attrib_id)
    }

    fn read_string(&mut self) -> Result<String, MarshalError> {
        self.inner.read_string()
    }

    fn read_string_with_id(&mut self, attrib_id: &AttributeId) -> Result<String, MarshalError> {
        self.inner.read_string_with_id(attrib_id)
    }

    fn try_read_string_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<Option<String>, MarshalError> {
        self.inner.try_read_string_with_id(attrib_id)
    }

    fn read_space(&mut self) -> Result<AddressSpaceRef, MarshalError> {
        self.inner.read_space()
    }

    fn read_space_with_id(
        &mut self,
        attrib_id: &AttributeId,
    ) -> Result<AddressSpaceRef, MarshalError> {
        self.inner.read_space_with_id(attrib_id)
    }
}

/// A zlib compression wrapper around a Write stream
struct CompressedWriter<W: Write> {
    encoder: ZlibEncoder<Vec<u8>>,
    output: W,
}

impl<W: Write> CompressedWriter<W> {
    fn new(output: W, level: u32) -> Self {
        let compression_level = match level {
            0 => Compression::none(),
            1 => Compression::fast(),
            2 => Compression::default(),
            3 => Compression::best(),
            _ => Compression::default(),
        };

        Self {
            encoder: ZlibEncoder::new(Vec::new(), compression_level),
            output,
        }
    }

    fn finish(mut self) -> Result<W, io::Error> {
        let data = self.encoder.finish()?;
        self.output.write_all(&data)?;
        self.output.flush()?;
        Ok(self.output)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        let compressed_data = self.encoder.get_mut();
        self.output.write_all(compressed_data)?;
        compressed_data.clear();
        self.output.flush()
    }
}

impl<W: Write> Write for CompressedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.encoder.write(buf)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.flush()
    }
}

/// SLA format encoder with compression support
pub struct FormatEncoder<W: Write> {
    /// The encoder for the packed format
    encoder: PackedEncoder<CompressedWriter<W>>,
}

impl<W: Write> FormatEncoder<W> {
    /// Create a new FormatEncoder
    pub fn new(mut output: W, level: u32) -> Result<Self, MarshalError> {
        // Write the SLA header
        write_sla_header(&mut output)?;

        // Create a compressed writer
        let compressed_writer = CompressedWriter::new(output, level);

        // Create the packed encoder that writes to the compressed writer
        let encoder = PackedEncoder::new(compressed_writer);

        Ok(Self { encoder })
    }

    pub fn finish(mut self) -> Result<W, MarshalError> {
        self.flush()?;
        let writer = self
            .encoder
            .out_stream
            .finish()
            .map_err(EncoderError::from)?;
        Ok(writer)
    }

    /// Flush all pending data to the output
    pub fn flush(&mut self) -> Result<(), MarshalError> {
        self.encoder
            .get_writer_mut()
            .flush()
            .map_err(EncoderError::from)?;
        Ok(())
    }
}

impl<W: Write> Encoder for FormatEncoder<W> {
    fn open_element(&mut self, elem_id: &ElementId) -> Result<(), MarshalError> {
        self.encoder.open_element(elem_id)
    }

    fn close_element(&mut self, elem_id: &ElementId) -> Result<(), MarshalError> {
        self.encoder.close_element(elem_id)
    }

    fn write_bool(&mut self, attrib_id: &AttributeId, val: bool) -> Result<(), MarshalError> {
        self.encoder.write_bool(attrib_id, val)
    }

    fn write_signed_integer(
        &mut self,
        attrib_id: &AttributeId,
        val: i64,
    ) -> Result<(), MarshalError> {
        self.encoder.write_signed_integer(attrib_id, val)
    }

    fn write_unsigned_integer(
        &mut self,
        attrib_id: &AttributeId,
        val: u64,
    ) -> Result<(), MarshalError> {
        self.encoder.write_unsigned_integer(attrib_id, val)
    }

    fn write_string(&mut self, attrib_id: &AttributeId, val: &str) -> Result<(), MarshalError> {
        self.encoder.write_string(attrib_id, val)
    }

    fn write_string_indexed(
        &mut self,
        attrib_id: &AttributeId,
        index: u32,
        val: &str,
    ) -> Result<(), MarshalError> {
        self.encoder.write_string_indexed(attrib_id, index, val)
    }

    fn write_space(
        &mut self,
        attrib_id: &AttributeId,
        spc: AddressSpaceRef,
    ) -> Result<(), MarshalError> {
        self.encoder.write_space(attrib_id, spc)
    }
}

impl<W: Write> PackedEncoder<W> {
    fn get_writer_mut(&mut self) -> &mut W {
        &mut self.out_stream
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::{Decoder, Encoder, MarshalError, ELEM_DATA};

    #[test]
    fn test_sla_format_encode_decode() -> Result<(), MarshalError> {
        let buffer = Vec::new();

        let mut encoder = FormatEncoder::new(buffer, 2)?;

        encoder.open_element(&ELEM_DATA)?;
        encoder.write_string(&ATTRIB_NAME, "test_data")?;
        encoder.write_unsigned_integer(&ATTRIB_SIZE, 42)?;
        encoder.close_element(&ELEM_DATA)?;

        let encoded_data = encoder.finish()?;

        let mut decoder = FormatDecoder::new();
        decoder.ingest_stream(Cursor::new(encoded_data))?;

        let elem_id = decoder.open_element()?;
        assert_eq!(elem_id, ELEM_DATA.id());

        let name = decoder.read_string_with_id(&ATTRIB_NAME)?;
        assert_eq!(name, "test_data");

        let size = decoder.read_unsigned_integer_with_id(&ATTRIB_SIZE)?;
        assert_eq!(size, 42);

        decoder.close_element(elem_id)?;

        Ok(())
    }
}
