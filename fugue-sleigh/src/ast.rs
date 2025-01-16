use std::fmt::Display;
use std::num::ParseIntError;
use std::ops::Range;

use itertools::Itertools;

use pest::error::Error;
use pest::iterators::Pair;
use pest::{Parser, Span};

use thiserror::Error;
use ustr::Ustr;

use crate::{Rule, SleighParser};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CodeBlock {
    stmts: Vec<Stmt>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Stmt {
    Assign {
        name: Ident,
        decl: bool,
        size: Option<u32>,
        bits: Option<Range<u32>>,
        source: Expr,
    },
    Declare {
        name: Ident,
        size: Option<u32>,
    },

    Store {
        space: Option<Ident>,
        size: Option<u32>,
        target: Expr,
        source: Expr,
    },

    Branch {
        target: BranchTarget,
    },
    CBranch {
        target: BranchTarget,
        condition: Expr,
    },

    Call {
        target: BranchTarget,
    },
    Return {
        target: BranchTarget,
    },

    Intrinsic {
        name: Ident,
        arguments: Vec<Expr>,
    },

    Label {
        label: Ident,
    },
}

impl Stmt {
    pub fn is_branch(&self) -> bool {
        matches!(
            self,
            Self::Branch { .. } | Stmt::CBranch { .. } | Stmt::Call { .. } | Stmt::Return { .. }
        )
    }

    pub fn has_fall(&self) -> bool {
        !matches!(self, Self::Return { .. } | Self::Branch { .. })
    }

    pub fn branch_target(&self) -> Option<Ident> {
        if let Self::Branch {
            target: BranchTarget::Label(label),
        }
        | Self::CBranch {
            target: BranchTarget::Label(label),
            ..
        } = self
        {
            Some(*label)
        } else {
            None
        }
    }

    pub fn label(&self) -> Option<Ident> {
        if let Self::Label { label } = self {
            Some(*label)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Expr {
    Ident {
        value: Ident,
        size: Option<u32>,
    },
    Literal {
        value: u64,
        size: Option<u32>,
    },

    UnOp {
        op: UnOp,
        value: Box<Expr>,
    },

    BinOp {
        op: BinOp,
        lvalue: Box<Expr>,
        rvalue: Box<Expr>,
    },
    BinRel {
        op: BinRel,
        lvalue: Box<Expr>,
        rvalue: Box<Expr>,
    },

    Load {
        space: Option<Ident>,
        size: Option<u32>,
        source: Box<Expr>,
    },

    AddressOf {
        value: Ident,
        size: Option<u32>,
    },
    BitsOf {
        value: Ident,
        range: Range<u32>,
        size: u32,
    },

    Truncate {
        value: Box<Expr>,
        size: u32,
    },

    Intrinsic {
        name: Ident,
        arguments: Vec<Expr>,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BinOp {
    BoolOr,
    BoolAnd,
    BoolXor,

    Or,
    And,
    Xor,

    ShiftLeft,
    ShiftRight,
    SignedShiftRight,

    Add,
    Sub,
    Mul,
    Div,
    Rem,

    SignedDiv,
    SignedRem,

    FloatAdd,
    FloatSub,
    FloatMul,
    FloatDiv,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BinRel {
    Eq,
    NotEq,

    Less,
    LessEq,
    Greater,
    GreaterEq,

    SignedLess,
    SignedLessEq,
    SignedGreater,
    SignedGreaterEq,

    FloatEq,
    FloatNotEq,

    FloatLess,
    FloatLessEq,
    FloatGreater,
    FloatGreaterEq,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum UnOp {
    BoolNot,
    Not,
    Neg,
    FloatNeg,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BranchTarget {
    Direct(BranchLabel),
    Indirect(Expr),
    Label(Ident),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BranchLabel {
    Offset { offset: u64, space: Option<Ident> },
    Varnode { name: Ident },
}

pub type Ident = Ustr;

#[derive(Debug, Error)]
pub enum AstError {
    #[error(transparent)]
    Parse(#[from] Error<Rule>),
    #[error("{0}: {1}")]
    Integer(ErrorSpan, ParseIntError),
    #[error("{0}: invalid bit-range")]
    BitRange(ErrorSpan),
    #[error("{0}: invalid size")]
    Size(ErrorSpan),
    #[error("attempt to define label `{0}` more than once")]
    DuplicateLabel(Ustr),
    #[error("reference to undefined label `{0}`")]
    UndefinedLabel(Ustr),
}

#[derive(Debug)]
pub struct ErrorSpan {
    pub start: usize,
    pub end: usize,
}

impl Display for ErrorSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

impl From<Span<'_>> for ErrorSpan {
    fn from(value: Span<'_>) -> Self {
        Self {
            start: value.start(),
            end: value.end(),
        }
    }
}

impl CodeBlock {
    pub fn parse(input: &str) -> Result<Self, AstError> {
        let parsed = SleighParser::parse(Rule::code_block, input)?
            .into_iter()
            .next() // code_block
            .unwrap()
            .into_inner()
            .next() // statements
            .unwrap();

        Ok(Self {
            stmts: parsed
                .into_inner()
                .map(|stmt| Self::parse_stmt(stmt))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    pub fn statements(&self) -> &[Stmt] {
        &self.stmts
    }

    fn parse_assignment(assign: Pair<'_, Rule>, decl: bool) -> Result<Stmt, AstError> {
        let mut pairs = assign.into_inner();

        let lvalue = pairs.next().unwrap().into_inner().next().unwrap();
        let expr = Self::parse_expr(pairs.next().unwrap())?;

        let assign = match lvalue.as_rule() {
            Rule::sembitrange => {
                let (name, bits, size) = Self::parse_sembitrange(lvalue)?;

                Stmt::Assign {
                    name,
                    decl,
                    size: Some(size),
                    bits: Some(bits),
                    source: expr,
                }
            }
            Rule::sized_identifier => {
                let (name, size) = Self::parse_sized(lvalue, Self::parse_identifier)?;

                Stmt::Assign {
                    name,
                    decl,
                    size: Some(size),
                    bits: None,
                    source: expr,
                }
            }
            Rule::identifier => Stmt::Assign {
                name: Self::parse_identifier(lvalue)?,
                decl,
                size: None,
                bits: None,
                source: expr,
            },
            Rule::sized_star_expr => {
                let mut pairs = lvalue.into_inner();
                let (space, size) = Self::parse_sized_star(pairs.next().unwrap())?;
                let target = Self::parse_expr(pairs.next().unwrap())?;

                Stmt::Store {
                    space,
                    size,
                    target,
                    source: expr,
                }
            }
            _ => unimplemented!(),
        };

        Ok(assign)
    }

    fn parse_declaration(decl: Pair<'_, Rule>) -> Result<Stmt, AstError> {
        let mut pairs = decl.into_inner();
        let decl = pairs.next().unwrap();

        let decl = match decl.as_rule() {
            Rule::declaration_ => Stmt::Declare {
                name: Self::parse_identifier(decl.into_inner().next().unwrap())?,
                size: None,
            },
            Rule::declaration_with_size => {
                let mut pairs = decl.into_inner();

                let ident = Self::parse_identifier(pairs.next().unwrap())?;
                let size = Self::parse_size(pairs.next().unwrap(), true)?;

                Stmt::Declare {
                    name: ident,
                    size: Some(size),
                }
            }
            _ => unreachable!(),
        };

        Ok(decl)
    }

    fn parse_expr(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        Self::parse_expr_boolor(pairs.next().unwrap())
    }

    fn parse_expr_boolor(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let mut expr = Self::parse_expr_booland(pairs.next().unwrap())?;

        for (_, pair) in pairs.tuples() {
            expr = Expr::BinOp {
                op: BinOp::BoolOr,
                lvalue: Box::new(expr),
                rvalue: Box::new(Self::parse_expr_booland(pair)?),
            };
        }

        Ok(expr)
    }

    fn parse_expr_booland(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let mut expr = Self::parse_expr_or(pairs.next().unwrap())?;

        for (op, pair) in pairs.tuples() {
            let op = match op.as_str() {
                "&&" => BinOp::BoolAnd,
                "^^" => BinOp::BoolXor,
                _ => unreachable!(),
            };

            expr = Expr::BinOp {
                op,
                lvalue: Box::new(expr),
                rvalue: Box::new(Self::parse_expr_or(pair)?),
            };
        }

        Ok(expr)
    }

    fn parse_expr_or(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let mut expr = Self::parse_expr_xor(pairs.next().unwrap())?;

        for (_, pair) in pairs.tuples() {
            expr = Expr::BinOp {
                op: BinOp::Or,
                lvalue: Box::new(expr),
                rvalue: Box::new(Self::parse_expr_xor(pair)?),
            };
        }

        Ok(expr)
    }

    fn parse_expr_xor(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let mut expr = Self::parse_expr_and(pairs.next().unwrap())?;

        for (_, pair) in pairs.tuples() {
            expr = Expr::BinOp {
                op: BinOp::Xor,
                lvalue: Box::new(expr),
                rvalue: Box::new(Self::parse_expr_and(pair)?),
            };
        }

        Ok(expr)
    }

    fn parse_expr_and(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let mut expr = Self::parse_expr_eq(pairs.next().unwrap())?;

        for (_, pair) in pairs.tuples() {
            expr = Expr::BinOp {
                op: BinOp::And,
                lvalue: Box::new(expr),
                rvalue: Box::new(Self::parse_expr_eq(pair)?),
            };
        }

        Ok(expr)
    }

    fn parse_expr_eq(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let mut expr = Self::parse_expr_comp(pairs.next().unwrap())?;

        for (op, pair) in pairs.tuples() {
            let op = match op.as_str() {
                "==" => BinRel::Eq,
                "!=" => BinRel::NotEq,
                "f==" => BinRel::FloatEq,
                "f!=" => BinRel::FloatNotEq,
                _ => unreachable!(),
            };

            expr = Expr::BinRel {
                op,
                lvalue: Box::new(expr),
                rvalue: Box::new(Self::parse_expr_comp(pair)?),
            };
        }

        Ok(expr)
    }

    fn parse_expr_comp(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let mut expr = Self::parse_expr_shift(pairs.next().unwrap())?;

        for (op, pair) in pairs.tuples() {
            let op = match op.as_str() {
                "<" => BinRel::Less,
                "<=" => BinRel::LessEq,
                ">" => BinRel::Greater,
                ">=" => BinRel::GreaterEq,
                "s<" => BinRel::SignedLess,
                "s<=" => BinRel::SignedLessEq,
                "s>" => BinRel::SignedGreater,
                "s>=" => BinRel::SignedGreaterEq,
                "f<" => BinRel::FloatLess,
                "f<=" => BinRel::FloatLessEq,
                "f>" => BinRel::FloatGreater,
                "f>=" => BinRel::FloatGreaterEq,
                _ => unreachable!(),
            };

            expr = Expr::BinRel {
                op,
                lvalue: Box::new(expr),
                rvalue: Box::new(Self::parse_expr_shift(pair)?),
            };
        }

        Ok(expr)
    }

    fn parse_expr_shift(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let mut expr = Self::parse_expr_add(pairs.next().unwrap())?;

        for (op, pair) in pairs.tuples() {
            let op = match op.as_str() {
                "<<" => BinOp::ShiftLeft,
                ">>" => BinOp::ShiftRight,
                ">>>" => BinOp::SignedShiftRight,
                _ => unreachable!(),
            };

            expr = Expr::BinOp {
                op,
                lvalue: Box::new(expr),
                rvalue: Box::new(Self::parse_expr_add(pair)?),
            };
        }

        Ok(expr)
    }

    fn parse_expr_add(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let mut expr = Self::parse_expr_mult(pairs.next().unwrap())?;

        for (op, pair) in pairs.tuples() {
            let op = match op.as_str() {
                "+" => BinOp::Add,
                "-" => BinOp::Sub,
                "f+" => BinOp::FloatAdd,
                "f-" => BinOp::FloatSub,
                _ => unreachable!(),
            };

            expr = Expr::BinOp {
                op,
                lvalue: Box::new(expr),
                rvalue: Box::new(Self::parse_expr_mult(pair)?),
            };
        }

        Ok(expr)
    }

    fn parse_expr_mult(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let mut expr = Self::parse_expr_unary(pairs.next().unwrap())?;

        for (op, pair) in pairs.tuples() {
            let op = match op.as_str() {
                "*" => BinOp::Mul,
                "/" => BinOp::Div,
                "%" => BinOp::Rem,
                "s/" => BinOp::SignedDiv,
                "s%" => BinOp::SignedRem,
                "f*" => BinOp::FloatMul,
                "f/" => BinOp::FloatDiv,
                _ => unreachable!(),
            };

            expr = Expr::BinOp {
                op,
                lvalue: Box::new(expr),
                rvalue: Box::new(Self::parse_expr_unary(pair)?),
            };
        }

        Ok(expr)
    }

    fn parse_expr_unary(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let op_or_expr = pairs.next().unwrap();

        if op_or_expr.as_rule() == Rule::unary_op {
            let op = match op_or_expr.as_str() {
                "!" => UnOp::BoolNot,
                "~" => UnOp::Not,
                "-" => UnOp::Neg,
                "f-" => UnOp::FloatNeg,
                _ => {
                    let (space, size) =
                        Self::parse_sized_star(op_or_expr.into_inner().next().unwrap())?;
                    let source = Self::parse_expr_func(pairs.next().unwrap())?;

                    return Ok(Expr::Load {
                        space,
                        size,
                        source: Box::new(source),
                    });
                }
            };

            Ok(Expr::UnOp {
                op,
                value: Box::new(Self::parse_expr_func(pairs.next().unwrap())?),
            })
        } else {
            Self::parse_expr_func(op_or_expr)
        }
    }

    fn parse_expr_func(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let expr = pairs.next().unwrap();

        match expr.as_rule() {
            Rule::expr_apply => Self::parse_expr_apply(expr),
            Rule::expr_term => Self::parse_expr_term(expr),
            _ => unreachable!("{expr:#?}"),
        }
    }

    fn parse_expr_apply(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let name = Self::parse_identifier(pairs.next().unwrap())?;

        let arguments = pairs
            .next()
            .unwrap()
            .into_inner()
            .next()
            .map(|exprs| {
                exprs
                    .into_inner()
                    .map(|expr| Self::parse_expr(expr))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();

        Ok(Expr::Intrinsic { name, arguments })
    }

    fn parse_expr_term(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let expr = pairs.next().unwrap();

        match expr.as_rule() {
            Rule::varnode => Self::parse_varnode(expr),
            Rule::sembitrange => {
                let (value, range, size) = Self::parse_sembitrange(expr)?;
                Ok(Expr::BitsOf { value, range, size })
            }
            _ => Self::parse_expr(expr),
        }
    }

    fn parse_varnode(target: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = target.into_inner();
        let target = pairs.next().unwrap();

        let result = match target.as_rule() {
            Rule::integer => Expr::Literal {
                value: Self::parse_integer(target)?,
                size: None,
            },
            Rule::identifier => Expr::Ident {
                value: Self::parse_identifier(target)?,
                size: None,
            },
            Rule::addressof => Expr::AddressOf {
                value: Self::parse_identifier(target.into_inner().next().unwrap())?,
                size: None,
            },
            Rule::integer_with_size => {
                let (value, size) = Self::parse_sized(target, Self::parse_integer)?;
                Expr::Literal {
                    value,
                    size: Some(size),
                }
            }
            Rule::identifier_with_size => {
                let (value, size) = Self::parse_sized(target, Self::parse_identifier)?;
                Expr::Ident {
                    value,
                    size: Some(size),
                }
            }
            Rule::addressof_with_size => {
                let mut pairs = target.into_inner();

                let size = Self::parse_size(pairs.next().unwrap(), true)?;
                let value = Self::parse_identifier(pairs.next().unwrap())?;

                Expr::AddressOf {
                    value,
                    size: Some(size),
                }
            }
            _ => {
                todo!("varnode: {target:#?}")
            }
        };

        Ok(result)
    }

    fn parse_integer(target: Pair<'_, Rule>) -> Result<u64, AstError> {
        let integer = target.as_str();

        if let Some(integer) = integer.strip_prefix("0x") {
            return u64::from_str_radix(integer, 16)
                .map_err(|e| AstError::Integer(target.as_span().into(), e));
        }

        if let Some(integer) = integer.strip_prefix("0b") {
            return u64::from_str_radix(integer, 2)
                .map_err(|e| AstError::Integer(target.as_span().into(), e));
        }

        u64::from_str_radix(integer, 10).map_err(|e| AstError::Integer(target.as_span().into(), e))
    }

    fn parse_size(target: Pair<'_, Rule>, check: bool) -> Result<u32, AstError> {
        let integer = target.as_str();

        if let Some(integer) = integer.strip_prefix("0x") {
            return u32::from_str_radix(integer, 16)
                .map_err(|e| AstError::Integer(target.as_span().into(), e));
        }

        if let Some(integer) = integer.strip_prefix("0b") {
            return u32::from_str_radix(integer, 2)
                .map_err(|e| AstError::Integer(target.as_span().into(), e));
        }

        let size = u32::from_str_radix(integer, 10)
            .map_err(|e| AstError::Integer(target.as_span().into(), e))?;

        if check && size == 0 {
            Err(AstError::Size(target.as_span().into()))
        } else {
            Ok(size)
        }
    }

    fn parse_identifier(target: Pair<'_, Rule>) -> Result<Ident, AstError> {
        Ok(target.as_str().into())
    }

    fn parse_sembitrange(target: Pair<'_, Rule>) -> Result<(Ident, Range<u32>, u32), AstError> {
        let mut pairs = target.into_inner();

        let ident = Self::parse_identifier(pairs.next().unwrap())?;
        let start = Self::parse_size(pairs.next().unwrap(), false)?;

        let nbits_pair = pairs.next().unwrap();
        let nbits_span = nbits_pair.as_span();
        let nbits = Self::parse_size(nbits_pair, false)?;

        if nbits == 0 {
            return Err(AstError::BitRange(nbits_span.into()));
        }

        let size = nbits.div_ceil(8);

        Ok((ident, start..start + nbits, size))
    }

    fn parse_sized<F, T>(target: Pair<'_, Rule>, rule: F) -> Result<(T, u32), AstError>
    where
        F: FnOnce(Pair<'_, Rule>) -> Result<T, AstError>,
    {
        let mut pairs = target.into_inner();

        let t = rule(pairs.next().unwrap())?;
        let s = Self::parse_size(pairs.next().unwrap(), true)?;

        Ok((t, s))
    }

    fn parse_sized_star(target: Pair<'_, Rule>) -> Result<(Option<Ident>, Option<u32>), AstError> {
        let mut pairs = target.into_inner();

        let Some(ident_or_size) = pairs.next() else {
            return Ok((None, None));
        };

        let ident = if ident_or_size.as_rule() == Rule::identifier {
            Self::parse_identifier(ident_or_size)?
        } else {
            return Ok((None, Some(Self::parse_size(ident_or_size, true)?)));
        };

        let Some(size) = pairs.next() else {
            return Ok((Some(ident), None));
        };

        Ok((Some(ident), Some(Self::parse_size(size, true)?)))
    }

    fn parse_branch_target(target: Pair<'_, Rule>) -> Result<BranchTarget, AstError> {
        let mut pairs = target.into_inner();
        let target = pairs.next().unwrap();

        let target = match target.as_rule() {
            Rule::identifier => BranchTarget::Direct(BranchLabel::Varnode {
                name: Self::parse_identifier(target)?,
            }),
            Rule::offset_in_space => {
                let mut pairs = target.into_inner();

                let offset = Self::parse_integer(pairs.next().unwrap())?;
                let space = Self::parse_identifier(pairs.next().unwrap())?;

                BranchTarget::Direct(BranchLabel::Offset {
                    offset,
                    space: Some(space),
                })
            }
            Rule::integer => {
                let offset = Self::parse_integer(target)?;

                BranchTarget::Direct(BranchLabel::Offset {
                    offset,
                    space: None,
                })
            }
            Rule::label => {
                let label = Self::parse_identifier(target.into_inner().next().unwrap())?;

                BranchTarget::Label(label)
            }
            _ => {
                let target = Self::parse_expr(target)?;
                BranchTarget::Indirect(target)
            }
        };

        Ok(target)
    }

    fn parse_stmt(pair: Pair<'_, Rule>) -> Result<Stmt, AstError> {
        let pair = pair.into_inner().next().unwrap();

        match pair.as_rule() {
            Rule::assignment => {
                let assign = pair.into_inner().next().unwrap();
                let decl = assign.as_rule() == Rule::assignment_with_local;
                Self::parse_assignment(assign, decl)
            }
            Rule::declaration => {
                let decl = pair.into_inner().next().unwrap();
                Self::parse_declaration(decl)
            }
            Rule::funcall => {
                let mut pairs = pair.into_inner().next().unwrap().into_inner();
                let name = pairs.next().unwrap();
                let args = pairs
                    .next()
                    .unwrap()
                    .into_inner()
                    .next() // expr_list
                    .map(|list| {
                        list.into_inner()
                            .map(Self::parse_expr)
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?
                    .unwrap_or_default();

                Ok(Stmt::Intrinsic {
                    name: name.as_str().into(),
                    arguments: args,
                })
            }
            Rule::goto_stmt => {
                let target = pair.into_inner().next().unwrap();
                Ok(Stmt::Branch {
                    target: Self::parse_branch_target(target)?,
                })
            }
            Rule::cond_stmt => {
                let mut pairs = pair.into_inner();
                let condition = Self::parse_expr(pairs.next().unwrap())?;
                let target = Self::parse_branch_target(pairs.next().unwrap())?;
                Ok(Stmt::CBranch { condition, target })
            }
            Rule::call_stmt => {
                let target = pair.into_inner().next().unwrap();
                Ok(Stmt::Call {
                    target: Self::parse_branch_target(target)?,
                })
            }
            Rule::return_stmt => {
                let target = pair.into_inner().next().unwrap();
                Ok(Stmt::Return {
                    target: BranchTarget::Indirect(Self::parse_expr(target)?),
                })
            }
            Rule::label => {
                let label = pair.into_inner().next().unwrap();
                Ok(Stmt::Label {
                    label: label.as_str().into(),
                })
            }
            rule => unreachable!("{rule:?}"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse() -> Result<(), Box<dyn std::error::Error>> {
        let ast = CodeBlock::parse(r#"
            memcpy(a || b || c, *[other] b, 10 + 20);

            local b:32 = 10;
            local a = *b;

            *[ram] (a + 10) = b;

            < label >

            a[0,1] = b[0,2] * c[0,10] + d:10 / b(10);

            goto dest1;
            goto [dest2];
            goto 0x0 [codespace];
            goto 0x0;
            goto [0x10 + 20];

            if (var > 10) goto <dest>;

            return [dest3];
            return [0x10];

            *:4 sp = inst_next;
            sp = sp-4;
            call dest;
"#,
        );

        assert!(ast.is_ok());
        assert_eq!(ast.unwrap().stmts.len(), 17);

        Ok(())
    }
}
