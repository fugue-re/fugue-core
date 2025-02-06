use std::collections::hash_map::Entry;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::iter::{once, repeat};
use std::mem::take;
use std::ops::{Index, Range};
use std::sync::Arc;

use fugue_ir::disassembly::{ArenaVec, IRBuilderArena, Opcode, PCodeBlock, PCodeData};
use fugue_ir::{AddressSpace, Translator, VarnodeData};

use once_cell::sync::Lazy;
use thiserror::Error;
use ustr::{ustr, Ustr, UstrMap};

use crate::ast::{AstError, BinOp, BinRel, BranchLabel, BranchTarget, CodeBlock, Expr, Stmt, UnOp};
use crate::cfg::CFG;

static BUILTINS: Lazy<UstrMap<(Opcode, usize)>> = Lazy::new(|| {
    UstrMap::from_iter(
        [
            ("carry", (Opcode::IntCarry, 2)),
            ("scarry", (Opcode::IntSCarry, 2)),
            ("sborrow", (Opcode::IntSBorrow, 2)),
            ("sext", (Opcode::IntSExt, 1)),
            ("zext", (Opcode::IntZExt, 1)),
            ("abs", (Opcode::FloatAbs, 1)),
            ("sqrt", (Opcode::FloatSqrt, 1)),
            ("ceil", (Opcode::FloatCeiling, 1)),
            ("floor", (Opcode::FloatFloor, 1)),
            ("round", (Opcode::FloatRound, 1)),
            ("nan", (Opcode::FloatIsNaN, 1)),
            ("int2float", (Opcode::FloatOfInt, 1)),
            ("float2float", (Opcode::FloatOfFloat, 1)),
            ("trunc", (Opcode::FloatTruncate, 1)),
            ("popcount", (Opcode::PopCount, 1)),
            // ("lzcount", (Opcode::LZCount, 1)),
        ]
        .into_iter()
        .map(|(n, a)| (ustr(n), a)),
    )
});

#[derive(Debug, Clone)]
pub enum LocalIdent {
    Named(Ustr),
    Unnamed(LocalId),
}

impl From<Ustr> for LocalIdent {
    fn from(value: Ustr) -> Self {
        Self::Named(value)
    }
}

impl From<LocalId> for LocalIdent {
    fn from(value: LocalId) -> Self {
        Self::Unnamed(value)
    }
}

impl Display for LocalIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Named(name) => name.fmt(f),
            Self::Unnamed(id) => write!(f, "unnamed {id}"),
        }
    }
}

