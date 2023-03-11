use std::borrow::Borrow;
use std::convert::TryFrom;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use ahash::AHashMap as Map;

use fugue_arch::ArchitectureDef;
use itertools::Itertools;

use unsafe_unwrap::UnsafeUnwrap;
use ustr::Ustr;

use crate::address::AddressValue;

use crate::deserialise::parse::XmlExt;
use crate::deserialise::Error as DeserialiseError;

use crate::disassembly::lift::{FloatFormats, UserOpStr};
use crate::disassembly::symbol::{FixedHandle, Symbol, SymbolScope, SymbolTable};
use crate::disassembly::walker::InstructionFormatter;
use crate::disassembly::ContextDatabase;
use crate::disassembly::Error as DisassemblyError;
use crate::disassembly::PatternExpression;
use crate::disassembly::VarnodeData;
use crate::disassembly::{
    IRBuilder, IRBuilderArena, IRBuilderBase, PCodeRaw, ParserContext, ParserState, ParserWalker,
};

use crate::il::ecode::ECode;
use crate::il::instruction::{Instruction, InstructionFull};
use crate::il::pcode::PCode;

use crate::error::Error;

use crate::float_format::FloatFormat;

use crate::compiler;
use crate::convention::Convention;

use crate::register::RegisterNames;
use crate::space_manager::SpaceManager;

// Translator is used for parsing the processor spec XML and
// lifting instructions
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct Translator {
    alignment: usize,
    big_endian: bool,
    unique_base: u64,
    unique_mask: u64,
    maximum_delay: usize,
    section_count: usize,
    float_formats: FloatFormats,
    manager: SpaceManager,
    symbol_table: SymbolTable,
    root: Arc<Symbol>,
    global_scope: Arc<SymbolScope>,
    registers: Arc<RegisterNames>,
    registers_size: usize,
    program_counter: VarnodeData,
    user_ops: Vec<UserOpStr>,
    context_db: ContextDatabase,
    architecture: ArchitectureDef,
    compiler_conventions: Map<String, Convention>,
    source_files: Map<String, usize>,
}

impl Translator {
    pub fn is_big_endian(&self) -> bool {
        self.big_endian
    }

    pub fn is_little_endian(&self) -> bool {
        !self.big_endian
    }

    pub fn alignment(&self) -> usize {
        self.alignment
    }

    pub fn unique_base(&self) -> u64 {
        self.unique_base
    }

    pub fn unique_mask(&self) -> u64 {
        self.unique_mask
    }

    pub fn float_formats(&self) -> &Map<usize, Arc<FloatFormat>> {
        &self.float_formats
    }

    pub fn float_format(&self, size: usize) -> Option<Arc<FloatFormat>> {
        self.float_formats.get(&size).cloned()
    }

    pub fn context_database(&self) -> ContextDatabase {
        self.context_db.clone()
    }

    pub fn set_variable_default<S: Borrow<str>>(&mut self, name: S, value: u32) {
        let name = name.borrow();
        log::trace!("setting context variable {} to {}", name, value);
        self.context_db.set_variable_default(name, value);
    }

    pub fn address(&self, address: u64) -> AddressValue {
        let space = self.manager().default_space();
        AddressValue::new(space, address)
    }

    pub fn manager(&self) -> &SpaceManager {
        &self.manager
    }

    pub fn manager_mut(&mut self) -> &mut SpaceManager {
        &mut self.manager
    }

    pub fn registers(&self) -> &Arc<RegisterNames> {
        &self.registers
    }

    pub fn register_by_name<S: AsRef<str>>(&self, name: S) -> Option<VarnodeData> {
        self.registers
            .get_by_name(name.as_ref())
            .map(|(_, offset, size)| {
                VarnodeData::new(&self.registers.register_space(), offset, size)
            })
    }

    pub fn register_space_size(&self) -> usize {
        self.registers_size
    }

    pub fn unique_space_size(&self) -> usize {
        // base is first free offset
        self.unique_base as usize
    }

    pub fn symbol_table(&self) -> &SymbolTable {
        &self.symbol_table
    }

    pub fn user_ops(&self) -> &[UserOpStr] {
        &self.user_ops
    }

    pub fn architecture(&self) -> &ArchitectureDef {
        &self.architecture
    }

