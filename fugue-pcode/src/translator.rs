use itertools::Itertools;
use snafu::{OptionExt, ResultExt};
use std::collections::BTreeMap as Map;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use crate::address::Address;
use crate::context::ContextDatabase;
use crate::disassembly::{InstructionFormatter, ParserContext, ParserState, ParserWalker};
use crate::error::deserialisation as de;
use crate::error::disassembly as di;
use crate::error::{self, Error};
use crate::float_format::FloatFormat;
use crate::parse::XmlExt;
use crate::pattern::PatternExpression;
use crate::pcode::{PCode, PCodeBuilder};
use crate::space::AddressSpace;
use crate::space_manager::SpaceManager;
use crate::symbol_table::{FixedHandle, Symbol, SymbolTable};
use crate::varnodedata::VarnodeData;

#[derive(Clone)]
pub struct Translator {
    alignment: usize,
    big_endian: bool,
    float_formats: Vec<FloatFormat>,
    manager: SpaceManager,
    //maximum_delay: usize,
    //section_count: usize,
    root: usize,
    symbol_table: SymbolTable,

    unique_mask: u64,
    //unique_base: u64,
    context_db: ContextDatabase,

    registers: HashMap<(u64, usize), String>,
    registers_by_name: HashMap<String, VarnodeData>,
    program_counter: VarnodeData,
    user_ops: Vec<String>,
}

impl Translator {
    pub fn is_big_endian(&self) -> bool {
        self.big_endian
    }

    pub fn is_little_endian(&self) -> bool {
        self.big_endian
    }

    pub fn alignment(&self) -> usize {
        self.alignment
    }

    pub fn unique_mask(&self) -> u64 {
        self.unique_mask
    }

    pub fn float_formats(&self) -> &[FloatFormat] {
        self.float_formats.as_ref()
    }

    pub fn float_format(&self, size: usize) -> Option<&FloatFormat> {
        self.float_formats.iter().find(|ff| ff.size() == size)
    }

    pub fn context(&self) -> &ContextDatabase {
        &self.context_db
    }

    pub fn context_mut(&mut self) -> &mut ContextDatabase {
        &mut self.context_db
    }

    pub fn manager(&self) -> &SpaceManager {
        &self.manager
    }

    pub fn manager_mut(&mut self) -> &mut SpaceManager {
        &mut self.manager
    }

    pub fn registers(&self) -> &HashMap<(u64, usize), String> {
        &self.registers
    }

    pub fn register_by_name<S: AsRef<str>>(&self, name: S) -> Option<&VarnodeData> {
        self.registers_by_name.get(name.as_ref())
    }

    pub fn symbol_table(&self) -> &SymbolTable {
        &self.symbol_table
    }

    pub fn user_ops(&self) -> &[String] {
        &self.user_ops
    }

    pub fn from_file<PC: AsRef<str>, P: AsRef<Path>>(program_counter: PC, path: P) -> Result<Self, Error> {
        let path = path.as_ref();
        let mut file = File::open(path).with_context(|| error::ParseFile {
            path: path.to_owned(),
        })?;

        let mut input = String::new();
        file.read_to_string(&mut input).with_context(|| error::ParseFile {
            path: path.to_owned(),
        })?;

        Self::from_str(program_counter, &input).with_context(|| error::DeserialiseFile {
            path: path.to_owned(),
        })
    }

    pub fn from_str<PC: AsRef<str>, S: AsRef<str>>(program_counter: PC, input: S) -> Result<Self, de::Error> {
        let document = xml::Document::parse(input.as_ref())
            .map_err(|source| de::Error::Xml { source })?;

        Self::from_xml(program_counter, document.root_element())
    }