#[derive(Debug, Error)]
pub enum IRBuilderError {
    #[error("attempt to calculate address of temporary {name}")]
    LocalAddr { name: LocalIdent },
    #[error("redefinition of temporary {name}")]
    LocalDup { name: LocalIdent },
    #[error("inconsistent size for {name}: {old_size} vs {size}")]
    LocalSize {
        name: LocalIdent,
        size: u32,
        old_size: u32,
    },
    #[error("use of undefined temporary {name}")]
    LocalUndef { name: LocalIdent },
    #[error("unable to infer size of temporary {name}")]
    LocalUnsized { name: LocalIdent },
    #[error("inconsistent arity for {op:?}; expected {expected}, got {actual}")]
    OpcodeArity {
        op: Opcode,
        expected: usize,
        actual: usize,
    },
    #[error("redefinition of register {name}")]
    RegDup { name: Ustr },
    #[error("inconsistent size for {name}: {old_size} vs {size}")]
    RegSize {
        name: Ustr,
        size: u32,
        old_size: u32,
    },
    #[error("bit range {}..{} of {value:?} is invalid", range.start, range.end)]
    BitRange { value: IRValue, range: Range<u32> },
    #[error("masked bit range of {value:?} produces varnode larger than 64 bits")]
    BitRangeSize { value: IRValue },
    #[error(transparent)]
    Parse(#[from] AstError),
    #[error("unknown intrinsic `{name}`")]
    UnknownIntrinsic { name: Ustr },
    #[error("unknown label `{label}`")]
    UnknownLabel { label: Ustr },
    #[error("unknown space `{space}`")]
    UnknownSpace { space: Ustr },
    #[error("unable to reconcile size of {v1:?} with expected size {size}")]
    UpdateSize1 { v1: IRValue, size: u32 },
    #[error("unable to reconcile size of {v1:?} and {v2:?}")]
    UpdateSize2 { v1: IRValue, v2: IRValue },
    #[error("unable to reconcile size of {v1:?}, {v2:?}, and {v3:?}")]
    UpdateSize3 {
        v1: IRValue,
        v2: IRValue,
        v3: IRValue,
    },
    #[error("unable to convert intermediate value {value:?} to varnode")]
    ValueToVarnode { value: IRValue },
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct IRBlock {
    stmts: Vec<IRStmt>,
}

impl IRBlock {
    pub fn emit(&mut self, op: Opcode, inputs: Vec<IRValue>, output: Option<IRValue>) {
        self.stmts.push(IRStmt { op, inputs, output })
    }

    pub fn clear(&mut self) {
        self.stmts.clear();
    }

    pub fn len(&self) -> usize {
        self.stmts.len()
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum IRValue {
    Const(u64, Option<u32>),
    Register(VarnodeData),
    Temporary(LocalId),
    Address(u64, Option<Ustr>),
    Label(Ustr),
}

impl IRValue {
    fn is_const(&self) -> bool {
        matches!(self, Self::Const(_, _))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum IRExpr {
    Var {
        value: IRValue,
        offset: u32,
        size: Option<u32>,
    },
    Const {
        value: u64,
        size: Option<u32>,
    },
    UnRel {
        op: Opcode,
        value: IRValue,
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
    BinRel {
        op: Opcode,
        lvalue: IRValue,
        rvalue: IRValue,
    },
    Load {
        space: Option<Ustr>,
        size: Option<u32>,
        source: IRValue,
    },
    Intrinsic {
        id: usize,
        arguments: Vec<IRValue>,
    },
    Subpiece {
        input: IRValue,
        bytes: u32,
    },
    AddressOf {
        value: IRValue,
        size: Option<u32>,
    },
    BitsOf {
        value: IRValue,
        range: Range<u32>,
        size: u32,
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
    locals: Locals,
    labels: UstrMap<usize>,
    default_size: u32,
    user_ops: UstrMap<usize>,
    translator: &'a Translator,
}

pub type LocalId = usize;

#[derive(Debug)]
pub struct LocalVar {
    name: Option<Ustr>,
    size: Option<u32>,
}

#[derive(Default)]
pub struct Locals {
    locals: Vec<LocalVar>,
    mapping: UstrMap<usize>,
}

impl Locals {
    pub fn new() -> Self {
        Self {
            locals: Vec::new(),
            mapping: UstrMap::default(),
        }
    }

    pub fn new_named(&mut self, name: Ustr, size: Option<u32>) -> Result<LocalId, IRBuilderError> {
        match self.mapping.entry(name) {
            Entry::Vacant(entry) => {
                let id = self.locals.len();
                self.locals.push(LocalVar {
                    name: Some(name),
                    size,
                });
                entry.insert(id);
                Ok(id)
            }
            Entry::Occupied(_) => Err(IRBuilderError::LocalDup { name: name.into() }),
        }
    }

    pub fn new_unnamed(&mut self, size: Option<u32>) -> LocalId {
        let id = self.locals.len();
        self.locals.push(LocalVar { name: None, size });
        id
    }

    pub fn get_or_insert(
        &mut self,
        name: Ustr,
        size: Option<u32>,
    ) -> Result<LocalId, IRBuilderError> {
        if let Some(id) = self.mapping.get(&name) {
            return Ok(*id);
        }

        self.new_named(name, size)
    }

    pub fn get(&self, name: Ustr) -> Option<LocalId> {
        self.mapping.get(&name).copied()
    }

    pub fn try_update_size(&mut self, id: LocalId, size: u32) -> Result<(), IRBuilderError> {
        self.update_size(id, size).ok();
        Ok(())
    }

    pub fn update_size(&mut self, id: LocalId, size: u32) -> Result<(), IRBuilderError> {
        let Some(var) = self.locals.get_mut(id) else {
            panic!("local `{id}` undefined")
        };

        if matches!(var.size, Some(old_size) if old_size != size) {
            return Err(IRBuilderError::LocalSize {
                name: var
                    .name
                    .map(LocalIdent::Named)
                    .unwrap_or(LocalIdent::Unnamed(id)),
                size,
                old_size: var.size.unwrap(),
            });
        }
        var.size = Some(size);

        Ok(())
    }

    pub fn into_varnodes(self, space: &AddressSpace) -> Result<Vec<VarnodeData>, IRBuilderError> {
        let mut offset = 0u64;
        self.locals
            .into_iter()
            .enumerate()
            .map(|(id, local)| {
                let size = local.size.ok_or_else(|| {
                    let name = local
                        .name
                        .map(LocalIdent::Named)
                        .unwrap_or(LocalIdent::Unnamed(id));
                    IRBuilderError::LocalUnsized { name }
                })? as usize;
                let varnode = VarnodeData::new(space, offset, size);
                offset += size as u64;
                Ok(varnode)
            })
            .collect::<Result<_, _>>()
    }

    pub fn size_of(&self, id: LocalId) -> Option<u32> {
        self.locals.get(id).and_then(|v| v.size)
    }

    pub fn clear(&mut self) {
        self.locals.clear();
        self.mapping.clear();
    }
}

impl Index<LocalId> for Locals {
    type Output = LocalVar;

    fn index(&self, index: LocalId) -> &LocalVar {
        &self.locals[index]
    }
}

impl<'a> IRBuilder<'a> {
    pub fn new(translator: &'a Translator) -> Self {
        Self {
            default_size: translator.manager().default_space().address_size() as _,
            user_ops: translator
                .user_ops()
                .iter()
                .copied()
                .enumerate()
                .map(|(i, v)| (v, i))
                .collect(),
            translator,
            locals: Locals::new(),
            labels: UstrMap::default(),
            emitted: IRBlock::default(),
        }
    }

    pub fn translate_parsed<'ir>(
        &mut self,
        irb: &'ir IRBuilderArena,
        ast: &CodeBlock,
    ) -> Result<PCodeBlock<'ir>, IRBuilderError> {
        let cfg = CFG::new(ast)?;

        self.emitted.clear();
        self.locals.clear();
        self.labels.clear();

        let mut posn_to_labels = BTreeMap::<usize, Vec<Ustr>>::new();

        for (label, block) in cfg.labels().iter() {
            posn_to_labels.entry(block).or_default().push(label);
        }

        for (block_id, block) in cfg.blocks().iter().enumerate() {
            if let Some(labels) = posn_to_labels.remove(&block_id) {
                self.labels
                    .extend(labels.into_iter().zip(repeat(self.emitted.len())));
            }
            for stmt in block.iter() {
                self.resolve_stmt(stmt)?;
            }
        }

        let mut emitted = take(&mut self.emitted);

        for (i, emitted) in emitted.stmts.iter_mut().enumerate() {
            // attempt to resolve labels into relatives
            self.update_varnode_labels(i, &mut emitted.inputs)?;
            // attempt to infer missing sizes with fallback to default size
            self.update_varnodes(
                emitted.op,
                &mut emitted.inputs,
                emitted.output.as_mut(),
                true,
            )?;
            // attempt to fill unsized values with default sizes (covers cases without inference)
            self.update_varnode_defaults(&mut emitted.inputs, emitted.output.as_mut())?;
        }

        self.to_pcode(irb, emitted)
    }

    pub fn translate<'ir>(
        &mut self,
        irb: &'ir IRBuilderArena,
        input: impl AsRef<str>,
    ) -> Result<PCodeBlock<'ir>, IRBuilderError> {
        let ast = CodeBlock::parse(input.as_ref())?;
        self.translate_parsed(irb, &ast)
    }

    fn to_varnode(
        &mut self,
        locals: &[VarnodeData],
        value: IRValue,
    ) -> Result<VarnodeData, IRBuilderError> {
        let vnd = match value {
            IRValue::Const(offset, Some(size)) => VarnodeData::new(
                self.translator.manager().constant_space_ref(),
                offset,
                size as _,
            ),
            IRValue::Register(vnd) => vnd,
            IRValue::Temporary(id) => locals[id],
            IRValue::Address(offset, _) => VarnodeData::new(
                self.translator.manager().default_space_ref(),
                offset,
                self.default_size as _,
            ),
            _ => return Err(IRBuilderError::ValueToVarnode { value }),
        };
        Ok(vnd)
    }

    fn to_pcode_data<'ir>(
        &mut self,
        irb: &'ir IRBuilderArena,
        locals: &[VarnodeData],
        stmt: IRStmt,
    ) -> Result<PCodeData<'ir>, IRBuilderError> {
        let output = stmt
            .output
            .map(|value| self.to_varnode(locals, value))
            .transpose()?;
        let mut inputs = ArenaVec::new_in(irb);

        for input in stmt.inputs {
            inputs.push(self.to_varnode(locals, input)?);
        }

        Ok(PCodeData {
            opcode: stmt.op,
            inputs,
            output,
        })
    }

    fn to_pcode<'ir>(
        &mut self,
        irb: &'ir IRBuilderArena,
        emitted: IRBlock,
    ) -> Result<PCodeBlock<'ir>, IRBuilderError> {
        let locals =
            take(&mut self.locals).into_varnodes(self.translator.manager().unique_space_ref())?;
        let mut block = PCodeBlock::new_in(irb);

        for stmt in emitted.stmts {
            block.push(self.to_pcode_data(irb, &locals, stmt)?);
        }

        Ok(block)
    }

