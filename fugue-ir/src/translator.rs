use std::borrow::Borrow;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use fnv::FnvHashMap as Map;
use fugue_arch::ArchitectureDef;
use itertools::Itertools;

use crate::address::AddressValue;

use crate::deserialise::parse::XmlExt;
use crate::deserialise::Error as DeserialiseError;

use crate::disassembly::ContextDatabase;
use crate::disassembly::Error as DisassemblyError;
use crate::disassembly::PatternExpression;
use crate::disassembly::{ParserContext, ParserState, ParserWalker, PCodeRaw, IRBuilder};
use crate::disassembly::VarnodeData;
use crate::disassembly::symbol::{FixedHandle, Symbol, SymbolScope, SymbolTable};
use crate::disassembly::walker::InstructionFormatter;

use crate::il::Instruction;
use crate::il::ecode::ECode;
use crate::il::pcode::PCode;

use crate::error::Error;

use crate::float_format::FloatFormat;

use crate::compiler;
use crate::convention::Convention;
use crate::space_manager::SpaceManager;

#[ouroboros::self_referencing(chain_hack)]
#[derive(Clone)]
pub struct TranslatorImpl {
    alignment: usize,
    big_endian: bool,

    unique_base: u64,
    unique_mask: u64,

    maximum_delay: usize,
    section_count: usize,

    float_formats: Vec<Arc<FloatFormat>>,
    manager: Box<SpaceManager>,

    #[borrows(manager)]
    #[covariant]
    pub symbol_table: Box<SymbolTable>,

    #[borrows(symbol_table)]
    #[covariant]
    pub root: &'this Symbol,

    #[borrows(symbol_table)]
    #[covariant]
    pub global_scope: &'this SymbolScope,

    #[borrows(manager)]
    #[covariant]
    pub registers: Map<(u64, usize), Arc<str>>,

    #[borrows(manager)]
    #[covariant]
    pub registers_by_name: Map<Arc<str>, VarnodeData>,

    pub registers_size: usize,

    #[borrows(manager)]
    #[covariant]
    pub program_counter: VarnodeData,

    #[borrows(manager)]
    #[covariant]
    pub user_ops: Vec<Arc<str>>,

    #[borrows(manager)]
    #[covariant]
    pub context_db: ContextDatabase,

    #[covariant]
    pub architecture: ArchitectureDef,

    #[borrows(manager)]
    #[covariant]
    pub compiler_conventions: Map<String, Convention>,
}

#[derive(Clone)]
#[repr(transparent)]
pub struct Translator(TranslatorImpl);

impl Translator {
    pub fn is_big_endian(&self) -> bool {
        *self.0.borrow_big_endian()
    }

    pub fn is_little_endian(&self) -> bool {
        !*self.0.borrow_big_endian()
    }

    pub fn alignment(&self) -> usize {
        *self.0.borrow_alignment()
    }

    pub fn unique_base(&self) -> u64 {
        *self.0.borrow_unique_base()
    }

    pub fn unique_mask(&self) -> u64 {
        *self.0.borrow_unique_mask()
    }

    pub fn float_formats(&self) -> &[Arc<FloatFormat>] {
        self.0.borrow_float_formats().as_ref()
    }

    pub fn float_format(&self, size: usize) -> Option<Arc<FloatFormat>> {
        self.0.borrow_float_formats()
            .iter()
            .find_map(|ff| if ff.size() == size { Some(ff.clone()) } else { None })
    }

    pub fn context_database(&self) -> ContextDatabase {
        self.0.borrow_context_db().clone()
    }

    pub fn set_variable_default<S: Borrow<str>>(&mut self, name: S, value: u32) {
        self.0.with_context_db_mut(|db| db.set_variable_default(name, value));
    }

    pub fn address(&self, address: u64) -> AddressValue {
        let space = self.manager().default_space();
        AddressValue::new(space, address)
    }

    pub fn manager(&self) -> &SpaceManager {
        self.0.borrow_manager()
    }

    pub fn registers(&self) -> &Map<(u64, usize), Arc<str>> {
        self.0.borrow_registers()
    }

    pub fn register_by_name<S: AsRef<str>>(&self, name: S) -> Option<&VarnodeData> {
        self.0.borrow_registers_by_name().get(name.as_ref())
    }

    pub fn register_space_size(&self) -> usize {
        *self.0.borrow_registers_size()
    }

    pub fn unique_space_size(&self) -> usize {
        *self.0.borrow_unique_mask() as usize
    }