    pub fn build_xrefs<PC: AsRef<str>>(&mut self, program_counter: PC, register_space: Arc<AddressSpace>) -> Result<(), de::Error> {
        let global_scope = self.symbol_table.global_scope().with_context(|| de::Invariant {
            reason: "missing global scope",
        })?;
        let varnode_xrefs = &mut self.registers;
        let user_ops = &mut self.user_ops;
        let pc_name = program_counter.as_ref();
        let mut pc_set = false;

        for sym_id in global_scope.iter() {
            match self.symbol_table.symbol(*sym_id) {
                None => {
                    return de::Invariant {
                        reason: "invalid symbol",
                    }
                    .fail()
                }
                Some(Symbol::Varnode {
                    ref name,
                    ref offset,
                    ref size,
                    ..
                }) => {
                    if varnode_xrefs
                        .insert((*offset, *size), name.to_owned())
                        .is_some()
                    {
                        // duplicate
                        return de::Invariant {
                            reason: "duplicate varnode",
                        }
                        .fail();
                    }
                    self.registers_by_name.insert(
                        name.to_owned(),
                        VarnodeData::new(register_space.clone(), *offset, *size)
                    );

                    if pc_name == name {
                        if pc_set {
                            return de::Invariant {
                                reason: "duplicate definition of program counter",
                            }.fail()
                        }
                        self.program_counter.offset = *offset;
                        self.program_counter.size = *size;
                        pc_set = true;
                    }
                }
                Some(Symbol::Context {
                    ref name,
                    ref pattern_value,
                    ..
                }) => {
                    if let PatternExpression::ContextField {
                        bit_start, bit_end, ..
                    } = pattern_value
                    {
                        self.context_db
                            .register_variable(&**name, *bit_start, *bit_end)
                            .expect("context variable is not duplicate");
                    } else {
                        return de::Invariant {
                            reason: "context symbol does not have context pattern",
                        }
                        .fail();
                    }
                }
                Some(Symbol::UserOp {
                    index, ref name, ..
                }) => {
                    if user_ops.len() <= *index {
                        user_ops.resize_with(index + 1, Default::default);
                    }
                    user_ops[*index].clone_from(name);
                }
                _ => (),
            }
        }

        if !pc_set {
            de::Invariant {
                reason: "program counter not defined as a register",
            }.fail()
        } else {
            Ok(())
        }
    }

    pub fn from_xml<PC: AsRef<str>>(program_counter: PC, input: xml::Node) -> Result<Self, de::Error> {
        if input.tag_name().name() != "sleigh" {
            return de::TagUnexpected {
                name: input.tag_name().name().to_owned(),
            }
            .fail();
        }

        let alignment = input.attribute_int("align")?;
        let big_endian = input.attribute_bool("bigendian")?;
        // let unique_base = input.attribute_int("uniqbase")?;

        //let maximum_delay = input.attribute_int_opt("maxdelay", 0)?;
        let unique_mask = input.attribute_int_opt("uniqmask", 0)?;
        //let section_count = input.attribute_int_opt("numsections", 0)?;

        let mut children = input.children().filter(xml::Node::is_element).peekable();

        let mut float_formats = children
            .peeking_take_while(|node| node.tag_name().name() == "floatformat")
            .map(FloatFormat::from_xml)
            .collect::<Result<Vec<_>, _>>()?;

        if float_formats.is_empty() {
            float_formats.push(FloatFormat::float4());
            float_formats.push(FloatFormat::float8());
        }

        let manager = SpaceManager::from_xml(children.next().with_context(|| de::Invariant {
            reason: "spaces not defined",
        })?)?;

        let symbol_table = SymbolTable::from_xml(
            &manager,
            children.next().with_context(|| de::Invariant {
                reason: "symbol table not defined",
            })?,
        )?;

        let root = symbol_table
            .global_scope()
            .with_context(|| de::Invariant {
                reason: "global scope not defined",
            })?
            .find("instruction", &symbol_table)
            .with_context(|| de::Invariant {
                reason: "instruction root symbol not defined",
            })?;

        let register_space = manager
            .space_by_name("register")
            .with_context(|| de::Invariant {
                reason: "missing register space",
            })?;
        let program_counter_vnd = VarnodeData::new(register_space.clone(), 0, 0);

        let mut slf = Self {
            alignment,
            big_endian,
            float_formats,
            manager,
            //maximum_delay,
            //section_count,
            symbol_table,
            root,
            //unique_base,
            unique_mask,
            context_db: ContextDatabase::new(),
            registers: HashMap::new(),
            registers_by_name: HashMap::new(),
            program_counter: program_counter_vnd,
            user_ops: Vec::new(),
        };

        slf.build_xrefs(program_counter, register_space)?;

        Ok(slf)
    }

    pub fn format_instruction(&self, address: u64, bytes: &[u8]) -> Result<(String, String, usize), Error> {
        let default_space = self.manager.default_space().with_context(|| di::InvalidSpace)?;
        let address = Address::new(default_space, address);

        let mut context = ParserContext::new(self, address, bytes);
        let mut walker = ParserWalker::new(&mut context);

        Translator::resolve(&mut walker, self.root, &self.symbol_table)?;
        walker.base_state();

        let length = walker.length();
        let ctor = walker.constructor()?.with_context(|| di::InvalidConstructor)?;

        let fmt = InstructionFormatter::new(walker, &self.symbol_table, ctor);

        let mnemonic = format!("{}", fmt.mnemonic());
        let operands = format!("{}", fmt.operands());

        Ok((mnemonic, operands, length))
    }