    fn emit_op(
        &mut self,
        op: Opcode,
        mut inputs: Vec<IRValue>,
        mut output: Option<IRValue>,
    ) -> Result<Option<IRValue>, IRBuilderError> {
        self.update_varnodes(op, &mut inputs, output.as_mut(), false)?;
        self.emitted.emit(op, inputs, output.clone());
        Ok(output)
    }

    fn emit_copy(&mut self, input: IRValue, output: IRValue) -> Result<IRValue, IRBuilderError> {
        let output = self
            .emit_op(Opcode::Copy, vec![input], Some(output))?
            .expect("has output");
        Ok(output)
    }

    fn emit_with_output(
        &mut self,
        op: Opcode,
        inputs: Vec<IRValue>,
        output: Option<IRValue>,
    ) -> Result<IRValue, IRBuilderError> {
        let output = output.unwrap_or_else(|| self.new_local(None));
        let output = self.emit_op(op, inputs, Some(output))?.expect("has output");
        Ok(output)
    }

    fn emit_unrel(
        &mut self,
        op: Opcode,
        input: IRValue,
        output: Option<IRValue>,
    ) -> Result<IRValue, IRBuilderError> {
        self.emit_with_output(op, vec![input], output)
    }

    fn emit_unop(
        &mut self,
        op: Opcode,
        input: IRValue,
        output: Option<IRValue>,
    ) -> Result<IRValue, IRBuilderError> {
        self.emit_with_output(op, vec![input], output)
    }

    fn emit_binop(
        &mut self,
        op: Opcode,
        lvalue: IRValue,
        rvalue: IRValue,
        output: Option<IRValue>,
    ) -> Result<IRValue, IRBuilderError> {
        self.emit_with_output(op, vec![lvalue, rvalue], output)
    }

    fn emit_binrel(
        &mut self,
        op: Opcode,
        lvalue: IRValue,
        rvalue: IRValue,
        output: Option<IRValue>,
    ) -> Result<IRValue, IRBuilderError> {
        self.emit_with_output(op, vec![lvalue, rvalue], output)
    }

    fn emit_load(
        &mut self,
        space: Option<Ustr>,
        size: Option<u32>,
        mut source: IRValue,
        output: Option<IRValue>,
    ) -> Result<IRValue, IRBuilderError> {
        let space = self.resolve_space(space)?;
        let space_id = IRValue::Const(space.index() as _, None);

        source = self.resize(source, self.default_size, None)?;

        let mut output = self.emit_with_output(Opcode::Load, vec![space_id, source], output)?;

        if let Some(sz) = size {
            self.update_varnode(&mut output, sz)?;
        }

        Ok(output)
    }

    fn resize(
        &mut self,
        value: IRValue,
        size: u32,
        output: Option<IRValue>,
    ) -> Result<IRValue, IRBuilderError> {
        if matches!(self.size_of(&value), Some(actual) if actual != size) {
            let target = output.unwrap_or_else(|| self.new_local(Some(size)));
            // NOTE: maybe truncate??
            self.emit_unop(Opcode::IntZExt, value, Some(target))
        } else {
            Ok(value)
        }
    }

    fn new_named_local(
        &mut self,
        name: Ustr,
        size: Option<u32>,
    ) -> Result<IRValue, IRBuilderError> {
        self.locals.new_named(name, size).map(IRValue::Temporary)
    }

    fn new_local(&mut self, size: Option<u32>) -> IRValue {
        IRValue::Temporary(self.locals.new_unnamed(size))
    }

    fn resolve_stmt(&mut self, stmt: &Stmt) -> Result<(), IRBuilderError> {
        match stmt {
            Stmt::Assign {
                name,
                decl,
                size,
                bits,
                source,
            } => {
                let source = self.resolve_expr(source)?;
                let mut bits = bits.clone();

                // convert truncated assignment into bit range assignment
                if bits.is_none() {
                    if let Ok(target) = self.resolve_existing_name(*name) {
                        let existing = self.size_of(&target);

                        if matches!(existing.zip(*size), Some((s1, s2)) if s1 != s2) {
                            let nbits = size.unwrap() * 8;
                            bits = Some(0..nbits);
                        }
                    }
                }

                if let Some(range) = bits {
                    let target = self.resolve_name(*decl, *name, None)?;
                    let size = size.expect("bit ranges have a known size");
                    let bits = range.end - range.start;

                    let mask = !((2u64 << bits.wrapping_sub(1)).wrapping_sub(1) << range.start);
                    let shift_needed = range.start != 0;
                    let mut zext_needed = true;

                    if let Some(known_size) = self.size_of(&target) {
                        let known_bits = known_size * 8;

                        zext_needed = known_size > size;

                        if range.start >= known_bits || range.end > known_bits {
                            return Err(IRBuilderError::BitRange {
                                value: target,
                                range,
                            });
                        }

                        if range.start == 0 && bits == known_bits {
                            // superfluous
                            self.expr_to_value(source, Some(target))?;
                            return Ok(());
                        }
                    }

                    if range.end > 64 {
                        return Err(IRBuilderError::BitRangeSize { value: target });
                    }

                    let mut source = self.expr_to_value(source, None)?;

                    source =
                        self.emit_binop(Opcode::IntAnd, source, IRValue::Const(mask, None), None)?;

                    if zext_needed {
                        source = self.emit_unop(Opcode::IntZExt, source, None)?;
                    }

                    if shift_needed {
                        source = self.emit_binop(
                            Opcode::IntLShift,
                            source,
                            IRValue::Const(range.start as _, None),
                            None,
                        )?;
                    }

                    self.emit_binop(Opcode::IntOr, target, source, Some(target))?;
                } else {
                    let mut target = self.resolve_name(*decl, *name, *size)?;

                    if let Some(size) = size {
                        self.update_varnode(&mut target, *size)?;
                    }

                    self.expr_to_value(source, Some(target))?;
                }

                Ok(())
            }
            Stmt::Declare { name, size } => {
                self.new_named_local(*name, *size)?;
                Ok(())
            }
            Stmt::Store {
                space,
                size,
                target,
                source,
            } => self.resolve_store(*space, *size, target, source),
            Stmt::Branch { target } => self.resolve_branch(target),
            Stmt::CBranch { target, condition } => self.resolve_cbranch(target, condition),
            Stmt::Call { target } => self.resolve_call(target),
            Stmt::Return { target } => self.resolve_return(target),
            Stmt::Intrinsic { name, arguments } => self.resolve_intrinsic(*name, arguments),
            Stmt::Label { .. } => unreachable!("labels are removed from the IR at this point"),
        }
    }