    pub fn symbol_table(&self) -> &SymbolTable {
        self.0.borrow_symbol_table()
    }

    pub fn user_ops(&self) -> &[Arc<str>] {
        self.0.borrow_user_ops()
    }

    pub fn architecture(&self) -> &ArchitectureDef {
        self.0.borrow_architecture()
    }

    pub fn program_counter(&self) -> &VarnodeData {
        self.0.borrow_program_counter()
    }

    pub fn from_file<PC: AsRef<str>, P: AsRef<Path>>(
        program_counter: PC,
        architecture: &ArchitectureDef,
        compiler_specs: &Map<String, compiler::Specification>,
        path: P,
    ) -> Result<Self, Error> {
        let path = path.as_ref();
        let mut file = File::open(path).map_err(|error| Error::ParseFile {
            path: path.to_owned(),
            error,
        })?;

        let mut input = String::new();
        file.read_to_string(&mut input)
            .map_err(|error| Error::ParseFile {
                path: path.to_owned(),
                error,
            })?;

        Self::from_str(program_counter, architecture, compiler_specs, &input)
            .map_err(|error| Error::DeserialiseFile {
                path: path.to_owned(),
                error,
            })
    }

    pub fn from_str<PC: AsRef<str>, S: AsRef<str>>(
        program_counter: PC,
        architecture: &ArchitectureDef,
        compiler_specs: &Map<String, compiler::Specification>,
        input: S,
    ) -> Result<Self, DeserialiseError> {
        let document = xml::Document::parse(input.as_ref()).map_err(DeserialiseError::Xml)?;

        Self::from_xml(program_counter, architecture, compiler_specs, document.root_element())
    }

    fn build_xrefs<PC: AsRef<str>>(&mut self, program_counter: PC, compiler_specs: &Map<String, compiler::Specification>) -> Result<(), DeserialiseError> {
        self.0.with_mut(|mut slf| {
            let registers = &mut slf.registers;
            let register_names = &mut slf.registers_by_name;

            let user_ops = &mut slf.user_ops;
            let mut pc = None;
            let mut registers_size = 0;

            let pc_name = program_counter.as_ref();
            let register_space = slf.manager.register_space();

            for sym_id in slf.global_scope.iter() {
                match slf.symbol_table.symbol(*sym_id) {
                    None => return Err(DeserialiseError::Invariant("invalid symbol")),
                    Some(Symbol::Varnode {
                        name,
                        ref offset,
                        ref size,
                        ..
                    }) => {
                        if registers
                            .insert((*offset, *size), name.clone())
                            .is_some()
                        {
                            // duplicate
                            return Err(DeserialiseError::Invariant("duplicate varnode"));
                        }
                        register_names.insert(
                            name.clone(),
                            VarnodeData::new(register_space.clone(), *offset, *size),
                        );

                        if let Some(size) = size.checked_add(*offset as usize) {
                            registers_size = registers_size.max(size);
                        } else {
                            return Err(DeserialiseError::Invariant("offset with size of varnode overflows"));
                        }

                        if pc_name == name.as_ref() {
                            if pc.is_some() {
                                return Err(DeserialiseError::Invariant(
                                    "duplicate definition of program counter",
                                ));
                            }
                            pc = Some((*offset, *size));
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
                            slf.context_db
                                .register_variable(name.clone(), *bit_start, *bit_end)
                                .ok_or_else(|| DeserialiseError::Invariant("duplicate context variable"))?;
                        } else {
                            return Err(DeserialiseError::Invariant(
                                "context symbol does not have context pattern"
                            ))
                        }
                    }
                    Some(Symbol::UserOp {
                        index, name, ..
                    }) => {
                        if user_ops.len() <= *index {
                            user_ops.resize_with(index + 1, || Arc::from(""));
                        }
                        user_ops[*index] = name.clone();
                    }
                    _ => (),
                }
            }

            if let Some((pc_offset, pc_size)) = pc {
                slf.program_counter.offset = pc_offset;
                slf.program_counter.size = pc_size;
            } else {
                return Err(DeserialiseError::Invariant(
                    "program counter not defined as a register",
                ))
            }

            *slf.registers_size = registers_size;

            for (name, spec) in compiler_specs.iter() {
                let conv = Convention::from_spec(spec,
                                                 &slf.registers_by_name,
                                                 &slf.manager)?;
                slf.compiler_conventions.insert(name.clone(), conv);
            }

            Ok(())
        })
    }