    pub fn instruction(&mut self, address: u64, bytes: &[u8]) -> Result<PCode, Error> {
        if self.alignment != 1 {
            if address % self.alignment as u64 != 0 {
                return di::IncorrectAlignment {
                    address,
                    alignment: self.alignment,
                }
                .fail()?;
            }
        }

        // Main instruction
        let default_space = self.manager.default_space().with_context(|| di::InvalidSpace)?;
        let address = Address::new(default_space, address);
        let mut context = ParserContext::new(&self, address.clone(), bytes);
        let mut walker = ParserWalker::new(&mut context);

        Translator::resolve(&mut walker, self.root, &self.symbol_table)?;
        Translator::resolve_handles(&mut walker, &self.manager, &self.symbol_table)?;

        walker.base_state();
        walker.apply_commits(&mut self.context_db, &self.manager, &self.symbol_table)?;

        let mut fall_offset = walker.length();

        let delay_slots = walker.delay_slot();
        let mut delay_contexts = Map::new();

        if delay_slots > 0 {
            let mut byte_count = 0;
            loop {
                let mut dcontext =
                    ParserContext::new(&self, address.clone() + fall_offset, &bytes[fall_offset..]);
                let mut dwalker = ParserWalker::new(&mut dcontext);

                Translator::resolve(&mut dwalker, self.root, &self.symbol_table)?;
                Translator::resolve_handles(&mut dwalker, &self.manager, &self.symbol_table)?;

                dwalker.base_state();
                dwalker.apply_commits(&mut self.context_db, &self.manager, &self.symbol_table)?;

                let length = dwalker.length();

                delay_contexts.insert(address.clone() + fall_offset, dcontext);

                fall_offset += length;
                byte_count += length;

                if byte_count >= delay_slots {
                    break;
                }
            }
            walker.set_next_address(address.clone() + fall_offset);
        }

        // let mut delay_refs = delay_contexts.iter_mut().map(|(a, v)| (a.clone(), v)).collect::<Map<_, _>>();

        if let Some(ctor) = walker.constructor()? {
            let tmpl = ctor.template().with_context(|| di::InconsistentState)?;
            let mut builder =
                PCodeBuilder::new(ParserWalker::new(&mut context), &mut delay_contexts, &self)?;

            builder.build(tmpl, None, &self.symbol_table)?;
            builder.resolve_relatives();
            Ok(builder.emit(fall_offset))
        } else {
            Ok(PCode::nop(address, walker.length()))
        }
    }

    fn resolve_handles<'a, 'b>(
        walker: &mut ParserWalker<'a, 'b>,
        manager: &'a SpaceManager,
        symbol_table: &'a SymbolTable,
    ) -> Result<(), Error> {
        // assumes resolve has resolved all constructors
        walker.base_state();

        while walker.is_state() {
            let ct = walker.constructor()?.with_context(|| di::InvalidConstructor)?;

            let nops = ct.operand_count();
            let mut op = walker.operand();

            'inner: while op < nops {
                let operand = symbol_table
                    .symbol(ct.operand(op))
                    .with_context(|| di::InvalidSymbol)?;

                walker.push_operand(op)?;

                if let Some(tsym) = operand.defining_symbol(symbol_table)? {
                    if tsym.is_subtable() {
                        break 'inner;
                    } else {
                        let h = tsym.fixed_handle(walker, manager, symbol_table)?;
                        walker.set_parent_handle(h)?;
                    }
                } else {
                    let pexp = operand.defining_expression()?.with_context(|| di::InvalidPattern)?;
                    let res = pexp.value(walker, symbol_table)?;
                    let const_space = manager.constant_space().with_context(|| di::InvalidSpace)?;
                    if let Some(handle) = walker.parent_handle_mut()? {
                        handle.space = const_space;
                        handle.offset_space = None;
                        handle.offset_offset = res as u64;
                        handle.size = 0;
                    } else {
                        let mut handle = FixedHandle::new(const_space);
                        handle.offset_space = None;
                        handle.offset_offset = res as u64;
                        handle.size = 0;
                        walker.set_parent_handle(handle)?;
                    }
                }
                walker.pop_operand()?;
                op += 1;
            }
            if op >= nops {
                if let Some(templ) = ct.template() {
                    if let Some(res) = templ.result() {
                        let h = res.fix(walker, manager)?;
                        walker.set_parent_handle(h)?;
                    }
                }
                walker.pop_operand()?;
            }
        }