    fn resolve_store(
        &mut self,
        space: Option<Ustr>,
        size: Option<u32>,
        target: &Expr,
        source: &Expr,
    ) -> Result<(), IRBuilderError> {
        let source = self.resolve_expr(source)?;
        let target = self.resolve_expr(target)?;

        // store => index, target, source

        let space = self.resolve_space(space)?;

        let mut source_vnd = self.expr_to_value(source, None)?;
        let mut target_vnd = self.expr_to_value(target, None)?;

        if let Some(size) = size {
            self.update_varnode(&mut source_vnd, size)?;
        }

        target_vnd = self.resize(target_vnd, self.default_size, None)?;

        let space_id = IRValue::Const(space.index() as _, None);

        self.emit_op(Opcode::Store, vec![space_id, target_vnd, source_vnd], None)?;

        Ok(())
    }

    fn resolve_space(&mut self, space: Option<Ustr>) -> Result<Arc<AddressSpace>, IRBuilderError> {
        let manager = self.translator.manager();
        if let Some(space) = space {
            manager
                .space_by_name(space)
                .ok_or(IRBuilderError::UnknownSpace { space })
        } else {
            Ok(manager.default_space())
        }
    }

    fn resolve_branch(&mut self, target: &BranchTarget) -> Result<(), IRBuilderError> {
        let (op, target) = match target {
            BranchTarget::Label(label) => (Opcode::Branch, IRValue::Label(*label)),
            BranchTarget::Direct(target) => {
                let BranchLabel::Offset { offset, space } = target else {
                    unreachable!()
                };

                let varnode = IRValue::Address(*offset, *space);

                (Opcode::Branch, varnode)
            }
            BranchTarget::Indirect(target) => {
                let expr = self.resolve_expr(target)?;
                let value = self.expr_to_value(expr, None)?;
                (Opcode::IBranch, value)
            }
        };

        self.emit_op(op, vec![target], None)?;

        Ok(())
    }

    fn resolve_cbranch(
        &mut self,
        target: &BranchTarget,
        condition: &Expr,
    ) -> Result<(), IRBuilderError> {
        let expr = self.resolve_expr(condition)?;
        let condition = self.expr_to_value(expr, None)?;

        let target = match target {
            BranchTarget::Label(label) => IRValue::Label(*label),
            BranchTarget::Direct(target) => {
                let BranchLabel::Offset { offset, space } = target else {
                    unreachable!()
                };

                IRValue::Address(*offset, *space)
            }
            _ => unreachable!(),
        };

        self.emit_op(Opcode::CBranch, vec![target, condition], None)?;

        Ok(())
    }

    fn resolve_call(&mut self, target: &BranchTarget) -> Result<(), IRBuilderError> {
        let (op, target) = match target {
            BranchTarget::Label(label) => (Opcode::Call, IRValue::Label(*label)),
            BranchTarget::Direct(target) => {
                let BranchLabel::Offset { offset, space } = target else {
                    unreachable!()
                };

                let varnode = IRValue::Address(*offset, *space);

                (Opcode::Branch, varnode)
            }
            BranchTarget::Indirect(target) => {
                let expr = self.resolve_expr(target)?;
                let value = self.expr_to_value(expr, None)?;
                (Opcode::ICall, value)
            }
        };

        self.emit_op(op, vec![target], None)?;

        Ok(())
    }

    fn resolve_return(&mut self, target: &BranchTarget) -> Result<(), IRBuilderError> {
        let (op, target) = if let BranchTarget::Indirect(target) = target {
            let expr = self.resolve_expr(target)?;
            let value = self.expr_to_value(expr, None)?;
            (Opcode::Return, value)
        } else {
            unreachable!("non-indirect return cannot be constructed")
        };

        self.emit_op(op, vec![target], None)?;

        Ok(())
    }

    fn resolve_intrinsic(&mut self, name: Ustr, arguments: &[Expr]) -> Result<(), IRBuilderError> {
        // check if known intrinsic
        if let Some(intrinsic) = self.user_ops.get(&name) {
            let id = IRValue::Const(*intrinsic as _, None);
            let inputs = once(Ok(id))
                .chain(arguments.into_iter().map(|expr| {
                    let expr = self.resolve_expr(expr)?;
                    self.expr_to_value(expr, None)
                }))
                .collect::<Result<Vec<_>, _>>()?;

            self.emit_op(Opcode::CallOther, inputs, None)?;

            Ok(())
        } else {
            // as a stmt
            Err(IRBuilderError::UnknownIntrinsic { name })
        }
    }

    fn resolve_expr_intrinsic(
        &mut self,
        name: Ustr,
        arguments: &[Expr],
    ) -> Result<IRExpr, IRBuilderError> {
        if let Some(id) = self.user_ops.get(&name).copied() {
            let arguments = arguments
                .into_iter()
                .map(|expr| {
                    let expr = self.resolve_expr(expr)?;
                    self.expr_to_value(expr, None)
                })
                .collect::<Result<Vec<_>, _>>()?;

            return Ok(IRExpr::Intrinsic { id, arguments });
        }

        let expr = if let Some((op, expected_arity)) = BUILTINS.get(&name).copied() {
            let actual_arity = arguments.len();
            if expected_arity != actual_arity {
                return Err(IRBuilderError::OpcodeArity {
                    op,
                    expected: expected_arity,
                    actual: actual_arity,
                });
            }

            let mut inputs = arguments.into_iter().map(|expr| {
                let expr = self.resolve_expr(expr)?;
                self.expr_to_value(expr, None)
            });

            match op {
                Opcode::IntCarry | Opcode::IntSCarry | Opcode::IntSBorrow => IRExpr::BinRel {
                    op,
                    lvalue: inputs.next().unwrap()?,
                    rvalue: inputs.next().unwrap()?,
                },
                Opcode::IntZExt
                | Opcode::IntSExt
                | Opcode::PopCount
                | Opcode::FloatAbs
                | Opcode::FloatSqrt
                | Opcode::FloatCeiling
                | Opcode::FloatFloor
                | Opcode::FloatRound
                | Opcode::FloatOfInt
                | Opcode::FloatOfFloat
                | Opcode::FloatTruncate => IRExpr::UnOp {
                    op,
                    value: inputs.next().unwrap()?,
                },
                Opcode::FloatIsNaN => IRExpr::UnRel {
                    op,
                    value: inputs.next().unwrap()?,
                },
                _ => unreachable!(),
            }
        } else {
            // check if subpiece operation, e.g., r1(4)
            if arguments.len() != 1 {
                return Err(IRBuilderError::UnknownIntrinsic { name });
            }

            let Expr::Literal { value, size: None } = &arguments[0] else {
                return Err(IRBuilderError::UnknownIntrinsic { name });
            };

            let Ok(varnode) = self.resolve_existing_name(name) else {
                return Err(IRBuilderError::UnknownIntrinsic { name });
            };

            IRExpr::Subpiece {
                input: varnode,
                bytes: *value as _,
            }
        };

        Ok(expr)
    }