    pub fn from_xml<PC: AsRef<str>>(
        program_counter: PC,
        architecture: &ArchitectureDef,
        compiler_specs: &Map<String, compiler::Specification>,
        input: xml::Node,
    ) -> Result<Self, DeserialiseError> {
        if input.tag_name().name() != "sleigh" {
            return Err(DeserialiseError::TagUnexpected(
                input.tag_name().name().to_owned(),
            ));
        }

        let alignment = input.attribute_int("align")?;
        let big_endian = input.attribute_bool("bigendian")?;
        let unique_base = input.attribute_int("uniqbase")?;

        let maximum_delay = input.attribute_int_opt("maxdelay", 0)?;
        let unique_mask = input.attribute_int_opt("uniqmask", 0)?;
        let section_count = input.attribute_int_opt("numsections", 0)?;

        let mut children = input.children().filter(xml::Node::is_element).peekable();

        let mut float_formats = children
            .peeking_take_while(|node| node.tag_name().name() == "floatformat")
            .map(|node| Ok(Arc::new(FloatFormat::from_xml(node)?)))
            .collect::<Result<Vec<_>, DeserialiseError>>()?;

        if float_formats.is_empty() {
            float_formats.push(Arc::new(FloatFormat::float4()));
            float_formats.push(Arc::new(FloatFormat::float8()));
        }

        let manager = SpaceManager::from_xml(
            children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("spaces not defined"))?,
        )?;

        let mut slf = Self(TranslatorImpl::try_new(
            alignment,
            big_endian,
            unique_base,
            unique_mask,
            maximum_delay,
            section_count,
            float_formats,
            Box::new(manager),
            |manager| {
                SymbolTable::from_xml(
                    &manager,
                    children
                        .next()
                        .ok_or_else(|| DeserialiseError::Invariant("symbol table not defined"))?,
                )
                .map(Box::new)
            },
            |symbol_table| {
                symbol_table
                    .global_scope()
                    .ok_or_else(|| DeserialiseError::Invariant("global scope not defined"))?
                    .find("instruction", &symbol_table)
                    .ok_or_else(|| {
                        DeserialiseError::Invariant("instruction root symbol not defined")
                    })
            },
            |symbol_table| {
                symbol_table
                    .global_scope()
                    .ok_or_else(|| DeserialiseError::Invariant("global scope not defined"))
            },
            |_| Ok(Map::default()),
            |_| Ok(Map::default()),
            0,
            |manager| {
                let register_space = manager.register_space();
                Ok(VarnodeData::new(register_space, 0, 0))
            },
            |_| Ok(Vec::new()),
            |_| Ok(ContextDatabase::new()),
            architecture.clone(),
            |_| Ok(Map::default()),
        )?);

        slf.build_xrefs(program_counter, compiler_specs)?;

