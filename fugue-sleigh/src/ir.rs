use std::cmp::Ordering;
use std::collections::hash_map::Entry;

use fugue_ir::disassembly::{ArenaVec, IRBuilderArena, Opcode, PCodeRaw};
use fugue_ir::{AddressSpace, Translator, VarnodeData};

use thiserror::Error;
use ustr::{Ustr, UstrMap};

use crate::ast::{AstError, CodeBlock, Stmt};
use crate::cfg::CFG;

#[derive(Debug, Error)]
pub enum IRBuilderError {
    #[error("redefinition of temporary {name}")]
    LocalDup { name: Ustr },
    #[error("inconsistent size for {name}: {old_size} vs {size}")]
    LocalSize {
        name: Ustr,
        size: usize,
        old_size: usize,
    },
    #[error("redefinition of register {name}")]
    RegDup { name: Ustr },
    #[error(transparent)]
    Parse(#[from] AstError),
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct IRBlock {
    stmts: Vec<IRStmt>,
}

impl IRBlock {
    pub fn emit(&mut self, op: Opcode, inputs: Vec<IRValue>, output: Option<IRValue>) {
        self.stmts.push(IRStmt { op, inputs, output })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum IRValue {
    Constant(u64),
    Register(VarnodeData),
    Temporary(LocalId),
    Label(Ustr),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum IRExpr {
    Local {
        value: IRValue,
    },
    Const {
        value: u64,
        size: Option<u32>,
    },
    UnOp {
        op: Opcode,
        value: IRValue,
    },
    BinOp {
        op: Opcode,
        lvalue: IRValue,
        rvalue: IRValue,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IRStmt {
    op: Opcode,
    inputs: Vec<IRValue>,
    output: Option<IRValue>,
}

pub struct IRBuilder<'a> {
    emitted: IRBlock,
    locals: Locals<'a>,
    translator: &'a Translator,
}

pub type LocalId = usize;

pub struct LocalVar {
    name: Option<Ustr>,
    size: Option<u32>,
}

pub struct Locals<'a> {
    locals: Vec<LocalVar>,
    mapping: UstrMap<usize>,
    space: &'a AddressSpace,
}

impl<'a> Locals<'a> {
    pub fn new(translator: &'a Translator) -> Self {
        Self {
            locals: Vec::new(),
            mapping: UstrMap::default(),
            space: translator.manager().unique_space_ref(),
        }
    }

    pub fn new_named(&mut self, name: Ustr, size: Option<u32>) -> Result<LocalId, IRBuilderError> {
        match self.mapping.entry(name) {
            Entry::Vacant(entry) => {
                let id = self.locals.len();
                self.locals.push(LocalVar { name: Some(name), size });
                entry.insert(id);
                Ok(id)
            }
            Entry::Occupied(_) => {
                Err(IRBuilderError::LocalDup { name })
            }
        }
    }

    pub fn new_unnamed(&mut self, size: Option<u32>) -> LocalId {
        let id = self.locals.len();
        self.locals.push(LocalVar { name: None, size });
        id
    }

    pub fn get_or_insert(&mut self, name: Ustr, size: Option<u32>) -> Result<LocalId, IRBuilderError> {
        if let Some(id) = self.mapping.get(&name) {
            return Ok(*id)
        }

        self.new_named(name, size)
    }

    pub fn size_of(&self, id: LocalId) -> Option<u32> {
        self.locals.get(id).and_then(|v| v.size)
    }
}

impl<'a> IRBuilder<'a> {
    pub fn new(translator: &'a Translator) -> Self {
        Self {
            translator,
            locals: Locals::new(translator),
            emitted: IRBlock::default(),
        }
    }

    // TODO: do we take the inst_start?
    pub fn translate<'ir>(
        &mut self,
        irb: &'ir IRBuilderArena,
        input: impl AsRef<str>,
    ) -> Result<ArenaVec<'ir, PCodeRaw<'ir>>, IRBuilderError> {
        let ast = CodeBlock::parse(input.as_ref())?;
        let cfg = CFG::new(&ast);

        todo!()
    }

    fn emit_op(&mut self, op: Opcode, mut inputs: Vec<IRValue>, mut output: Option<IRValue>) -> Option<IRValue> {
        self.update_varnodes(op, &mut inputs, output.as_mut());
        self.emitted.emit(op, inputs, output);
        output
    }

    fn emit_stmt(&mut self, stmt: &Stmt) -> Result<(), IRBuilderError> {
        match stmt {
            Stmt::Assign { name, decl, size, bits, source } => {
                let target = self.resolve_name(*decl, *name, *size)?;

                todo!()

            }
            Stmt::Declare { name, size } => {
                let target = self.locals.new_named(*name, *size)?;

                todo!()
            }
            _ => todo!()
        }
    }

    fn resolve_name(&mut self, decl: bool, name: Ustr, size: Option<u32>) -> Result<IRValue, IRBuilderError> {
        if let Some(reg) = self.translator.register_by_name(name) {
            return if decl {
                Err(IRBuilderError::RegDup { name })
            } else {
                Ok(IRValue::Register(reg))
            };
        }

        if decl {
            self.locals.new_named(name, size)
        } else {
            self.locals.get_or_insert(name, size)
        }.map(IRValue::Temporary)
    }

    fn update_varnodes(&mut self, op: Opcode, inputs: &mut [IRValue], output: Option<&mut IRValue>) {
        // apply size updates
        todo!()
    }
}