    fn expr_to_value(
        &mut self,
        expr: IRExpr,
        output: Option<IRValue>,
    ) -> Result<IRValue, IRBuilderError> {
        match expr {
            IRExpr::Var {
                mut value,
                offset: _todo, // TODO: bit range
                size,
            } => {
                // NOTE: this implies truncation
                if let Some(size) = size {
                    self.update_varnode(&mut value, size)?;
                }
                output.map_or(Ok(value), |output| self.emit_copy(value, output))
            }
            IRExpr::Const { value, size } => {
                let value = IRValue::Const(value, size);
                output.map_or(Ok(value), |output| self.emit_copy(value, output))
            }
            IRExpr::UnRel { op, value } => self.emit_unrel(op, value, output),
            IRExpr::UnOp { op, value } => self.emit_unop(op, value, output),
            IRExpr::BinOp { op, lvalue, rvalue } => self.emit_binop(op, lvalue, rvalue, output),
            IRExpr::BinRel { op, lvalue, rvalue } => self.emit_binrel(op, lvalue, rvalue, output),
            IRExpr::Load {
                space,
                size,
                source,
            } => self.emit_load(space, size, source, output),
            IRExpr::Intrinsic { id, arguments } => self.emit_with_output(
                Opcode::CallOther,
                once(IRValue::Const(id as _, None))
                    .chain(arguments)
                    .collect(),
                output,
            ),
            IRExpr::Subpiece { input, bytes } => self.emit_with_output(
                Opcode::Subpiece,
                vec![input, IRValue::Const(bytes as _, None)],
                output,
            ),
            IRExpr::AddressOf { value, size } => {
                let value = self.address_of(value, size)?;
                output.map_or(Ok(value), |output| self.emit_copy(value, output))
            }
            IRExpr::BitsOf {
                mut value,
                mut range,
                size,
            } => {
                let bits = range.end - range.start;
                let mut mask_needed = bits % 8 != 0;
                let mut trunc_needed = false;

                if let Some(known_size) = self.size_of(&value) {
                    let known_bits = known_size * 8;

                    if range.start >= known_bits || range.end > known_bits {
                        return Err(IRBuilderError::BitRange { value, range });
                    }

                    trunc_needed = size < known_size;

                    if mask_needed && range.end == known_bits {
                        mask_needed = false;
                    }
                }

                let mask = (2u64 << bits.wrapping_sub(1)).wrapping_sub(1);
                let mut trunc_shift = 0;

                if trunc_needed && range.start % 8 == 0 {
                    trunc_shift = range.start / 8;
                    range.start = 0;
                }

                if range.start == 0 && !trunc_needed && !mask_needed {
                    // superfluous bit range
                    return Ok(value);
                }

                if mask_needed && size > 8 {
                    return Err(IRBuilderError::BitRangeSize { value });
                }

                if range.start != 0 {
                    value = self.emit_binop(
                        Opcode::IntRShift,
                        value,
                        IRValue::Const(range.start as _, None),
                        None,
                    )?;
                }

                if trunc_needed {
                    value = self.emit_with_output(
                        Opcode::Subpiece,
                        vec![value, IRValue::Const(trunc_shift as _, None)],
                        None,
                    )?;
                    self.update_varnode(&mut value, size)?;
                }

                if mask_needed {
                    value =
                        self.emit_binop(Opcode::IntAnd, value, IRValue::Const(mask, None), None)?;
                }

                self.update_varnode(&mut value, size)?;

                output.map_or(Ok(value), |output| self.emit_copy(value, output))
            }
        }
    }

    fn resolve_expr(&mut self, expr: &Expr) -> Result<IRExpr, IRBuilderError> {
        let expr = match expr {
            Expr::Ident { value: name, size } => {
                let value = self.resolve_existing_name(*name)?;
                IRExpr::Var {
                    value,
                    offset: 0,
                    size: *size,
                }
            }
            Expr::Literal { value, size } => IRExpr::Const {
                value: *value,
                size: *size,
            },
            Expr::UnOp { op, value } => {
                let expr = self.resolve_expr(value)?;
                let value = self.expr_to_value(expr, None)?;

                IRExpr::UnOp {
                    op: Self::unop_to_opcode(*op),
                    value,
                }
            }
            Expr::BinOp { op, lvalue, rvalue } => {
                let lexpr = self.resolve_expr(lvalue)?;
                let lvalue = self.expr_to_value(lexpr, None)?;

                let rexpr = self.resolve_expr(rvalue)?;
                let rvalue = self.expr_to_value(rexpr, None)?;

                IRExpr::BinOp {
                    op: Self::binop_to_opcode(*op),
                    lvalue,
                    rvalue,
                }
            }
            Expr::BinRel { op, lvalue, rvalue } => {
                let lexpr = self.resolve_expr(lvalue)?;
                let lvalue = self.expr_to_value(lexpr, None)?;

                let rexpr = self.resolve_expr(rvalue)?;
                let rvalue = self.expr_to_value(rexpr, None)?;

                let (flip, op) = Self::binrel_to_opcode(*op);

                let (lvalue, rvalue) = if flip {
                    (rvalue, lvalue)
                } else {
                    (lvalue, rvalue)
                };

                IRExpr::BinOp { op, lvalue, rvalue }
            }
            Expr::Load {
                space,
                size,
                source,
            } => {
                let source = self.resolve_expr(source)?;
                let value = self.expr_to_value(source, None)?;

                IRExpr::Load {
                    space: *space,
                    size: *size,
                    source: value,
                }
            }
            Expr::Intrinsic { name, arguments } => self.resolve_expr_intrinsic(*name, arguments)?,
            Expr::AddressOf { value, size } => {
                let value = self.resolve_existing_name(*value)?;

                IRExpr::AddressOf { value, size: *size }
            }
            Expr::BitsOf { value, range, size } => {
                let value = self.resolve_existing_name(*value)?;

                IRExpr::BitsOf {
                    value,
                    range: range.clone(),
                    size: *size,
                }
            }
        };

        Ok(expr)
    }