    pub fn program_counter(&self) -> &VarnodeData {
        &self.program_counter
    }

    pub fn compiler_conventions(&self) -> &Map<String, Convention> {
        &self.compiler_conventions
    }

    pub fn source_files(&self) -> &Map<String, usize> {
        &self.source_files
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

        Self::from_str(program_counter, architecture, compiler_specs, &input).map_err(|error| {
            Error::DeserialiseFile {
                path: path.to_owned(),
                error,
            }
        })
    }

    pub fn from_str<PC: AsRef<str>, S: AsRef<str>>(
        program_counter: PC,
        architecture: &ArchitectureDef,
        compiler_specs: &Map<String, compiler::Specification>,
        input: S,
    ) -> Result<Self, DeserialiseError> {
        let document = xml::Document::parse(input.as_ref()).map_err(DeserialiseError::Xml)?;

        Self::from_xml(
            program_counter,
            architecture,
            compiler_specs,
            document.root_element(),
        )
    }

    fn build_xrefs<PC: AsRef<str>>(
        &mut self,
        program_counter: PC,
        compiler_specs: &Map<String, compiler::Specification>,
    ) -> Result<(), DeserialiseError> {
        let registers = Arc::<RegisterNames>::get_mut(&mut self.registers)
            .expect("unique access to RegisterNames");

        let user_ops = &mut self.user_ops;
        let mut pc = None;
        let mut registers_size = 0;

        let pc_name = program_counter.as_ref();

        for sym_id in self.global_scope.iter() {
            match self.symbol_table.symbol(*sym_id) {
                None => return Err(DeserialiseError::Invariant("invalid symbol")),
                Some(Symbol::Varnode {
                    name,
                    ref offset,
                    ref size,
                    ..
                }) => {
                    registers.insert(*offset, *size, name.clone());

                    if let Some(size) = size.checked_add(*offset as usize) {
                        registers_size = registers_size.max(size);
                    } else {
                        return Err(DeserialiseError::Invariant(
                            "offset with size of varnode overflows",
                        ));
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
                        self.context_db
                            .register_variable(name.as_str(), *bit_start, *bit_end)
                            .ok_or_else(|| {
                                DeserialiseError::Invariant("duplicate context variable")
                            })?;
                    } else {
                        return Err(DeserialiseError::Invariant(
                            "context symbol does not have context pattern",
                        ));
                    }
                }
                Some(Symbol::UserOp { index, name, .. }) => {
                    if user_ops.len() <= *index {
                        user_ops.resize_with(index + 1, || Ustr::from(""));
                    }
                    user_ops[*index] = name.clone();
                }
                _ => (),
            }
        }

        if let Some((pc_offset, pc_size)) = pc {
            self.program_counter.offset = pc_offset;
            self.program_counter.size = pc_size;
        } else {
            return Err(DeserialiseError::Invariant(
                "program counter not defined as a register",
            ));
        }

        self.registers_size = registers_size;

        for (name, spec) in compiler_specs.iter() {
            let conv = Convention::from_spec(spec, &self.registers, &self.manager)?;
            log::debug!("loaded compiler convention `{}`", name);
            self.compiler_conventions.insert(name.clone(), conv);
        }

        Ok(())
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
        let version = input.attribute_int_opt("version", 2)?;

        let maximum_delay = input.attribute_int_opt("maxdelay", 0)?;
        let unique_mask = input.attribute_int_opt("uniqmask", 0)?;
        let section_count = input.attribute_int_opt("numsections", 0)?;

        let mut children = input.children().filter(xml::Node::is_element).peekable();

        let mut float_formats = children
            .peeking_take_while(|node| node.tag_name().name() == "floatformat")
            .map(|node| {
                let ff = Arc::new(FloatFormat::from_xml(node)?);
                Ok((ff.bits(), ff))
            })
            .collect::<Result<Map<_, _>, DeserialiseError>>()?;

        if float_formats.is_empty() {
            float_formats.insert(16, Arc::new(FloatFormat::float2()));
            float_formats.insert(32, Arc::new(FloatFormat::float4()));
            float_formats.insert(64, Arc::new(FloatFormat::float8()));
            float_formats.insert(80, Arc::new(FloatFormat::float10()));
            float_formats.insert(128, Arc::new(FloatFormat::float16()));
        }

        let mut source_files = Map::default();

        if version >= 3
            && matches!(
                children.peek().map(|node| node.tag_name().name()),
                Some("sourcefiles")
            )
        {
            let sources = children.next().unwrap();
            for source in sources.children().filter(xml::Node::is_element) {
                let name = source.attribute_string("name")?;
                let index = source.attribute_int("index")?;

                source_files.insert(name, index);
            }
        }

        let manager = SpaceManager::from_xml(
            children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("spaces not defined"))?,
        )?;

