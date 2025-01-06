use std::fmt::Display;
use std::num::ParseIntError;

use itertools::Itertools;
use pest::error::Error;
use pest::iterators::Pair;
use pest::{Parser, Span};
use thiserror::Error;

use crate::{Rule, SleighParser};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CodeBlock {
    stmts: Vec<Stmt>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Stmt {
    Assign {
        name: Ident,
        size: Option<u32>,
        source: Expr,
    },
    Declare {
        name: Ident,
        size: Option<u32>,
    },

    Copy {
        target: Expr,
        source: Expr,
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
    SliceOf {
        value: Box<Expr>,
        start: u32,
        end: u32,
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

#[derive(Debug, Clone, Eq, PartialEq)]
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

#[derive(Debug, Clone, Eq, PartialEq)]
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum UnOp {
    BoolNot,
    Not,
    Neg,
    FloatNeg,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BranchTarget {
    Direct(BranchLabel),
    Indirect(BranchLabel),
    Label(Ident),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BranchLabel {
    Named(Ident),
    Relative(u64, u32),
}

pub type Ident = String;

#[derive(Debug, Error)]
pub enum AstError {
    #[error(transparent)]
    Parse(#[from] Error<Rule>),
    #[error("{0}: {1}")]
    Integer(ErrorSpan, ParseIntError),
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
                .map(|stmt| Self::parse_stmt(stmt.into_inner().next().unwrap()))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    fn parse_assignment(assign: Pair<'_, Rule>) -> Result<Stmt, AstError> {
        todo!("assign: {assign:#?}")
    }

    fn parse_declaration(decl: Pair<'_, Rule>) -> Result<Stmt, AstError> {
        todo!("decl: {decl:#?}")
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
                _ => Self::parse_sized_star(op_or_expr)?,
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
            _ => unreachable!(),
        }
    }

    fn parse_expr_apply(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let expr = pairs.next().unwrap();

        match expr.as_rule() {
            Rule::expr_apply => Self::parse_expr_apply(expr),
            Rule::expr_term => Self::parse_expr_term(expr),
            _ => unreachable!(),
        }
    }

    fn parse_expr_term(expr: Pair<'_, Rule>) -> Result<Expr, AstError> {
        let mut pairs = expr.into_inner();
        let expr = pairs.next().unwrap();

        match expr.as_rule() {
            Rule::varnode => Self::parse_varnode(expr),
            Rule::sembitrange => Self::parse_sembitrange(expr),
            _ => Self::parse_expr(pairs.next().unwrap()),
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

    fn parse_size(target: Pair<'_, Rule>) -> Result<u32, AstError> {
        let integer = target.as_str();

        if let Some(integer) = integer.strip_prefix("0x") {
            return u32::from_str_radix(integer, 16)
                .map_err(|e| AstError::Integer(target.as_span().into(), e));
        }

        if let Some(integer) = integer.strip_prefix("0b") {
            return u32::from_str_radix(integer, 2)
                .map_err(|e| AstError::Integer(target.as_span().into(), e));
        }

        u32::from_str_radix(integer, 10).map_err(|e| AstError::Integer(target.as_span().into(), e))
    }

    fn parse_identifier(target: Pair<'_, Rule>) -> Result<Ident, AstError> {
        Ok(target.as_str().to_owned())
    }

    fn parse_sembitrange(target: Pair<'_, Rule>) -> Result<Expr, AstError> {
        todo!("sembitrange: {target:#?}")
    }

    fn parse_sized<F, T>(target: Pair<'_, Rule>, rule: F) -> Result<(T, u32), AstError>
    where
        F: FnOnce(Pair<'_, Rule>) -> Result<T, AstError>,
    {
        let mut pairs = target.into_inner();

        let t = rule(pairs.next().unwrap())?;
        let s = Self::parse_size(pairs.next().unwrap())?;

        Ok((t, s))
    }

    fn parse_sized_star(target: Pair<'_, Rule>) -> Result<UnOp, AstError> {
        todo!("sized star: {target:#?}")
    }

    fn parse_branch_target(target: Pair<'_, Rule>) -> Result<BranchTarget, AstError> {
        todo!("branch target: {target:#?}")
    }

    fn parse_stmt(pair: Pair<'_, Rule>) -> Result<Stmt, AstError> {
        match pair.as_rule() {
            Rule::assignment => {
                let assign = pair.into_inner().next().unwrap();
                Self::parse_assignment(assign)
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
                    name: name.as_str().to_owned(),
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
                let target = pair.into_inner().next().unwrap();
                Ok(Stmt::Call {
                    target: Self::parse_branch_target(target)?,
                })
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
                    target: Self::parse_branch_target(target)?,
                })
            }
            Rule::label => {
                let label = pair.into_inner().next().unwrap();
                Ok(Stmt::Label {
                    label: label.as_str().to_owned(),
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
        let _ast = CodeBlock::parse(
            r#"memcpy(a || b || c, b, 10 + 20);
return [10];
local b:20 = 10;
a = 30;
"#,
        )?;

        Ok(())
    }
}