    fn resolve_existing_name(&self, name: Ustr) -> Result<IRValue, IRBuilderError> {
        if let Some(reg) = self.translator.register_by_name(name) {
            Ok(IRValue::Register(reg))
        } else {
            self.locals
                .get(name)
                .map(IRValue::Temporary)
                .ok_or_else(|| IRBuilderError::LocalUndef { name: name.into() })
        }
    }

    fn resolve_name(
        &mut self,
        decl: bool,
        name: Ustr,
        size: Option<u32>,
    ) -> Result<IRValue, IRBuilderError> {
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
        }
        .map(IRValue::Temporary)
    }

    fn unop_to_opcode(value: UnOp) -> Opcode {
        match value {
            UnOp::BoolNot => Opcode::BoolNot,
            UnOp::Not => Opcode::IntNot,
            UnOp::Neg => Opcode::IntNeg,
            UnOp::FloatNeg => Opcode::FloatNeg,
        }
    }

    fn binop_to_opcode(value: BinOp) -> Opcode {
        match value {
            BinOp::BoolOr => Opcode::BoolOr,
            BinOp::BoolAnd => Opcode::BoolAnd,
            BinOp::BoolXor => Opcode::BoolXor,

            BinOp::Or => Opcode::IntOr,
            BinOp::And => Opcode::IntAnd,
            BinOp::Xor => Opcode::IntXor,

            BinOp::ShiftLeft => Opcode::IntLShift,
            BinOp::ShiftRight => Opcode::IntRShift,
            BinOp::SignedShiftRight => Opcode::IntSRShift,

            BinOp::Add => Opcode::IntAdd,
            BinOp::Sub => Opcode::IntSub,
            BinOp::Mul => Opcode::IntMul,
            BinOp::Div => Opcode::IntDiv,
            BinOp::Rem => Opcode::IntRem,

            BinOp::SignedDiv => Opcode::IntSDiv,
            BinOp::SignedRem => Opcode::IntSRem,

            BinOp::FloatAdd => Opcode::FloatAdd,
            BinOp::FloatSub => Opcode::FloatSub,
            BinOp::FloatMul => Opcode::FloatMul,
            BinOp::FloatDiv => Opcode::FloatDiv,
        }
    }

    fn binrel_to_opcode(value: BinRel) -> (bool, Opcode) {
        let mut flip = false;
        let opcode = match value {
            BinRel::Eq => Opcode::IntEq,
            BinRel::NotEq => Opcode::IntNotEq,

            BinRel::Less => Opcode::IntLess,
            BinRel::LessEq => Opcode::IntLessEq,
            BinRel::Greater => {
                flip = true;
                Opcode::IntLess
            }
            BinRel::GreaterEq => {
                flip = true;
                Opcode::IntLessEq
            }

            BinRel::SignedLess => Opcode::IntSLess,
            BinRel::SignedLessEq => Opcode::IntSLessEq,
            BinRel::SignedGreater => {
                flip = true;
                Opcode::IntSLess
            }
            BinRel::SignedGreaterEq => {
                flip = true;
                Opcode::IntSLessEq
            }

            BinRel::FloatEq => Opcode::FloatEq,
            BinRel::FloatNotEq => Opcode::FloatNotEq,

            BinRel::FloatLess => Opcode::FloatLess,
            BinRel::FloatLessEq => Opcode::FloatLessEq,
            BinRel::FloatGreater => {
                flip = true;
                Opcode::FloatLess
            }
            BinRel::FloatGreaterEq => {
                flip = true;
                Opcode::FloatLessEq
            }
        };
        (flip, opcode)
    }

    fn size_of(&self, value: &IRValue) -> Option<u32> {
        match value {
            IRValue::Const(_, sz) => *sz,
            IRValue::Register(vnd) => Some(vnd.size() as _),
            IRValue::Temporary(id) => self.locals.size_of(*id),
            IRValue::Address(_, _) => Some(self.default_size),
            _ => None,
        }
    }

    fn address_of(&self, value: IRValue, size: Option<u32>) -> Result<IRValue, IRBuilderError> {
        let space = self.space(&value);
        let value = match value {
            IRValue::Const(v, _) => v,
            IRValue::Register(vnd) => vnd.offset(),
            IRValue::Address(v, _) => v,
            IRValue::Temporary(id) => {
                return Err(IRBuilderError::LocalAddr {
                    name: self.locals[id]
                        .name
                        .map(LocalIdent::Named)
                        .unwrap_or(LocalIdent::Unnamed(id)),
                });
            }
            IRValue::Label(_) => panic!("labels do not have an offset"),
        };

        Ok(IRValue::Const(
            value,
            Some(size.unwrap_or_else(|| space.address_size() as u32)),
        ))
    }

    fn space(&self, value: &IRValue) -> &AddressSpace {
        let spaces = self.translator.manager();
        match value {
            IRValue::Const(_, _) => spaces.constant_space_ref(),
            IRValue::Register(_) => spaces.register_space_ref(),
            IRValue::Temporary(_) => spaces.unique_space_ref(),
            IRValue::Address(_, _) => spaces.default_space_ref(),
            IRValue::Label(_) => panic!("labels do not have a space"),
        }
    }

    fn update_varnode_labels(
        &mut self,
        index: usize,
        inputs: &mut [IRValue],
    ) -> Result<(), IRBuilderError> {
        for input in inputs.iter_mut() {
            let IRValue::Label(label) = input else {
                continue;
            };

            let position = self
                .labels
                .get(label)
                .copied()
                .ok_or_else(|| IRBuilderError::UnknownLabel { label: *label })?
                as u64;

            let rposition = position.wrapping_sub(index as u64);

            *input = IRValue::Const(rposition, None);
        }

        Ok(())
    }

    fn update_varnode_defaults(
        &mut self,
        inputs: &mut [IRValue],
        output: Option<&mut IRValue>,
    ) -> Result<(), IRBuilderError> {
        for input in inputs.iter_mut() {
            self.try_update_varnode(input, self.default_size)?;
        }

        if let Some(output) = output {
            self.try_update_varnode(output, self.default_size)?;
        }

        Ok(())
    }

    fn update_varnodes(
        &mut self,
        op: Opcode,
        inputs: &mut [IRValue],
        mut output: Option<&mut IRValue>,
        apply_defaults: bool,
    ) -> Result<(), IRBuilderError> {
        match op {
            Opcode::Copy
            | Opcode::IntNeg
            | Opcode::IntNot
            | Opcode::FloatAbs
            | Opcode::FloatCeiling
            | Opcode::FloatFloor
            | Opcode::FloatNeg
            | Opcode::FloatRound => {
                self.update_varnode_pair(&mut inputs[0], output.as_mut().unwrap(), apply_defaults)?;
            }
            Opcode::IntLShift | Opcode::IntRShift | Opcode::IntSRShift => {
                self.update_varnode_pair(&mut inputs[0], output.as_mut().unwrap(), apply_defaults)?;

                if self.size_of(&inputs[1]).is_none() {
                    self.update_varnode(&mut inputs[1], self.default_size)?;
                }
            }
            Opcode::IntAdd
            | Opcode::IntSub
            | Opcode::IntMul
            | Opcode::IntDiv
            | Opcode::IntRem
            | Opcode::IntSDiv
            | Opcode::IntSRem
            | Opcode::IntAnd
            | Opcode::IntOr
            | Opcode::IntXor
            | Opcode::FloatAdd
            | Opcode::FloatSub
            | Opcode::FloatMul
            | Opcode::FloatDiv => {
                let mut inputs = inputs.into_iter();

                self.update_varnode_triple(
                    inputs.next().unwrap(),
                    inputs.next().unwrap(),
                    output.as_mut().unwrap(),
                    apply_defaults,
                )?;
            }
            Opcode::IntCarry
            | Opcode::IntSCarry
            | Opcode::IntSBorrow
            | Opcode::IntEq
            | Opcode::IntNotEq
            | Opcode::IntLess
            | Opcode::IntSLess
            | Opcode::IntLessEq
            | Opcode::IntSLessEq
            | Opcode::FloatEq
            | Opcode::FloatNotEq
            | Opcode::FloatLess
            | Opcode::FloatLessEq => {
                let mut inputs = inputs.into_iter();

                self.update_varnode_pair(
                    inputs.next().unwrap(),
                    inputs.next().unwrap(),
                    apply_defaults,
                )?;
                self.update_varnode(output.as_mut().unwrap(), 1)?;
            }
            Opcode::FloatIsNaN => {
                self.update_varnode(output.as_mut().unwrap(), 1)?;
            }
            Opcode::BoolNot => {
                self.update_varnode(&mut inputs[0], 1)?;
                self.update_varnode(output.as_mut().unwrap(), 1)?;
            }
            Opcode::BoolAnd | Opcode::BoolOr | Opcode::BoolXor => {
                self.update_varnode(&mut inputs[0], 1)?;
                self.update_varnode(&mut inputs[1], 1)?;
                self.update_varnode(output.as_mut().unwrap(), 1)?;
            }
            Opcode::Load => {
                self.update_varnode(&mut inputs[1], self.default_size)?;
            }
            Opcode::Store => {
                self.update_varnode(&mut inputs[1], self.default_size)?;
            }
            _ => (),
        }

        Ok(())
    }

    fn update_varnode_pair(
        &mut self,
        vnd1: &mut IRValue,
        vnd2: &mut IRValue,
        apply_defaults: bool,
    ) -> Result<(), IRBuilderError> {
        let vnd1_size = self.size_of(vnd1);
        let vnd2_size = self.size_of(vnd2);

        match (vnd1_size, vnd2_size) {
            (None, None) if apply_defaults => {
                let sz = vnd1
                    .is_const()
                    .then(|| self.default_size)
                    .or_else(|| vnd2.is_const().then(|| self.default_size));

                if let Some(sz) = sz {
                    self.update_varnode(vnd1, sz)?;
                    self.update_varnode(vnd2, sz)?;
                }

                Ok(())
            }
            (None, Some(sz)) => self.update_varnode(vnd1, sz),
            (Some(sz), None) => self.update_varnode(vnd2, sz),
            (Some(sz1), Some(sz2)) => {
                if sz1 == sz2 {
                    Ok(())
                } else {
                    Err(IRBuilderError::UpdateSize2 {
                        v1: *vnd1,
                        v2: *vnd2,
                    })
                }
            }
            _ => Ok(()),
        }
    }

    fn update_varnode_triple(
        &mut self,
        vnd1: &mut IRValue,
        vnd2: &mut IRValue,
        vnd3: &mut IRValue,
        apply_defaults: bool,
    ) -> Result<(), IRBuilderError> {
        let vnd1_size = self.size_of(vnd1);
        let vnd2_size = self.size_of(vnd2);
        let vnd3_size = self.size_of(vnd3);

        match (vnd1_size, vnd2_size, vnd3_size) {
            (None, None, None) if apply_defaults => {
                let sz = vnd1
                    .is_const()
                    .then(|| self.default_size)
                    .or_else(|| vnd2.is_const().then(|| self.default_size))
                    .or_else(|| vnd3.is_const().then(|| self.default_size));

                if let Some(sz) = sz {
                    self.update_varnode(vnd1, sz)?;
                    self.update_varnode(vnd2, sz)?;
                    self.update_varnode(vnd3, sz)?;
                }
            }
            (Some(sz1), Some(sz2), Some(sz3)) => {
                if !(sz1 == sz2 && sz2 == sz3) {
                    return Err(IRBuilderError::UpdateSize3 {
                        v1: *vnd1,
                        v2: *vnd2,
                        v3: *vnd3,
                    });
                }
            }
            (Some(sz), _, _) => {
                self.update_varnode(vnd2, sz)?;
                self.update_varnode(vnd3, sz)?;
            }
            (_, Some(sz), _) => {
                self.update_varnode(vnd1, sz)?;
                self.update_varnode(vnd3, sz)?;
            }
            (_, _, Some(sz)) => {
                self.update_varnode(vnd1, sz)?;
                self.update_varnode(vnd2, sz)?;
            }
            _ => (),
        }

        Ok(())
    }

    fn update_varnode(&mut self, input: &mut IRValue, size: u32) -> Result<(), IRBuilderError> {
        match input {
            IRValue::Const(_, ref mut sz) => {
                if matches!(sz, Some(sz) if size != *sz) {
                    return Err(IRBuilderError::UpdateSize1 { v1: *input, size });
                } else {
                    *sz = Some(size);
                }
            }
            IRValue::Register(vnd) => {
                let expected = vnd.size() as u32;
                if expected != size {
                    let name = self
                        .translator
                        .registers()
                        .get(vnd.offset(), vnd.size())
                        .copied()
                        .unwrap();

                    return Err(IRBuilderError::RegSize {
                        name,
                        size,
                        old_size: expected,
                    });
                }
            }
            IRValue::Temporary(index) => self.locals.update_size(*index, size)?,
            _ => (),
        }

        Ok(())
    }

    fn try_update_varnode(&mut self, input: &mut IRValue, size: u32) -> Result<(), IRBuilderError> {
        match input {
            IRValue::Const(_, ref mut sz @ None) => {
                *sz = Some(size);
            }
            IRValue::Temporary(index) => self.locals.try_update_size(*index, size)?,
            _ => (),
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use fugue_ir::disassembly::IRBuilderArena;
    use fugue_ir::LanguageDB;

    use super::IRBuilder;

    #[test]
    fn test_features() -> Result<(), Box<dyn std::error::Error>> {
        let ldb = LanguageDB::from_directory_with(std::env::var("FUGUE_DATA")?, true)?;
        let translator = ldb
            .lookup_str("x86:LE:64:default")?
            .expect("valid language")
            .build()?;

        let mut builder = IRBuilder::new(&translator);
        let irb = IRBuilderArena::with_capacity(4096);

        let ir = builder.translate(
            &irb,
            r#"
            local v0:8 = 10;
            local v1:8 = 20;
            local counter:8 = 0;

            <label1>
            v2 = v0 + v1;
            if v2 < 10 goto <label1>;

            v2 = RAX(4);

            addr:8 = zext(v2);
            nvar = addr + 0x10;

            *:2 nvar = *addr;

            v3:4 = sext(0);
            v4:8 = zext(10:10);
            v5:4 = popcount(nvar);
            "#,
        )?;

        for (i, stmt) in ir.into_iter().enumerate() {
            println!("{i:03} {}", stmt.display(&translator));
        }

        Ok(())
    }

    #[test]
    fn test_bit_range() -> Result<(), Box<dyn std::error::Error>> {
        let ldb = LanguageDB::from_directory_with(std::env::var("FUGUE_DATA")?, true)?;
        let translator = ldb
            .lookup_str("x86:LE:64:default")?
            .expect("valid language")
            .build()?;

        let mut builder = IRBuilder::new(&translator);
        let irb = IRBuilderArena::with_capacity(4096);

        let ir = builder.translate(
            &irb,
            r#"
            EAX = RCX[3,32];
            EAX = RCX[32,32];
            AL = RCX[7,7];
            addr = &BX + 4;
            BL = addr[4,8];
            RBX[4,8] = AL;
            RBX[5,7] = AL;
            RBX:1 = AL;
            "#,
        )?;

        for (i, stmt) in ir.into_iter().enumerate() {
            println!("{i:03} {}", stmt.display(&translator));
        }

        Ok(())
    }

    #[test]
    fn test_eh_prolog() -> Result<(), Box<dyn std::error::Error>> {
        let ldb = LanguageDB::from_directory_with(std::env::var("FUGUE_DATA")?, true)?;
        let translator = ldb
            .lookup_str("x86:LE:64:default")?
            .expect("valid language")
            .build()?;

        let mut builder = IRBuilder::new(&translator);
        let irb = IRBuilderArena::with_capacity(4096);

        let ir = builder.translate(
            &irb,
            r#"
             ESP = ESP - 4;
             *:4 ESP = -1;
             ESP = ESP - 4;
             * ESP = EAX;
             EAX = * FS_OFFSET;
             ESP = ESP - 4;
             * ESP = EAX;
             * FS_OFFSET = ESP;
             tmp = ESP + 12;
             * tmp = EBP;
             EBP = tmp;
            "#,
        )?;

        for (i, stmt) in ir.into_iter().enumerate() {
            println!("{i:03} {}", stmt.display(&translator));
        }

        Ok(())
    }

    #[test]
    fn test_alloca_probe() -> Result<(), Box<dyn std::error::Error>> {
        let ldb = LanguageDB::from_directory_with(std::env::var("FUGUE_DATA")?, true)?;
        let translator = ldb
            .lookup_str("x86:LE:64:default")?
            .expect("valid language")
            .build()?;

        let mut builder = IRBuilder::new(&translator);
        let irb = IRBuilderArena::with_capacity(4096);

        let ir = builder.translate(
            &irb,
            r#"
            ESP = ESP + 4 - EAX;
            "#,
        )?;

        for (i, stmt) in ir.into_iter().enumerate() {
            println!("{i:03} {}", stmt.display(&translator));
        }

        Ok(())
    }

    #[test]
    fn test_seh_prolog() -> Result<(), Box<dyn std::error::Error>> {
        let ldb = LanguageDB::from_directory_with(std::env::var("FUGUE_DATA")?, true)?;
        let translator = ldb
            .lookup_str("x86:LE:64:default")?
            .expect("valid language")
            .build()?;

        let mut builder = IRBuilder::new(&translator);
        let irb = IRBuilderArena::with_capacity(4096);

        let ir = builder.translate(
            &irb,
            r#"
            newframetmp = ESP + 8;
            localsizetmp = * newframetmp;
            ESP = ESP - localsizetmp;
            ESP = ESP - 20;
            * newframetmp = EBP;
            EBP = newframetmp;
            *ESP = EDI;
            *(ESP+4) = ESI;
            *(ESP+8) = EBX;
            "#,
        )?;

        for (i, stmt) in ir.into_iter().enumerate() {
            println!("{i:03} {}", stmt.display(&translator));
        }

        Ok(())
    }

    #[test]
    fn test_seh_prolog4() -> Result<(), Box<dyn std::error::Error>> {
        let ldb = LanguageDB::from_directory_with(std::env::var("FUGUE_DATA")?, true)?;
        let translator = ldb
            .lookup_str("x86:LE:64:default")?
            .expect("valid language")
            .build()?;

        let mut builder = IRBuilder::new(&translator);
        let irb = IRBuilderArena::with_capacity(4096);

        let ir = builder.translate(
            &irb,
            r#"
            newframetmp = ESP - 8;
            local localsizetmp = * newframetmp;
            ESP = ESP - localsizetmp;
            ESP = ESP - 24;
            * newframetmp = EBP;
            EBP = newframetmp;
            *(ESP+4) = EDI;
            *(ESP+8) = ESI;
            *(ESP+12) = EBX;
            "#,
        )?;

        for (i, stmt) in ir.into_iter().enumerate() {
            println!("{i:03} {}", stmt.display(&translator));
        }

        Ok(())
    }
}