        walker.set_state(ParserState::PCode);

        Ok(())
    }

    fn resolve<'a, 'b>(
        walker: &mut ParserWalker<'a, 'b>,
        root: usize,
        symbol_table: &'a SymbolTable,
    ) -> Result<(), Error> {
        let ctor = symbol_table.resolve(root, walker)?;
        walker.set_constructor(ctor)?;
        ctor.apply_context(walker, symbol_table)?;

        while walker.is_state() {
            let ct = walker.constructor()?.with_context(|| di::InvalidConstructor)?;
            let nops = ct.operand_count();
            let mut op = walker.operand();

            'inner: while op < nops {
                let operand = symbol_table
                    .symbol(ct.operand(op))
                    .with_context(|| di::InvalidSymbol)?;

                let offset = walker.offset(operand.offset_base()?)? + operand.relative_offset()?;

                walker.allocate_operand(op)?;
                walker.set_offset(offset)?;

                if let Some(tsym) = operand.defining_symbol(symbol_table)? {
                    if let Some(sub_ct) = tsym.resolve(walker)? {
                        walker.set_constructor(sub_ct)?;
                        sub_ct.apply_context(walker, symbol_table)?;
                        break 'inner;
                    }
                }
                walker.set_current_length(operand.minimum_length()?)?;
                walker.pop_operand()?;
                op += 1;
            }
            if op >= nops {
                walker.calculate_length(ct.minimum_length(), nops)?;
                walker.pop_operand()?;

                match ct.template() {
                    Some(templ) if templ.delay_slot() > 0 => {
                        walker.set_delay_slot(templ.delay_slot());
                    }
                    _ => (),
                }
            }
        }
        walker.set_next_address(walker.address() + walker.length());
        walker.set_state(ParserState::Disassembly);

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::{Error, Translator};

    /*
    #[test]
    fn test_load_mips32be() -> Result<(), Error> {
        let translator = Translator::from_file("pc", "./data/mips32be.sla")?;

        assert_eq!(translator.manager().spaces().len(), 5);

        //println!("{:#?}", translator.registers);
        //println!("{:#?}", translator.user_ops);
        //println!("{:#?}", translator.context_db);

        Ok(())
    }

    #[test]
    fn test_insn_nop_x86() -> Result<(), Error> {
        let mut translator = Translator::from_file("EIP", "./data/x86.sla")?;

        translator.context_mut().set_variable_default("addrsize", 1);
        translator.context_mut().set_variable_default("opsize", 1);

        let output = translator.format_instruction(0x1000, &[0x90]).expect("ok");
        assert_eq!(output.0.trim(), "NOP");
        println!("{:#x} {}", 0x1000, output.0);

        let output = translator
            .format_instruction(0x1000, &[0x0c, 0xc0])
            .expect("ok");
        assert_eq!(output.0, "OR AL,0xc0");
        println!("{:#x} {}", 0x1000, output.0);

        let output = translator
            .format_instruction(0x1000, &[0x74, 0xc1])
            .expect("ok");
        assert_eq!(output.0, "JZ -0x3f");
        println!("{:#x} {}", 0x1000, output.0);

        let output = translator
            .format_instruction(0x1000, &[0xb0, 0x0b])
            .expect("ok");
        assert_eq!(output.0.trim(), "MOV AL,0xb");
        println!("{:#x} {}", 0x1000, output.0);

        let output = translator
            .format_instruction(0x1000, &[0x31, 0xc3])
            .expect("ok");
        assert_eq!(output.0.trim(), "XOR EBX,EAX");
        println!("{:#x} {}", 0x1000, output.0);

        let output = translator
            .format_instruction(0x1000, &[0x66, 0x31, 0xc3])
            .expect("ok");
        assert_eq!(output.0.trim(), "XOR BX,AX");
        println!("{:#x} {}", 0x1000, output.0);

        let output = translator
            .format_instruction(0x1000, &[0x8b, 0x40, 0x10])
            .expect("ok");
        assert_eq!(output.0.trim(), "MOV EAX,dword ptr [EAX + 0x10]");
        println!("{:#x} {}", 0x1000, output.0);

        let output = translator
            .format_instruction(0x1000, &[0x8b, 0x44, 0x90, 0x04])
            .expect("ok");
        assert_eq!(output.0.trim(), "MOV EAX,dword ptr [EAX + EDX*0x4 + 0x4]");
        println!("{:#x} {}", 0x1000, output.0);

        Ok(())
    }

    #[test]
    fn test_pcode_nop_x86() -> Result<(), Error> {
        let mut translator = Translator::from_file("EIP", "./data/x86.sla")?;

        translator.context_mut().set_variable_default("addrsize", 1);
        translator.context_mut().set_variable_default("opsize", 1);

        let output = translator.instruction(0x1000, &[0x90]).expect("ok");
        println!("{}", output.display(&translator));

        let output = translator.instruction(0x1000, &[0x0c, 0xc0]).expect("ok");
        println!("{}", output.display(&translator));

        let output = translator.instruction(0x1000, &[0x66, 0x31, 0xc3]).expect("ok");
        println!("{}", output.display(&translator));

        let output = translator
            .instruction(0x1000, &[0x8b, 0x44, 0x90, 0x04])
            .expect("ok");

        for op in output.operations() {
            println!("{}", op.display(&translator));
        }

        Ok(())
    }
    */

    #[test]
    fn test_insn_mips32() -> Result<(), Error> {
        // LAB_00406178                                    XREF[1]:     004060ac(j)
        //    00406178 1c 00 bf 8f     lw         ra,0x1c(sp)
        //    0040617c 00 00 00 00     nop
        //    00406180 08 00 e0 03     jr         ra
        //    00406184 20 00 bd 27     _addiu     sp,sp,0x20
        let mut translator = Translator::from_file("pc", "./data/mips32le.sla")?;

        translator.context_mut().set_variable_default("RELP", 1);
        translator
            .context_mut()
            .set_variable_default("PAIR_INSTRUCTION_FLAG", 0);

        /*
        let code = [
            0x1c, 0x00, 0xbf, 0x8f, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0xe0, 0x03, 0x20, 0x00,
            0xbd, 0x27,
        ];

        for i in 0..4usize {
            let output = translator
                .format_instruction(0x406178 + (i as u64) * 4, &code[(i * 4)..])
                .expect("ok");
            println!("{:#x} {}", 0x406178 + (i as u64) * 4, output.0);

            let output = translator
                .instruction(0x406178 + (i as u64) * 4, &code[(i * 4)..])
                .expect("ok");
            println!("L: {}", output.length());
        }
        */

        let more_code = [
            /*
            0x0c, 0x00, 0x1c, 0x3c, 0x50, 0x47, 0x9c, 0x27, 0x21, 0xe0, 0x99, 0x03, 0xe0, 0xff,
            0xbd, 0x27, 0x10, 0x00, 0xbc, 0xaf, 0x18, 0x80, 0x82, 0x8f, 0x00, 0x00, 0x00, 0x00,
            0xd0, 0x32, 0x42, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42, 0x90, 0x1c, 0x00,
            0xbf, 0xaf, 0x32, 0x00, 0x40, 0x14, 0x18, 0x00, 0xbc, 0xaf, 0x18, 0x80, 0x82, 0x8f,
            0x00, 0x00, 0x00, 0x00, 0x34, 0x20, 0x42, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x42, 0x8c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43, 0x8c, 0x00, 0x00, 0x00, 0x00,
            */
            0x15, 0x00, 0x60, 0x10, 0x04, 0x00, 0x42, 0x24,
        ];

        let start = 0x406080u64;
        let mut offset = 0;

        while offset < more_code.len() {
            let address = start + (offset as u64);
            /*
            let output = translator
                .format_instruction(address, &more_code[offset..])
                .expect("ok");
            println!("{:#x} {}", address, output.0);

            let mut orig_len = output.1;
            */
            let output = translator
                .instruction(address, &more_code[offset..])
                .expect("ok");
            /*
            let mut delays = output.delay_slots();

            while delays > 0 {
                let address = address + (orig_len as u64);
                let output = translator
                    .format_instruction(address, &more_code[offset + orig_len..])
                    .expect("ok");
                println!("{:#x} _{}", address, output.0);
                orig_len += output.1;
                delays -= 1;
            }
            */

            println!("{}", output.display(&translator));

            offset += output.length();
        }

        Ok(())
    }
}