        let symbol_table = SymbolTable::from_xml(
            &manager,
            children
                .next()
                .ok_or_else(|| DeserialiseError::Invariant("symbol table not defined"))?,
        )?;

        let register_space = manager.register_space();
        let program_counter_vnd = VarnodeData::new(&*register_space, 0, 0);

        let global_scope = Arc::new(
            symbol_table
                .global_scope()
                .ok_or_else(|| DeserialiseError::Invariant("global scope not defined"))?
                .to_owned(),
        );

        let root = Arc::new(
            symbol_table
                .global_scope()
                .ok_or_else(|| DeserialiseError::Invariant("global scope not defined"))?
                .find("instruction", &symbol_table)
                .ok_or_else(|| DeserialiseError::Invariant("instruction root symbol not defined"))?
                .to_owned(),
        );

        let mut slf = Self {
            alignment,
            big_endian,
            unique_base,
            unique_mask,
            maximum_delay,
            section_count,
            float_formats,
            manager,
            symbol_table,
            root,
            global_scope,
            registers: Arc::new(RegisterNames::new(register_space)),
            registers_size: 0,
            program_counter: program_counter_vnd,
            user_ops: Vec::new(),
            context_db: ContextDatabase::new(),
            architecture: architecture.clone(),
            compiler_conventions: Map::default(),
            source_files,
        };

        slf.build_xrefs(program_counter, compiler_specs)?;

