use std::cmp::Ordering;
use std::collections::hash_map::Entry;

use fugue_ir::disassembly::{ArenaVec, IRBuilderArena, Opcode, PCodeRaw};
use fugue_ir::{AddressSpace, Translator, VarnodeData};

use thiserror::Error;
use ustr::{Ustr, UstrMap};

use crate::ast::BinRel;
use crate::ast::BranchTarget;
use crate::ast::{AstError, BinOp, CodeBlock, Expr, Stmt, UnOp};
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
    #[error("use of undefined temporary {name}")]
    LocalUndef { name: Ustr },
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
    Const(u64, Option<u32>),
    Register(VarnodeData),
    Temporary(LocalId),
    Label(Ustr),
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
                self.locals.push(LocalVar {
                    name: Some(name),
                    size,
                });
                entry.insert(id);
                Ok(id)
            }
            Entry::Occupied(_) => Err(IRBuilderError::LocalDup { name }),
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
        let cfg = CFG::new(&ast)?;

        for block in cfg.blocks() {
            for stmt in block.iter() {
                self.resolve_stmt(stmt)?;
            }
        }

        todo!("{:#?}", self.emitted)
    }

    fn emit_op(
        &mut self,
        op: Opcode,
        mut inputs: Vec<IRValue>,
        mut output: Option<IRValue>,
    ) -> Option<IRValue> {
        self.update_varnodes(op, &mut inputs, output.as_mut());
        self.emitted.emit(op, inputs, output);
        output
    }

    fn emit_copy(&mut self, input: IRValue, output: IRValue) -> IRValue {
        self.emit_op(Opcode::Copy, vec![input], Some(output))
            .expect("has output")
    }

    fn emit_with_output(
        &mut self,
        op: Opcode,
        inputs: Vec<IRValue>,
        output: Option<IRValue>,
    ) -> IRValue {
        let output = output.unwrap_or_else(|| self.new_local(None));
        self.emit_op(op, inputs, Some(output)).expect("has output")
    }

    fn emit_unop(&mut self, op: Opcode, input: IRValue, output: Option<IRValue>) -> IRValue {
        self.emit_with_output(op, vec![input], output)
    }

    fn emit_binop(
        &mut self,
        op: Opcode,
        lvalue: IRValue,
        rvalue: IRValue,
        output: Option<IRValue>,
    ) -> IRValue {
        self.emit_with_output(op, vec![lvalue, rvalue], output)
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
                let target = self.resolve_name(*decl, *name, *size)?;

                self.expr_to_value(source, Some(target))?;

                // NOTE: if bits != None -> bit range of target
                // NOTE: if size != target.size -> truncate target (slice it)

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
        todo!()
    }

    fn resolve_branch(&mut self, target: &BranchTarget) -> Result<(), IRBuilderError> {
        todo!()
    }

    fn resolve_cbranch(
        &mut self,
        target: &BranchTarget,
        condition: &Expr,
    ) -> Result<(), IRBuilderError> {
        todo!()
    }

    fn resolve_call(&mut self, target: &BranchTarget) -> Result<(), IRBuilderError> {
        todo!()
    }

    fn resolve_return(&mut self, target: &BranchTarget) -> Result<(), IRBuilderError> {
        todo!()
    }

    fn resolve_intrinsic(&mut self, name: Ustr, arguments: &[Expr]) -> Result<(), IRBuilderError> {
        todo!()
    }

    fn expr_to_value(
        &mut self,
        expr: IRExpr,
        output: Option<IRValue>,
    ) -> Result<IRValue, IRBuilderError> {
        let value = match expr {
            IRExpr::Var {
                value,
                offset,
                size,
            } => output.map_or(value, |output| self.emit_copy(value, output)),
            IRExpr::Const { value, size } => {
                let value = IRValue::Const(value, size);
                output.map_or(value, |output| self.emit_copy(value, output))
            }
            IRExpr::UnOp { op, value } => self.emit_unop(op, value, output),
            IRExpr::BinOp { op, lvalue, rvalue } => self.emit_binop(op, lvalue, rvalue, output),
        };
        Ok(value)
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
            _ => todo!(),
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
                .ok_or_else(|| IRBuilderError::LocalUndef { name })
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

    fn update_varnodes(
        &mut self,
        op: Opcode,
        inputs: &mut [IRValue],
        output: Option<&mut IRValue>,
    ) {
        // apply size updates
    }
}

#[cfg(test)]
mod test {
    use fugue_ir::disassembly::IRBuilderArena;
    use fugue_ir::LanguageDB;

    use super::IRBuilder;

    #[test]
    fn test_build() -> Result<(), Box<dyn std::error::Error>> {
        let ldb = LanguageDB::from_directory_with(std::env::var("FUGUE_DATA")?, true)?;
        let translator = ldb
            .lookup_str("x86:LE:64:default")?
            .expect("valid language")
            .build()?;

        let mut builder = IRBuilder::new(&translator);
        let irb = IRBuilderArena::with_capacity(4096);

        let _ir = builder.translate(
            &irb,
            r#"
            local v0:8 = 10;
            local v1:8 = 20;
            local counter:8 = 0;

            v2 = v0 + v1;
            "#,
        )?;

        Ok(())
    }
}