        Ok(slf)
    }

    pub fn disassemble<'a>(&'a self, db: &mut ContextDatabase, address: AddressValue, bytes: &[u8]) -> Result<Instruction, Error> {
        if self.alignment() != 1 {
            if address.offset() % self.alignment() as u64 != 0 {
                return Err(DisassemblyError::IncorrectAlignment {
                    address: address.offset(),
                    alignment: self.alignment(),
                })?
            }
        }

        let mut context = ParserContext::new(db, address.clone(), bytes);
        let mut walker = ParserWalker::new(&mut context);

        Translator::resolve(&mut walker, self.0.borrow_root().id(), self.0.borrow_symbol_table())?;
        walker.base_state();

        let delay_slots = walker.delay_slot();
        let length = walker.length();

        let ctor = walker.constructor()?.ok_or_else(|| DisassemblyError::InvalidConstructor)?;

        let fmt = InstructionFormatter::new(walker, self.0.borrow_symbol_table(), ctor);

        let mnemonic = format!("{}", fmt.mnemonic());
        let operands = format!("{}", fmt.operands());

        Ok(Instruction {
            address,
            mnemonic,
            operands,
            delay_slots,
            length,
        })
    }

    pub fn lift_pcode_raw(&self, db: &mut ContextDatabase, address: AddressValue, bytes: &[u8]) -> Result<PCodeRaw, Error> {
        self.0.with(|slf| {
            if *slf.alignment != 1 {
                if address.offset() % *slf.alignment as u64 != 0 {
                    return Err(DisassemblyError::IncorrectAlignment {
                        address: address.offset(),
                        alignment: *slf.alignment,
                    })?
                }
            }

            // Main instruction
            let mut context = ParserContext::new(db, address.clone(), bytes);
            let mut walker = ParserWalker::new(&mut context);

            Translator::resolve(&mut walker, slf.root.id(), &slf.symbol_table)?;
            Translator::resolve_handles(&mut walker, &slf.manager, &slf.symbol_table)?;

            walker.base_state();
            walker.apply_commits(db, &slf.manager, &slf.symbol_table)?;

            let mut fall_offset = walker.length();

            let delay_slots = walker.delay_slot();
            let mut delay_contexts = Map::default();

            if delay_slots > 0 {
                let mut byte_count = 0;
                loop {
                    let mut dcontext =
                        ParserContext::new(db, address.clone() + fall_offset, &bytes[fall_offset..]);
                    let mut dwalker = ParserWalker::new(&mut dcontext);

                    Translator::resolve(&mut dwalker, slf.root.id(), &slf.symbol_table)?;
                    Translator::resolve_handles(&mut dwalker, &slf.manager, &slf.symbol_table)?;

                    dwalker.base_state();
                    dwalker.apply_commits(db, &slf.manager, &slf.symbol_table)?;

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
                let tmpl = ctor.template()
                    .ok_or_else(|| DisassemblyError::InconsistentState)?;
                let mut builder =
                    IRBuilder::new(ParserWalker::new(&mut context), &mut delay_contexts, &slf.manager, &slf.float_formats, &slf.registers, &slf.user_ops, *slf.unique_mask)?;

                builder.build(tmpl, None, &slf.symbol_table)?;
                builder.resolve_relatives();
                Ok(builder.emit_raw(fall_offset))
            } else {
                Ok(PCodeRaw::nop(address, walker.length()))
            }
        })
    }

    pub fn lift_pcode(&self, db: &mut ContextDatabase, address: AddressValue, bytes: &[u8]) -> Result<PCode, Error> {
        self.0.with(|slf| {
            if *slf.alignment != 1 {
                if address.offset() % *slf.alignment as u64 != 0 {
                    return Err(DisassemblyError::IncorrectAlignment {
                        address: address.offset(),
                        alignment: *slf.alignment,
                    })?
                }
            }

            // Main instruction
            let mut context = ParserContext::new(db, address.clone(), bytes);
            let mut walker = ParserWalker::new(&mut context);

            Translator::resolve(&mut walker, slf.root.id(), &slf.symbol_table)?;
            Translator::resolve_handles(&mut walker, &slf.manager, &slf.symbol_table)?;

            walker.base_state();
            walker.apply_commits(db, &slf.manager, &slf.symbol_table)?;

            let mut fall_offset = walker.length();

            let delay_slots = walker.delay_slot();
            let mut delay_contexts = Map::default();

            if delay_slots > 0 {
                let mut byte_count = 0;
                loop {
                    let mut dcontext =
                        ParserContext::new(db, address.clone() + fall_offset, &bytes[fall_offset..]);
                    let mut dwalker = ParserWalker::new(&mut dcontext);

                    Translator::resolve(&mut dwalker, slf.root.id(), &slf.symbol_table)?;
                    Translator::resolve_handles(&mut dwalker, &slf.manager, &slf.symbol_table)?;

                    dwalker.base_state();
                    dwalker.apply_commits(db, &slf.manager, &slf.symbol_table)?;

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

            if let Some(ctor) = walker.constructor()? {
                let tmpl = ctor.template()
                    .ok_or_else(|| DisassemblyError::InconsistentState)?;
                let mut builder =
                    IRBuilder::new(ParserWalker::new(&mut context), &mut delay_contexts, &slf.manager, &slf.float_formats, &slf.registers, &slf.user_ops, *slf.unique_mask)?;

                builder.build(tmpl, None, &slf.symbol_table)?;
                builder.resolve_relatives();
                Ok(builder.emit_pcode(fall_offset))
            } else {
                Ok(PCode::nop(address, walker.length()))
            }
        })
    }

    pub fn lift_ecode(&self, db: &mut ContextDatabase, address: AddressValue, bytes: &[u8]) -> Result<ECode, Error> {
        self.0.with(|slf| {
            if *slf.alignment != 1 {
                if address.offset() % *slf.alignment as u64 != 0 {
                    return Err(DisassemblyError::IncorrectAlignment {
                        address: address.offset(),
                        alignment: *slf.alignment,
                    })?
                }
            }

            // Main instruction
            let mut context = ParserContext::new(db, address.clone(), bytes);
            let mut walker = ParserWalker::new(&mut context);

            Translator::resolve(&mut walker, slf.root.id(), &slf.symbol_table)?;
            Translator::resolve_handles(&mut walker, &slf.manager, &slf.symbol_table)?;

            walker.base_state();
            walker.apply_commits(db, &slf.manager, &slf.symbol_table)?;

            let mut fall_offset = walker.length();

            let delay_slots = walker.delay_slot();
            let mut delay_contexts = Map::default();

            if delay_slots > 0 {
                let mut byte_count = 0;
                loop {
                    let mut dcontext =
                        ParserContext::new(db, address.clone() + fall_offset, &bytes[fall_offset..]);
                    let mut dwalker = ParserWalker::new(&mut dcontext);

                    Translator::resolve(&mut dwalker, slf.root.id(), &slf.symbol_table)?;
                    Translator::resolve_handles(&mut dwalker, &slf.manager, &slf.symbol_table)?;

                    dwalker.base_state();
                    dwalker.apply_commits(db, &slf.manager, &slf.symbol_table)?;

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

            if let Some(ctor) = walker.constructor()? {
                let tmpl = ctor.template()
                    .ok_or_else(|| DisassemblyError::InconsistentState)?;
                let mut builder =
                    IRBuilder::new(ParserWalker::new(&mut context), &mut delay_contexts, &slf.manager, &slf.float_formats, &slf.registers, &slf.user_ops, *slf.unique_mask)?;

                builder.build(tmpl, None, &slf.symbol_table)?;
                builder.resolve_relatives();
                Ok(builder.emit_ecode(fall_offset))
            } else {
                Ok(ECode::nop(address, walker.length()))
            }
        })
    }

    fn resolve_handles<'a, 'b, 'c>(
        walker: &mut ParserWalker<'b, 'c>,
        manager: &'a SpaceManager,
        symbol_table: &'b SymbolTable,
    ) -> Result<(), Error> {
        // assumes resolve has resolved all constructors
        walker.base_state();

        while walker.is_state() {
            let ct = walker.constructor()?.ok_or_else(|| DisassemblyError::InvalidConstructor)?;

            let nops = ct.operand_count();
            let mut op = walker.operand();

            'inner: while op < nops {
                let operand = symbol_table
                    .symbol(ct.operand(op))
                    .ok_or_else(|| DisassemblyError::InvalidSymbol)?;

                walker.push_operand(op)?;

                if let Some(tsym) = operand.defining_symbol(symbol_table)? {
                    if tsym.is_subtable() {
                        break 'inner;
                    } else {
                        let h = tsym.fixed_handle(walker, manager, symbol_table)?;
                        walker.set_parent_handle(h)?;
                    }
                } else {
                    let pexp = operand.defining_expression()?.ok_or_else(|| DisassemblyError::InvalidPattern)?;
                    let res = pexp.value(walker, symbol_table)?;
                    let const_space = manager.constant_space();
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

    fn resolve<'b, 'c>(
        walker: &mut ParserWalker<'b, 'c>,
        root: usize,
        symbol_table: &'b SymbolTable,
    ) -> Result<(), Error> {
        let ctor = symbol_table.resolve(root, walker)?;
        walker.set_constructor(ctor)?;
        ctor.apply_context(walker, symbol_table)?;

        while walker.is_state() {
            let ct = walker.constructor()?.ok_or_else(|| DisassemblyError::InvalidConstructor)?;
            let nops = ct.operand_count();
            let mut op = walker.operand();

            'inner: while op < nops {
                let operand = symbol_table
                    .symbol(ct.operand(op))
                    .ok_or_else(|| DisassemblyError::InvalidSymbol)?;

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
    use fugue_bytes::Endian;

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

        translator.set_variable_default("addrsize", 1);
        translator.set_variable_default("opsize", 1);

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

        translator.set_variable_default("addrsize", 1);
        translator.set_variable_default("opsize", 1);

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
        let mut translator = Translator::from_file(
            "pc",
            &super::ArchitectureDef::new("MIPS", Endian::Little, 32, "default"),
            &super::Map::default(),
            "./data/mips32le.sla")?;

        translator.set_variable_default("RELP", 1);
        translator.set_variable_default("PAIR_INSTRUCTION_FLAG", 0);

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
        let mut db = translator.context_database();

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
                .lift_pcode(&mut db, translator.address(address), &more_code[offset..])
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

            //println!("{}", output.display(&translator));
            println!("{}", output.display());

            offset += output.length();
        }

        Ok(())
    }
}