        Ok(slf)
    }

    pub fn disassemble<'a, 'z>(
        &'a self,
        db: &mut ContextDatabase,
        builder: &'z IRBuilderArena,
        address: AddressValue,
        bytes: &[u8],
    ) -> Result<Instruction<'z>, Error> {
        let arena = IRBuilderArena::with_capacity(4096);
        let mut ctxt = ParserContext::empty(&arena, self.manager());
        self.disassemble_with(db, &mut ctxt, &arena, builder, address, bytes)
    }

    pub fn disassemble_cached_with<'a, 'az, 'z>(
        &'a self,
        db: &mut ContextDatabase,
        context: &mut ParserContext<'a, 'az>,
        arena: &'az IRBuilderArena,
        builder: &'z IRBuilderArena,
        cache: &mut Map<[u8; 2], Instruction<'z>>,
        address: AddressValue,
        bytes: &[u8],
    ) -> Result<Instruction<'z>, Error> {
        if bytes.len() >= 2 {
            if let Some(insn) = cache.get(&bytes[..2]) {
                return Ok(insn.clone());
            }
        }

        match self.disassemble_with(db, context, arena, builder, address, bytes) {
            Ok(insn) if insn.length() == 2 => {
                cache.insert(<[u8; 2]>::try_from(&bytes[..2]).unwrap(), insn.clone());
                Ok(insn)
            }
            r => r,
        }
    }

    pub fn disassemble_aux<'a, 'az, 'c, T, E, F>(
        &'a self,
        db: &mut ContextDatabase,
        context: &'c mut ParserContext<'a, 'az>,
        arena: &'az IRBuilderArena,
        address: AddressValue,
        bytes: &[u8],
        mut f: F,
    ) -> Result<T, E>
    where
        F: FnMut(InstructionFormatter<'a, 'c, 'az>, usize, usize) -> Result<T, E>,
        E: From<Error>,
    {
        if self.alignment() != 1 {
            if address.offset() % self.alignment() as u64 != 0 {
                return Err(DisassemblyError::IncorrectAlignment {
                    address: address.offset(),
                    alignment: self.alignment(),
                })
                .map_err(Error::from)?;
            }
        }

        context.reinitialise(arena, db, address.clone(), bytes);
        let mut walker = ParserWalker::new(context);

        Translator::resolve(&mut walker, self.root.id(), &self.symbol_table)?;
        Translator::resolve_handles(&mut walker, &self.manager, &self.symbol_table)?;

        walker.base_state();
        walker
            .apply_commits(db, &self.manager, &self.symbol_table)
            .map_err(Error::from)?;

        let delay_slots = walker.delay_slot();
        let length = walker.length();

        let ctor = walker.unchecked_constructor();

        f(
            InstructionFormatter::new(walker, &self.symbol_table, ctor),
            delay_slots,
            length,
        )
    }

    pub fn disassemble_with<'a, 'az, 'z>(
        &'a self,
        db: &mut ContextDatabase,
        context: &mut ParserContext<'a, 'az>,
        arena: &'az IRBuilderArena,
        builder: &'z IRBuilderArena,
        address: AddressValue,
        bytes: &[u8],
    ) -> Result<Instruction<'z>, Error> {
        self.disassemble_aux(db, context, arena, address, bytes, |fmt, delay_slots, length| {
            let mnemonic = fmt.mnemonic_str(builder);
            let operands = fmt.operands_str(builder);

            Ok(Instruction {
                address,
                mnemonic,
                operands,
                delay_slots,
                length,
            })
        })
    }

    pub fn disassemble_full<'a, 'az, 'z>(
        &'a self,
        db: &mut ContextDatabase,
        context: &mut ParserContext<'a, 'az>,
        arena: &'az IRBuilderArena,
        builder: &'z IRBuilderArena,
        address: AddressValue,
        bytes: &[u8],
    ) -> Result<InstructionFull<'a, 'z>, Error> {
        self.disassemble_aux(db, context, arena, address, bytes, |fmt, delay_slots, length| {
            let mnemonic = fmt.mnemonic_str(builder);
            let operands = fmt.operands_str(builder);
            let operand_data = fmt.operand_data(builder);

            Ok(InstructionFull {
                address,
                mnemonic,
                operands,
                operand_data,
                delay_slots,
                length,
            })
        })
    }

    pub fn lift_pcode_raw<'z>(
        &self,
        db: &mut ContextDatabase,
        builder: &'z IRBuilderArena,
        address: AddressValue,
        bytes: &[u8],
    ) -> Result<PCodeRaw<'z>, Error> {
        let arena = IRBuilderArena::with_capacity(1024);
        let mut context = ParserContext::empty(&arena, self.manager());
        let mut base = builder.builder(self);
        self.lift_pcode_raw_with(db, &mut context, &arena, &mut base, address, bytes)
    }

    pub fn lift_pcode_raw_with<'a, 'az, 'z>(
        &'a self,
        db: &mut ContextDatabase,
        context: &mut ParserContext<'a, 'az>,
        arena: &'az IRBuilderArena,
        builder: &mut IRBuilderBase<'a, 'z>,
        address: AddressValue,
        bytes: &[u8],
    ) -> Result<PCodeRaw<'z>, Error> {
        if self.alignment != 1 {
            if address.offset() % self.alignment as u64 != 0 {
                return Err(DisassemblyError::IncorrectAlignment {
                    address: address.offset(),
                    alignment: self.alignment,
                })?;
            }
        }

        // Main instruction
        context.reinitialise(arena, db, address.clone(), bytes);
        let mut walker = ParserWalker::new(context);

        Translator::resolve(&mut walker, self.root.id(), &self.symbol_table)?;
        Translator::resolve_handles(&mut walker, &self.manager, &self.symbol_table)?;

        walker.base_state();
        walker.apply_commits(db, &self.manager, &self.symbol_table)?;

        let mut fall_offset = walker.length();

        let delay_slots = walker.delay_slot();
        let mut delay_contexts = Map::default();

        if delay_slots > 0 {
            let mut byte_count = 0;
            loop {
                let mut dcontext = ParserContext::new(
                    arena,
                    db,
                    address.clone() + fall_offset,
                    &bytes[fall_offset..],
                );
                let mut dwalker = ParserWalker::new(&mut dcontext);

                Translator::resolve(&mut dwalker, self.root.id(), &self.symbol_table)?;
                Translator::resolve_handles(&mut dwalker, &self.manager, &self.symbol_table)?;

                dwalker.base_state();
                dwalker.apply_commits(db, &self.manager, &self.symbol_table)?;

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
            let tmpl = ctor.unchecked_template();
            let mut builder =
                IRBuilder::new(builder, ParserWalker::new(context), &mut delay_contexts);
            builder.build(tmpl, None, &self.symbol_table)?;
            builder.resolve_relatives()?;
            Ok(builder.emit_raw(fall_offset))
        } else {
            Ok(PCodeRaw::nop_in(builder.arena(), address, walker.length()))
        }
    }

    pub fn lift_pcode(
        &self,
        db: &mut ContextDatabase,
        address: AddressValue,
        bytes: &[u8],
    ) -> Result<PCode, Error> {
        let arena = IRBuilderArena::with_capacity(4096);
        let mut context = ParserContext::empty(&arena, self.manager());
        self.lift_pcode_with(db, &mut context, &arena, address, bytes)
    }

    pub fn lift_pcode_with<'a, 'az>(
        &'a self,
        db: &mut ContextDatabase,
        context: &mut ParserContext<'a, 'az>,
        arena: &'az IRBuilderArena,
        address: AddressValue,
        bytes: &[u8],
    ) -> Result<PCode, Error> {
        if self.alignment != 1 {
            if address.offset() % self.alignment as u64 != 0 {
                return Err(DisassemblyError::IncorrectAlignment {
                    address: address.offset(),
                    alignment: self.alignment,
                })?;
            }
        }

        // Main instruction
        // Parse the pcode of the current instruction

        context.reinitialise(arena, db, address.clone(), bytes);
        let mut walker = ParserWalker::new(context);

        Translator::resolve(&mut walker, self.root.id(), &self.symbol_table)?;
        Translator::resolve_handles(&mut walker, &self.manager, &self.symbol_table)?;

        walker.base_state();
        walker.apply_commits(db, &self.manager, &self.symbol_table)?;

        let mut fall_offset = walker.length();

        let delay_slots = walker.delay_slot();
        let mut delay_contexts = Map::default();

        if delay_slots > 0 {
            let mut byte_count = 0;
            loop {
                let mut dcontext = ParserContext::new(
                    arena,
                    db,
                    address.clone() + fall_offset,
                    &bytes[fall_offset..],
                );
                let mut dwalker = ParserWalker::new(&mut dcontext);

                Translator::resolve(&mut dwalker, self.root.id(), &self.symbol_table)?;
                Translator::resolve_handles(&mut dwalker, &self.manager, &self.symbol_table)?;

                dwalker.base_state();
                dwalker.apply_commits(db, &self.manager, &self.symbol_table)?;

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
            let tmpl = ctor.unchecked_template();
            let mut base = arena.builder(self);
            let mut builder =
                IRBuilder::new(&mut base, ParserWalker::new(context), &mut delay_contexts);
            builder.build(tmpl, None, &self.symbol_table)?;
            builder.resolve_relatives()?;
            Ok(builder.emit_pcode(fall_offset))
        } else {
            Ok(PCode::nop(address, walker.length()))
        }
    }

    pub fn lift_ecode(
        &self,
        db: &mut ContextDatabase,
        address: AddressValue,
        bytes: &[u8],
    ) -> Result<ECode, Error> {
        let arena = IRBuilderArena::with_capacity(1024);
        let mut context = ParserContext::empty(&arena, self.manager());
        self.lift_ecode_with(db, &mut context, &arena, address, bytes)
    }

    pub fn lift_ecode_with<'a, 'az>(
        &'a self,
        db: &mut ContextDatabase,
        context: &mut ParserContext<'a, 'az>,
        arena: &'az IRBuilderArena,
        address: AddressValue,
        bytes: &[u8],
    ) -> Result<ECode, Error> {
        if self.alignment != 1 {
            if address.offset() % self.alignment as u64 != 0 {
                return Err(DisassemblyError::IncorrectAlignment {
                    address: address.offset(),
                    alignment: self.alignment,
                })?;
            }
        }

        // Main instruction
        // let mut context = ParserContext::new(db, address.clone(), bytes);
        context.reinitialise(arena, db, address.clone(), bytes);
        let mut walker = ParserWalker::new(context);

        Translator::resolve(&mut walker, self.root.id(), &self.symbol_table)?;
        Translator::resolve_handles(&mut walker, &self.manager, &self.symbol_table)?;

        walker.base_state();
        walker.apply_commits(db, &self.manager, &self.symbol_table)?;

        let mut fall_offset = walker.length();

        let delay_slots = walker.delay_slot();
        let mut delay_contexts = Map::default();

        if delay_slots > 0 {
            let mut byte_count = 0;
            loop {
                let mut dcontext = ParserContext::new(
                    arena,
                    db,
                    address.clone() + fall_offset,
                    &bytes[fall_offset..],
                );
                let mut dwalker = ParserWalker::new(&mut dcontext);

                Translator::resolve(&mut dwalker, self.root.id(), &self.symbol_table)?;
                Translator::resolve_handles(&mut dwalker, &self.manager, &self.symbol_table)?;

                dwalker.base_state();
                dwalker.apply_commits(db, &self.manager, &self.symbol_table)?;

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
            let tmpl = ctor.unchecked_template();
            let mut base = arena.builder(self);
            let mut builder =
                IRBuilder::new(&mut base, ParserWalker::new(context), &mut delay_contexts);
            builder.build(tmpl, None, &self.symbol_table)?;
            builder.resolve_relatives()?;
            Ok(builder.emit_ecode(fall_offset))
        } else {
            Ok(ECode::nop(address, walker.length()))
        }
    }

    fn resolve_handles<'b, 'c, 'z>(
        walker: &mut ParserWalker<'b, 'c, 'z>,
        manager: &'b SpaceManager,
        symbol_table: &'b SymbolTable,
    ) -> Result<(), Error> {
        // assumes resolve has resolved all constructors
        walker.base_state();

        while walker.is_state() {
            let ct = walker.unchecked_constructor();

            let nops = ct.operand_count();
            let mut op = walker.operand();

            'inner: while op < nops {
                let operand = symbol_table.unchecked_symbol(ct.operand(op));
                //.ok_or_else(|| DisassemblyError::InvalidSymbol)?;

                walker.unchecked_push_operand(op);

                if let Some(tsym) = operand.defining_symbol(symbol_table) {
                    if tsym.is_subtable() {
                        break 'inner;
                    } else {
                        let h = tsym.fixed_handle(walker, manager, symbol_table)?;
                        walker.set_parent_handle(h);
                    }
                } else {
                    let pexp = unsafe { operand.defining_expression().unsafe_unwrap() };
                    let res = pexp.value(walker, symbol_table)?;
                    let const_space = manager.constant_space_ref();
                    if let Some(handle) = walker.parent_handle_mut() {
                        handle.space = const_space;
                        handle.offset_space = None;
                        handle.offset_offset = res as u64;
                        handle.size = 0;
                    } else {
                        let mut handle = FixedHandle::new(const_space);
                        handle.offset_space = None;
                        handle.offset_offset = res as u64;
                        handle.size = 0;
                        walker.set_parent_handle(handle);
                    }
                }
                walker.unchecked_pop_operand();
                op += 1;
            }
            if op >= nops {
                if let Some(templ) = ct.template() {
                    if let Some(res) = templ.result() {
                        let h = res.fix(walker, manager);
                        walker.set_parent_handle(h);
                    }
                }
                walker.unchecked_pop_operand();
            }
        }

        walker.set_state(ParserState::PCode);

        Ok(())
    }

    fn resolve<'b, 'c, 'z>(
        walker: &mut ParserWalker<'b, 'c, 'z>,
        root: usize,
        symbol_table: &'b SymbolTable,
    ) -> Result<(), Error> {
        let ctor = symbol_table.resolve(root, walker)?;
        walker.set_constructor(ctor);
        ctor.apply_context(walker, symbol_table)?;

        while walker.is_state() {
            let ct = walker.unchecked_constructor();
            let nops = ct.operand_count();
            let mut op = walker.operand();

            'inner: while op < nops {
                let operand = symbol_table.unchecked_symbol(ct.operand(op));
                //.ok_or_else(|| DisassemblyError::InvalidSymbol)?;

                let offset = walker.offset(operand.offset_base()) + operand.relative_offset();

                walker.unchecked_allocate_operand(op);
                walker.set_offset(offset)?;

                if let Some(tsym) = operand.defining_symbol(symbol_table) {
                    if let Some(sub_ct) = tsym.resolve(walker, symbol_table)? {
                        walker.set_constructor(sub_ct);
                        sub_ct.apply_context(walker, symbol_table)?;
                        break 'inner;
                    }
                }
                walker.set_current_length(operand.minimum_length()?);
                walker.unchecked_pop_operand();
                op += 1;
            }
            if op >= nops {
                walker.calculate_length(ct.minimum_length(), nops); //?;
                walker.unchecked_pop_operand();

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

    use super::*;

    #[test]
    #[ignore = "test for bug"]
    fn test_aarch64_bug() -> Result<(), Error> {
        let mut translator = Translator::from_file(
            "pc",
            &ArchitectureDef::new("AARCH64", Endian::Little, 64, "v8A"),
            &Map::default(),
            "./data/processors/AARCH64/AARCH64.sla",
        )?;

        translator.set_variable_default("ShowPAC", 0);
        translator.set_variable_default("PAC_clobber", 0);
        translator.set_variable_default("ShowBTI", 0);
        translator.set_variable_default("ShowMemTag", 0);

        let bytes = [0x20, 0xf8, 0x48, 0x4f];

        let mut db = translator.context_database();

        let addr = translator.address(0x10cbe0u64);
        let _insn = translator.lift_pcode(&mut db, addr, &bytes)?;

        Ok(())
    }

    #[test]
    #[ignore = "test arm32 bug #6"]
    fn test_arm32_bug_6() -> Result<(), Box<dyn std::error::Error>> {
        let mut translator = Translator::from_file(
            "pc",
            &ArchitectureDef::new("ARM", Endian::Little, 32, "V8T"),
            &Default::default(),
            "./data/processors/ARM/ARM8_le.sla",
        )?;

        translator.set_variable_default("TMode", 1);
        translator.set_variable_default("LRset", 0);
        translator.set_variable_default("spsr", 0);

        let bytes = [0xF5, 0xF7, 0x8C, 0xEF];

        let mut db = translator.context_database();
        let irb = IRBuilderArena::with_capacity(4096);

        let addr = translator.address(0x1000u64);
        let _insn = translator.disassemble(&mut db, &irb, addr, &bytes)?;

        Ok(())
    }

    #[test]
    #[ignore = "test arm32 bug #9"]
    fn test_arm32_bug_9() -> Result<(), Box<dyn std::error::Error>> {
        let mut translator = Translator::from_file(
            "pc",
            &ArchitectureDef::new("ARM", Endian::Little, 32, "V8T"),
            &Default::default(),
            "./data/processors/ARM/ARM8_le.sla",
        )?;

        translator.set_variable_default("TMode", 1);
        translator.set_variable_default("LRset", 0);
        translator.set_variable_default("spsr", 0);

        let bytes = [0xf5, 0xf7, 0x8c, 0xef, 0x00, 0xb1, 0x08, 0xbd];

        let mut db = translator.context_database();
        let irb = IRBuilderArena::with_capacity(4096);

        let addr = translator.address(0x0u64);
        let mut offset = 0;
        while offset < bytes.len() {
            let insn = translator.lift_pcode_raw(&mut db, &irb, addr + offset, &bytes[offset..])?;
            println!("{}", insn.display(&translator));
            offset += insn.length();
        }

        let mut db = translator.context_database();
        let addr = translator.address(0xb000u64);
        let mut offset = 0;
        while offset < bytes.len() {
            let insn = translator.lift_pcode_raw(&mut db, &irb, addr + offset, &bytes[offset..])?;
            println!("{}", insn.display(&translator));
            offset += insn.length();
        }

        Ok(())
    }

    #[test]
    #[ignore = "test sha2a bug #4"]
    fn test_sh2a_bug_4() -> Result<(), Box<dyn std::error::Error>> {
        let translator = Translator::from_file(
            "pc",
            &ArchitectureDef::new("SuperH", Endian::Big, 32, "SH-2A"),
            &Default::default(),
            "./data/processors/SuperH/sh-2a.sla",
        )?;

        let bytes = [0xe2, 0xf5, 0x40, 0x08, 0x44, 0x2d];

        let mut db = translator.context_database();
        let irb = IRBuilderArena::with_capacity(4096);

        let addr = translator.address(0xd92u64);
        let mut offset = 0;
        while offset < bytes.len() {
            let insn = translator.lift_pcode_raw(&mut db, &irb, addr + offset, &bytes[offset..])?;
            println!("{}", insn.display(&translator));
            offset += insn.length();
        }

        Ok(())
    }
}
