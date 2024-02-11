use std::borrow::Cow;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

use fugue_bv::BitVec;
use smallvec::{smallvec, SmallVec};
use ustr::Ustr;

use crate::address::AddressValue;
use crate::disassembly::lift::{FloatFormats, UserOpStr};
use crate::disassembly::{IRBuilderArena, Opcode, VarnodeData};
use crate::float_format::FloatFormat;
use crate::il::pcode::{Operand, PCode, PCodeOp};
use crate::il::traits::*;
use crate::space::{AddressSpace, AddressSpaceId};
use crate::space_manager::{FromSpace, IntoSpace, SpaceManager};
use crate::Translator;

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub struct Var {
    space: AddressSpaceId,
    offset: u64,
    bits: usize,
    generation: usize,
}

impl Var {
    pub fn new<S: Into<AddressSpaceId>>(
        space: S,
        offset: u64,
        bits: usize,
        generation: usize,
    ) -> Self {
        Self {
            space: space.into(),
            offset,
            bits,
            generation,
        }
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }
}

impl BitSize for Var {
    fn bits(&self) -> usize {
        self.bits
    }
}

impl Variable for Var {
    fn generation(&self) -> usize {
        self.generation
    }

    fn generation_mut(&mut self) -> &mut usize {
        &mut self.generation
    }

    fn with_generation(&self, generation: usize) -> Self {
        Self {
            space: self.space.clone(),
            generation,
            ..*self
        }
    }

    fn space(&self) -> AddressSpaceId {
        self.space
    }
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "space[{}][{:#x}].{}:{}",
            self.space().index(),
            self.offset(),
            self.generation(),
            self.bits()
        )
    }
}

impl<'var, 'trans> fmt::Display for VarFormatter<'var, 'trans> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(trans) = self.fmt.translator {
            let space = trans.manager().unchecked_space_by_id(self.var.space());
            if space.is_register() {
                let name = trans
                    .registers()
                    .get(self.var.offset(), self.var.bits() / 8)
                    .unwrap();
                write!(
                    f,
                    "{}{}{}.{}{}{}:{}{}{}",
                    self.fmt.variable_start,
                    name,
                    self.fmt.variable_end,
                    self.fmt.value_start,
                    self.var.generation(),
                    self.fmt.value_end,
                    self.fmt.value_start,
                    self.var.bits(),
                    self.fmt.value_end,
                )
            } else {
                let off = self.var.offset();
                let sig = (off as i64).signum() as i128;
                write!(
                    f,
                    "{}{}{}[{}{}{}{}{:#x}{}].{}{}{}:{}{}{}",
                    self.fmt.variable_start,
                    space.name(),
                    self.fmt.variable_end,
                    self.fmt.keyword_start,
                    if sig == 0 {
                        ""
                    } else if sig > 0 {
                        "+"
                    } else {
                        "-"
                    },
                    self.fmt.keyword_end,
                    self.fmt.value_start,
                    self.var.offset() as i64 as i128 * sig,
                    self.fmt.value_end,
                    self.fmt.value_start,
                    self.var.generation(),
                    self.fmt.value_end,
                    self.fmt.value_start,
                    self.var.bits(),
                    self.fmt.value_end,
                )
            }
        } else {
            self.fmt(f)
        }
    }
}

pub struct VarFormatter<'var, 'trans> {
    var: &'var Var,
    fmt: Cow<'trans, TranslatorFormatter<'trans>>,
}

impl<'var, 'trans> TranslatorDisplay<'var, 'trans> for Var {
    type Target = VarFormatter<'var, 'trans>;

    fn display_full(
        &'var self,
        fmt: Cow<'trans, TranslatorFormatter<'trans>>,
    ) -> VarFormatter<'var, 'trans> {
        VarFormatter { var: self, fmt }
    }
}

impl From<VarnodeData> for Var {
    fn from(vnd: VarnodeData) -> Self {
        Self {
            space: vnd.space(),
            offset: vnd.offset(),
            bits: vnd.size() * 8,
            generation: 0,
        }
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
/// Location of the meta step
pub struct Location {
    /// The address of the architecture step
    address: AddressValue,
    /// The index of the meta step in the decoded instruction
    position: usize,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.address, self.position)
    }
}

impl<'loc, 'trans> TranslatorDisplay<'loc, 'trans> for Location {
    type Target = &'loc Self;

    fn display_with(&'loc self, _translator: Option<&'trans Translator>) -> Self::Target {
        self
    }

    fn display_full(&'loc self, _fmt: Cow<'trans, TranslatorFormatter<'trans>>) -> Self::Target {
        self
    }
}

impl Location {
    pub fn new<A>(address: A, position: usize) -> Location
    where
        A: Into<AddressValue>,
    {
        Self {
            address: address.into(),
            position,
        }
    }

    pub fn address(&self) -> Cow<AddressValue> {
        Cow::Borrowed(&self.address)
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn space(&self) -> AddressSpaceId {
        self.address.space()
    }

    pub fn is_relative(&self) -> bool {
        self.space().is_constant() && self.position == 0
    }

    pub fn is_absolute(&self) -> bool {
        !self.is_relative()
    }

    pub fn absolute_from<A>(&mut self, address: A, position: usize)
    where
        A: Into<AddressValue>,
    {
        if self.is_absolute() {
            return;
        }

        let offset = self.address().offset() as i64;
        let position = if offset.is_negative() {
            position
                .checked_sub(offset.abs() as usize)
                .expect("negative offset from position in valid range")
        } else {
            position
                .checked_add(offset as usize)
                .expect("positive offset from position in valid range")
        };

        self.address = address.into();
        self.position = position;
    }
}

impl<'z> FromSpace<'z, VarnodeData> for Location {
    fn from_space_with(t: VarnodeData, _arena: &'z IRBuilderArena, manager: &SpaceManager) -> Self {
        Location::from_space(t, manager)
    }

    fn from_space(vnd: VarnodeData, manager: &SpaceManager) -> Self {
        Self {
            address: AddressValue::new(manager.unchecked_space_by_id(vnd.space()), vnd.offset()),
            position: 0,
        }
    }
}

impl From<AddressValue> for Location {
    fn from(address: AddressValue) -> Self {
        Self {
            address,
            position: 0,
        }
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub enum BranchTargetT<Loc, Val, Var> {
    Location(Loc),
    Computed(ExprT<Loc, Val, Var>),
}

impl<'target, 'trans, Loc, Val, Var> fmt::Display for BranchTargetT<Loc, Val, Var>
where
    Loc: fmt::Display,
    Val: fmt::Display,
    Var: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BranchTargetT::Location(loc) => write!(f, "{}", loc),
            BranchTargetT::Computed(expr) => write!(f, "{}", expr),
        }
    }
}

pub struct BranchTargetTFormatter<'target, 'trans, Loc, Val, Var> {
    target: &'target BranchTargetT<Loc, Val, Var>,
    fmt: Cow<'trans, TranslatorFormatter<'trans>>,
}

impl<'target, 'trans, Loc, Val, Var> fmt::Display
    for BranchTargetTFormatter<'target, 'trans, Loc, Val, Var>
where
    Loc: for<'a> TranslatorDisplay<'target, 'a>,
    Val: for<'a> TranslatorDisplay<'target, 'a>,
    Var: for<'a> TranslatorDisplay<'target, 'a>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.target {
            BranchTargetT::Location(loc) => {
                write!(
                    f,
                    "{}{}{}",
                    self.fmt.branch_start,
                    loc.display_full(Cow::Borrowed(&*self.fmt)),
                    self.fmt.branch_end
                )
            }
            BranchTargetT::Computed(expr) => {
                write!(f, "{}", expr.display_full(Cow::Borrowed(&*self.fmt)))
            }
        }
    }
}

impl<'target, 'trans, Loc, Val, Var> TranslatorDisplay<'target, 'trans>
    for BranchTargetT<Loc, Val, Var>
where
    Loc: for<'a> TranslatorDisplay<'target, 'a> + 'target,
    Val: for<'a> TranslatorDisplay<'target, 'a> + 'target,
    Var: for<'a> TranslatorDisplay<'target, 'a> + 'target,
{
    type Target = BranchTargetTFormatter<'target, 'trans, Loc, Val, Var>;

    fn display_full(&'target self, fmt: Cow<'trans, TranslatorFormatter<'trans>>) -> Self::Target {
        BranchTargetTFormatter { target: self, fmt }
    }
}

impl<Val, Var> From<Location> for BranchTargetT<Location, Val, Var> {
    fn from(t: Location) -> Self {
        Self::Location(t)
    }
}

impl<Loc, Val, Var> BranchTargetT<Loc, Val, Var> {
    pub fn computed<E: Into<ExprT<Loc, Val, Var>>>(expr: E) -> Self {
        Self::Computed(expr.into())
    }

    pub fn is_fixed(&self) -> bool {
        !self.is_computed()
    }

    pub fn is_computed(&self) -> bool {
        matches!(self, Self::Computed(_))
    }

    pub fn location<L: Into<Loc>>(location: L) -> Self {
        Self::Location(location.into())
    }

    pub fn translate<T: TranslateIR<Loc, Val, Var>>(
        self,
        t: &T,
    ) -> BranchTargetT<T::TLoc, T::TVal, T::TVar> {
        match self {
            BranchTargetT::Location(loc) => BranchTargetT::Location(t.translate_loc(loc)),
            BranchTargetT::Computed(exp) => BranchTargetT::Computed(exp.translate(t)),
        }
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub enum Cast {
    Void,
    Bool, // T -> Bool

    Signed(usize),   // sign-extension
    Unsigned(usize), // zero-extension

    Float(Arc<FloatFormat>), // T -> FloatFormat::T

    Pointer(Box<Cast>, usize),
    Function(Box<Cast>, SmallVec<[Box<Cast>; 4]>),
    Named(Ustr, usize),
}

impl fmt::Display for Cast {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Void => write!(f, "void"),
            Self::Bool => write!(f, "bool"),
            Self::Float(format) => write!(f, "f{}", format.bits()),
            Self::Signed(bits) => write!(f, "i{}", bits),
            Self::Unsigned(bits) => write!(f, "u{}", bits),
            Self::Pointer(typ, _) => write!(f, "ptr<{}>", typ),
            Self::Function(typ, typs) => {
                write!(f, "fn(")?;
                if !typs.is_empty() {
                    write!(f, "{}", typs[0])?;
                    for typ in &typs[1..] {
                        write!(f, "{}", typ)?;
                    }
                }
                write!(f, ") -> {}", typ)
            }
            Self::Named(name, _) => write!(f, "{}", name),
        }
    }
}

impl BitSize for Cast {
    fn bits(&self) -> usize {
        match self {
            Self::Void | Self::Function(_, _) => 0, // do not have a size
            Self::Bool => 1,
            Self::Float(format) => format.bits(),
            Self::Signed(bits)
            | Self::Unsigned(bits)
            | Self::Pointer(_, bits)
            | Self::Named(_, bits) => *bits,
        }
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub enum UnOp {
    NOT,
    NEG,

    ABS,
    SQRT,
    CEILING,
    FLOOR,
    ROUND,

    POPCOUNT,
    LZCOUNT,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub enum UnRel {
    NAN,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub enum BinOp {
    AND,
    OR,
    XOR,
    ADD,
    SUB,
    DIV,
    SDIV,
    MUL,
    REM,
    SREM,
    SHL,
    SAR,
    SHR,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub enum BinRel {
    EQ,
    NEQ,
    LT,
    LE,
    SLT,
    SLE,

    SBORROW,
    CARRY,
    SCARRY,
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub enum ExprT<Loc, Val, Var> {
    UnRel(UnRel, Box<ExprT<Loc, Val, Var>>), // T -> bool
    BinRel(BinRel, Box<ExprT<Loc, Val, Var>>, Box<ExprT<Loc, Val, Var>>), // T * T -> bool

    UnOp(UnOp, Box<ExprT<Loc, Val, Var>>), // T -> T
    BinOp(BinOp, Box<ExprT<Loc, Val, Var>>, Box<ExprT<Loc, Val, Var>>), // T * T -> T

    Cast(Box<ExprT<Loc, Val, Var>>, Cast), // T -> Cast::T
    Load(Box<ExprT<Loc, Val, Var>>, usize, AddressSpaceId), // SPACE[T]:SIZE -> T

    IfElse(
        Box<ExprT<Loc, Val, Var>>,
        Box<ExprT<Loc, Val, Var>>,
        Box<ExprT<Loc, Val, Var>>,
    ), // if T then T else T

    Extract(Box<ExprT<Loc, Val, Var>>, usize, usize), // T T[LSB..MSB) -> T
    ExtractHigh(Box<ExprT<Loc, Val, Var>>, usize),
    ExtractLow(Box<ExprT<Loc, Val, Var>>, usize),

    Concat(Box<ExprT<Loc, Val, Var>>, Box<ExprT<Loc, Val, Var>>), // T * T -> T

    Call(
        Box<BranchTargetT<Loc, Val, Var>>,
        SmallVec<[Box<ExprT<Loc, Val, Var>>; 4]>,
        usize,
    ),
    Intrinsic(UserOpStr, SmallVec<[Box<ExprT<Loc, Val, Var>>; 4]>, usize),

    Val(Val), // BitVec -> T
    Var(Var), // String * usize -> T
}

impl<Loc, Val, Var> ExprT<Loc, Val, Var>
where
    Loc: fmt::Display,
    Val: fmt::Display,
    Var: fmt::Display,
{
    fn fmt_l1(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprT::Val(v) => write!(f, "{}", v),
            ExprT::Var(v) => write!(f, "{}", v),

            ExprT::Intrinsic(name, args, _) => {
                write!(f, "{}(", name)?;
                if !args.is_empty() {
                    write!(f, "{}", args[0])?;
                    for arg in &args[1..] {
                        write!(f, ", {}", arg)?;
                    }
                }
                write!(f, ")")
            }

            ExprT::ExtractHigh(expr, bits) => {
                write!(f, "extract-high({}, bits={})", expr, bits)
            }
            ExprT::ExtractLow(expr, bits) => write!(f, "extract-low({}, bits={})", expr, bits),

            ExprT::Cast(expr, t) => {
                expr.fmt_l1(f)?;
                write!(f, " as {}", t)
            }

            ExprT::Load(expr, bits, space) => {
                write!(f, "space[{}][{}]:{}", space.index(), expr, bits)
            }

            ExprT::Extract(expr, lsb, msb) => {
                write!(f, "extract({}, from={}, to={})", expr, lsb, msb)
            }

            ExprT::UnOp(UnOp::ABS, expr) => write!(f, "abs({})", expr),
            ExprT::UnOp(UnOp::SQRT, expr) => {
                write!(f, "sqrt({})", expr)
            }
            ExprT::UnOp(UnOp::ROUND, expr) => {
                write!(f, "round({})", expr)
            }
            ExprT::UnOp(UnOp::CEILING, expr) => {
                write!(f, "ceiling({})", expr)
            }
            ExprT::UnOp(UnOp::FLOOR, expr) => {
                write!(f, "floor({})", expr)
            }
            ExprT::UnOp(UnOp::POPCOUNT, expr) => {
                write!(f, "popcount({})", expr)
            },
            ExprT::UnOp(UnOp::LZCOUNT, expr) => {
                write!(f, "lzcount({})", expr)
            }

            ExprT::UnRel(UnRel::NAN, expr) => {
                write!(f, "is-nan({})", expr)
            }

            ExprT::BinRel(BinRel::CARRY, e1, e2) => write!(f, "carry({}, {})", e1, e2),
            ExprT::BinRel(BinRel::SCARRY, e1, e2) => write!(f, "scarry({}, {})", e1, e2),
            ExprT::BinRel(BinRel::SBORROW, e1, e2) => write!(f, "sborrow({}, {})", e1, e2),

            expr => write!(f, "({})", expr),
        }
    }

    fn fmt_l2(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprT::UnOp(UnOp::NEG, expr) => {
                write!(f, "-")?;
                expr.fmt_l1(f)
            }
            ExprT::UnOp(UnOp::NOT, expr) => {
                write!(f, "!")?;
                expr.fmt_l1(f)
            }
            expr => expr.fmt_l1(f),
        }
    }

    fn fmt_l3(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprT::BinOp(BinOp::MUL, e1, e2) => {
                e1.fmt_l3(f)?;
                write!(f, " * ")?;
                e2.fmt_l2(f)
            }
            ExprT::BinOp(BinOp::DIV, e1, e2) => {
                e1.fmt_l3(f)?;
                write!(f, " / ")?;
                e2.fmt_l2(f)
            }
            ExprT::BinOp(BinOp::SDIV, e1, e2) => {
                e1.fmt_l3(f)?;
                write!(f, " s/ ")?;
                e2.fmt_l2(f)
            }
            ExprT::BinOp(BinOp::REM, e1, e2) => {
                e1.fmt_l3(f)?;
                write!(f, " % ")?;
                e2.fmt_l2(f)
            }
            ExprT::BinOp(BinOp::SREM, e1, e2) => {
                e1.fmt_l3(f)?;
                write!(f, " s% ")?;
                e2.fmt_l2(f)
            }
            expr => expr.fmt_l2(f),
        }
    }

    fn fmt_l4(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprT::BinOp(BinOp::ADD, e1, e2) => {
                e1.fmt_l4(f)?;
                write!(f, " + ")?;
                e2.fmt_l3(f)
            }
            ExprT::BinOp(BinOp::SUB, e1, e2) => {
                e1.fmt_l4(f)?;
                write!(f, " - ")?;
                e2.fmt_l3(f)
            }
            expr => expr.fmt_l3(f),
        }
    }

    fn fmt_l5(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprT::BinOp(BinOp::SHL, e1, e2) => {
                e1.fmt_l5(f)?;
                write!(f, " << ")?;
                e2.fmt_l4(f)
            }
            ExprT::BinOp(BinOp::SHR, e1, e2) => {
                e1.fmt_l5(f)?;
                write!(f, " >> ")?;
                e2.fmt_l4(f)
            }
            ExprT::BinOp(BinOp::SAR, e1, e2) => {
                e1.fmt_l5(f)?;
                write!(f, " s>> ")?;
                e2.fmt_l4(f)
            }
            expr => expr.fmt_l4(f),
        }
    }

    fn fmt_l6(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprT::BinRel(BinRel::LT, e1, e2) => {
                e1.fmt_l6(f)?;
                write!(f, " < ")?;
                e2.fmt_l5(f)
            }
            ExprT::BinRel(BinRel::LE, e1, e2) => {
                e1.fmt_l6(f)?;
                write!(f, " <= ")?;
                e2.fmt_l5(f)
            }
            ExprT::BinRel(BinRel::SLT, e1, e2) => {
                e1.fmt_l6(f)?;
                write!(f, " s< ")?;
                e2.fmt_l5(f)
            }
            ExprT::BinRel(BinRel::SLE, e1, e2) => {
                e1.fmt_l6(f)?;
                write!(f, " s<= ")?;
                e2.fmt_l5(f)
            }
            expr => expr.fmt_l5(f),
        }
    }

    fn fmt_l7(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprT::BinRel(BinRel::EQ, e1, e2) => {
                e1.fmt_l7(f)?;
                write!(f, " == ")?;
                e2.fmt_l6(f)
            }
            ExprT::BinRel(BinRel::NEQ, e1, e2) => {
                e1.fmt_l7(f)?;
                write!(f, " != ")?;
                e2.fmt_l6(f)
            }
            expr => expr.fmt_l6(f),
        }
    }

    fn fmt_l8(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let ExprT::BinOp(BinOp::AND, e1, e2) = self {
            e1.fmt_l8(f)?;
            write!(f, " & ")?;
            e2.fmt_l7(f)
        } else {
            self.fmt_l7(f)
        }
    }

    fn fmt_l9(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let ExprT::BinOp(BinOp::XOR, e1, e2) = self {
            e1.fmt_l9(f)?;
            write!(f, " ^ ")?;
            e2.fmt_l8(f)
        } else {
            self.fmt_l8(f)
        }
    }

    fn fmt_l10(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let ExprT::BinOp(BinOp::OR, e1, e2) = self {
            e1.fmt_l10(f)?;
            write!(f, " | ")?;
            e2.fmt_l9(f)
        } else {
            self.fmt_l9(f)
        }
    }

    fn fmt_l11(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let ExprT::Concat(e1, e2) = self {
            e1.fmt_l11(f)?;
            write!(f, " ++ ")?;
            e2.fmt_l10(f)
        } else {
            self.fmt_l10(f)
        }
    }

    fn fmt_l12(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let ExprT::IfElse(c, et, ef) = self {
            write!(f, "if ")?;
            c.fmt_l12(f)?;
            write!(f, " then ")?;
            et.fmt_l12(f)?;
            write!(f, " else ")?;
            ef.fmt_l12(f)
        } else {
            self.fmt_l11(f)
        }
    }
}

impl<'v, 't, Loc, Val, Var> ExprT<Loc, Val, Var>
where
    Loc: for<'a> TranslatorDisplay<'v, 'a>,
    Val: for<'a> TranslatorDisplay<'v, 'a>,
    Var: for<'a> TranslatorDisplay<'v, 'a>,
{
    fn fmt_l1_with(
        &'v self,
        f: &mut fmt::Formatter<'_>,
        d: &ExprTFormatter<'v, 't, Loc, Val, Var>,
    ) -> fmt::Result {
        match self {
            ExprT::Val(v) => write!(f, "{}", v.display_full(Cow::Borrowed(&*d.fmt)),),
            ExprT::Var(v) => write!(f, "{}", v.display_full(Cow::Borrowed(&*d.fmt))),

            ExprT::Intrinsic(name, args, _) => {
                write!(f, "{}{}{}(", d.fmt.keyword_start, name, d.fmt.keyword_end)?;
                if !args.is_empty() {
                    write!(f, "{}", args[0].display_full(Cow::Borrowed(&*d.fmt)))?;
                    for arg in &args[1..] {
                        write!(f, ", {}", arg.display_full(Cow::Borrowed(&*d.fmt)))?;
                    }
                }
                write!(f, ")")
            }

            ExprT::ExtractHigh(expr, bits) => write!(
                f,
                "{}extract-high{}({}, {}bits{}={}{}{})",
                d.fmt.keyword_start,
                d.fmt.keyword_end,
                expr.display_full(Cow::Borrowed(&*d.fmt)),
                d.fmt.keyword_start,
                d.fmt.keyword_end,
                d.fmt.value_start,
                bits,
                d.fmt.value_end,
            ),
            ExprT::ExtractLow(expr, bits) => write!(
                f,
                "{}extract-low{}({}, {}bits{}={}{}{})",
                d.fmt.keyword_start,
                d.fmt.keyword_end,
                expr.display_full(Cow::Borrowed(&*d.fmt)),
                d.fmt.keyword_start,
                d.fmt.keyword_end,
                d.fmt.value_start,
                bits,
                d.fmt.value_end,
            ),

            ExprT::Cast(expr, t) => {
                expr.fmt_l1_with(f, d)?;
                write!(
                    f,
                    " {}as{} {}{}{}",
                    d.fmt.keyword_start, d.fmt.keyword_end, d.fmt.type_start, t, d.fmt.type_end
                )
            }

            ExprT::Load(expr, bits, space) => {
                if let Some(trans) = d.fmt.translator {
                    let space = trans.manager().unchecked_space_by_id(*space);
                    write!(
                        f,
                        "{}{}{}[{}]:{}{}{}",
                        d.fmt.variable_start,
                        space.name(),
                        d.fmt.variable_end,
                        expr.display_full(Cow::Borrowed(&*d.fmt)),
                        d.fmt.value_start,
                        bits,
                        d.fmt.value_end,
                    )
                } else {
                    write!(
                        f,
                        "{}space{}[{}{}{}][{}]:{}{}{}",
                        d.fmt.variable_start,
                        d.fmt.variable_end,
                        d.fmt.value_start,
                        space.index(),
                        d.fmt.value_end,
                        expr.display_full(Cow::Borrowed(&*d.fmt)),
                        d.fmt.value_start,
                        bits,
                        d.fmt.value_end,
                    )
                }
            }

            ExprT::Extract(expr, lsb, msb) => write!(
                f,
                "{}extract{}({}, {}from{}={}{}{}, {}to{}={}{}{})",
                d.fmt.keyword_start,
                d.fmt.keyword_end,
                expr.display_full(Cow::Borrowed(&*d.fmt)),
                d.fmt.keyword_start,
                d.fmt.keyword_end,
                d.fmt.value_start,
                lsb,
                d.fmt.value_end,
                d.fmt.keyword_start,
                d.fmt.keyword_end,
                d.fmt.value_start,
                msb,
                d.fmt.value_end,
            ),

            ExprT::UnOp(UnOp::ABS, expr) => {
                write!(
                    f,
                    "{}abs{}({})",
                    d.fmt.keyword_start,
                    d.fmt.keyword_end,
                    expr.display_full(Cow::Borrowed(&*d.fmt))
                )
            }
            ExprT::UnOp(UnOp::SQRT, expr) => {
                write!(
                    f,
                    "{}sqrt{}({})",
                    d.fmt.keyword_start,
                    d.fmt.keyword_end,
                    expr.display_full(Cow::Borrowed(&*d.fmt))
                )
            }
            ExprT::UnOp(UnOp::ROUND, expr) => {
                write!(
                    f,
                    "{}round{}({})",
                    d.fmt.keyword_start,
                    d.fmt.keyword_end,
                    expr.display_full(Cow::Borrowed(&*d.fmt))
                )
            }
            ExprT::UnOp(UnOp::CEILING, expr) => {
                write!(
                    f,
                    "{}ceiling{}({})",
                    d.fmt.keyword_start,
                    d.fmt.keyword_end,
                    expr.display_full(Cow::Borrowed(&*d.fmt))
                )
            }
            ExprT::UnOp(UnOp::FLOOR, expr) => {
                write!(
                    f,
                    "{}floor{}({})",
                    d.fmt.keyword_start,
                    d.fmt.keyword_end,
                    expr.display_full(Cow::Borrowed(&*d.fmt))
                )
            }
            ExprT::UnOp(UnOp::POPCOUNT, expr) => {
                write!(
                    f,
                    "{}popcount{}({})",
                    d.fmt.keyword_start,
                    d.fmt.keyword_end,
                    expr.display_full(Cow::Borrowed(&*d.fmt))
                )
            }
            ExprT::UnOp(UnOp::LZCOUNT, expr) => {
                write!(
                    f,
                    "{}lzcount{}({})",
                    d.fmt.keyword_start,
                    d.fmt.keyword_end,
                    expr.display_full(Cow::Borrowed(&*d.fmt))
                )
            }

            ExprT::UnRel(UnRel::NAN, expr) => {
                write!(
                    f,
                    "{}is-nan{}({})",
                    d.fmt.keyword_start,
                    d.fmt.keyword_end,
                    expr.display_full(Cow::Borrowed(&*d.fmt))
                )
            }

            ExprT::BinRel(BinRel::CARRY, e1, e2) => write!(
                f,
                "{}carry{}({}, {})",
                d.fmt.keyword_start,
                d.fmt.keyword_end,
                e1.display_full(Cow::Borrowed(&*d.fmt)),
                e2.display_full(Cow::Borrowed(&*d.fmt))
            ),
            ExprT::BinRel(BinRel::SCARRY, e1, e2) => write!(
                f,
                "{}scarry{}({}, {})",
                d.fmt.keyword_start,
                d.fmt.keyword_end,
                e1.display_full(Cow::Borrowed(&*d.fmt)),
                e2.display_full(Cow::Borrowed(&*d.fmt))
            ),
            ExprT::BinRel(BinRel::SBORROW, e1, e2) => write!(
                f,
                "{}sborrow{}({}, {})",
                d.fmt.keyword_start,
                d.fmt.keyword_end,
                e1.display_full(Cow::Borrowed(&*d.fmt)),
                e2.display_full(Cow::Borrowed(&*d.fmt))
            ),

            expr => write!(f, "({})", expr.display_full(Cow::Borrowed(&*d.fmt))),
        }
    }

    fn fmt_l2_with(
        &'v self,
        f: &mut fmt::Formatter<'_>,
        d: &ExprTFormatter<'v, 't, Loc, Val, Var>,
    ) -> fmt::Result {
        match self {
            ExprT::UnOp(UnOp::NEG, expr) => {
                write!(f, "{}-{}", d.fmt.keyword_start, d.fmt.keyword_end)?;
                expr.fmt_l1_with(f, d)
            }
            ExprT::UnOp(UnOp::NOT, expr) => {
                write!(f, "{}!{}", d.fmt.keyword_start, d.fmt.keyword_end)?;
                expr.fmt_l1_with(f, d)
            }
            expr => expr.fmt_l1_with(f, d),
        }
    }

    fn fmt_l3_with(
        &'v self,
        f: &mut fmt::Formatter<'_>,
        d: &ExprTFormatter<'v, 't, Loc, Val, Var>,
    ) -> fmt::Result {
        match self {
            ExprT::BinOp(BinOp::MUL, e1, e2) => {
                e1.fmt_l3_with(f, d)?;
                write!(f, " {}*{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l2_with(f, d)
            }
            ExprT::BinOp(BinOp::DIV, e1, e2) => {
                e1.fmt_l3_with(f, d)?;
                write!(f, " {}/{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l2_with(f, d)
            }
            ExprT::BinOp(BinOp::SDIV, e1, e2) => {
                e1.fmt_l3_with(f, d)?;
                write!(f, " {}s/{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l2_with(f, d)
            }
            ExprT::BinOp(BinOp::REM, e1, e2) => {
                e1.fmt_l3_with(f, d)?;
                write!(f, " {}%{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l2_with(f, d)
            }
            ExprT::BinOp(BinOp::SREM, e1, e2) => {
                e1.fmt_l3_with(f, d)?;
                write!(f, " {}s%{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l2_with(f, d)
            }
            expr => expr.fmt_l2_with(f, d),
        }
    }

    fn fmt_l4_with(
        &'v self,
        f: &mut fmt::Formatter<'_>,
        d: &ExprTFormatter<'v, 't, Loc, Val, Var>,
    ) -> fmt::Result {
        match self {
            ExprT::BinOp(BinOp::ADD, e1, e2) => {
                e1.fmt_l4_with(f, d)?;
                write!(f, " {}+{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l3_with(f, d)
            }
            ExprT::BinOp(BinOp::SUB, e1, e2) => {
                e1.fmt_l4_with(f, d)?;
                write!(f, " {}-{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l3_with(f, d)
            }
            expr => expr.fmt_l3_with(f, d),
        }
    }

    fn fmt_l5_with(
        &'v self,
        f: &mut fmt::Formatter<'_>,
        d: &ExprTFormatter<'v, 't, Loc, Val, Var>,
    ) -> fmt::Result {
        match self {
            ExprT::BinOp(BinOp::SHL, e1, e2) => {
                e1.fmt_l5_with(f, d)?;
                write!(f, " {}<<{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l4_with(f, d)
            }
            ExprT::BinOp(BinOp::SHR, e1, e2) => {
                e1.fmt_l5_with(f, d)?;
                write!(f, " {}>>{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l4_with(f, d)
            }
            ExprT::BinOp(BinOp::SAR, e1, e2) => {
                e1.fmt_l5_with(f, d)?;
                write!(f, " {}s>>{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l4_with(f, d)
            }
            expr => expr.fmt_l4_with(f, d),
        }
    }

    fn fmt_l6_with(
        &'v self,
        f: &mut fmt::Formatter<'_>,
        d: &ExprTFormatter<'v, 't, Loc, Val, Var>,
    ) -> fmt::Result {
        match self {
            ExprT::BinRel(BinRel::LT, e1, e2) => {
                e1.fmt_l6_with(f, d)?;
                write!(f, " {}<{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l5_with(f, d)
            }
            ExprT::BinRel(BinRel::LE, e1, e2) => {
                e1.fmt_l6_with(f, d)?;
                write!(f, " {}<={} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l5_with(f, d)
            }
            ExprT::BinRel(BinRel::SLT, e1, e2) => {
                e1.fmt_l6_with(f, d)?;
                write!(f, " {}s<{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l5_with(f, d)
            }
            ExprT::BinRel(BinRel::SLE, e1, e2) => {
                e1.fmt_l6_with(f, d)?;
                write!(f, " {}s<={} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l5_with(f, d)
            }
            expr => expr.fmt_l5_with(f, d),
        }
    }

    fn fmt_l7_with(
        &'v self,
        f: &mut fmt::Formatter<'_>,
        d: &ExprTFormatter<'v, 't, Loc, Val, Var>,
    ) -> fmt::Result {
        match self {
            ExprT::BinRel(BinRel::EQ, e1, e2) => {
                e1.fmt_l7_with(f, d)?;
                write!(f, " {}=={} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l6_with(f, d)
            }
            ExprT::BinRel(BinRel::NEQ, e1, e2) => {
                e1.fmt_l7_with(f, d)?;
                write!(f, " {}!={} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
                e2.fmt_l6_with(f, d)
            }
            expr => expr.fmt_l6_with(f, d),
        }
    }

    fn fmt_l8_with(
        &'v self,
        f: &mut fmt::Formatter<'_>,
        d: &ExprTFormatter<'v, 't, Loc, Val, Var>,
    ) -> fmt::Result {
        if let ExprT::BinOp(BinOp::AND, e1, e2) = self {
            e1.fmt_l8_with(f, d)?;
            write!(f, " {}&{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
            e2.fmt_l7_with(f, d)
        } else {
            self.fmt_l7_with(f, d)
        }
    }

    fn fmt_l9_with(
        &'v self,
        f: &mut fmt::Formatter<'_>,
        d: &ExprTFormatter<'v, 't, Loc, Val, Var>,
    ) -> fmt::Result {
        if let ExprT::BinOp(BinOp::XOR, e1, e2) = self {
            e1.fmt_l9_with(f, d)?;
            write!(f, " {}^{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
            e2.fmt_l8_with(f, d)
        } else {
            self.fmt_l8_with(f, d)
        }
    }

    fn fmt_l10_with(
        &'v self,
        f: &mut fmt::Formatter<'_>,
        d: &ExprTFormatter<'v, 't, Loc, Val, Var>,
    ) -> fmt::Result {
        if let ExprT::BinOp(BinOp::OR, e1, e2) = self {
            e1.fmt_l10_with(f, d)?;
            write!(f, " {}|{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
            e2.fmt_l9_with(f, d)
        } else {
            self.fmt_l9_with(f, d)
        }
    }

    fn fmt_l11_with(
        &'v self,
        f: &mut fmt::Formatter<'_>,
        d: &ExprTFormatter<'v, 't, Loc, Val, Var>,
    ) -> fmt::Result {
        if let ExprT::Concat(e1, e2) = self {
            e1.fmt_l11_with(f, d)?;
            write!(f, " {}++{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
            e2.fmt_l10_with(f, d)
        } else {
            self.fmt_l10_with(f, d)
        }
    }

    fn fmt_l12_with(
        &'v self,
        f: &mut fmt::Formatter<'_>,
        d: &ExprTFormatter<'v, 't, Loc, Val, Var>,
    ) -> fmt::Result {
        if let ExprT::IfElse(c, et, ef) = self {
            write!(f, "{}if{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
            c.fmt_l12_with(f, d)?;
            write!(f, " {}then{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
            et.fmt_l12_with(f, d)?;
            write!(f, " {}else{} ", d.fmt.keyword_start, d.fmt.keyword_end)?;
            ef.fmt_l12_with(f, d)
        } else {
            self.fmt_l11_with(f, d)
        }
    }
}

impl<Loc, Val, Var> fmt::Display for ExprT<Loc, Val, Var>
where
    Loc: fmt::Display,
    Val: fmt::Display,
    Var: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_l12(f)
    }
}

pub struct ExprTFormatter<'expr, 'trans, Loc, Val, Var> {
    expr: &'expr ExprT<Loc, Val, Var>,
    fmt: Cow<'trans, TranslatorFormatter<'trans>>,
}

impl<'expr, 'trans, Loc, Val, Var> fmt::Display for ExprTFormatter<'expr, 'trans, Loc, Val, Var>
where
    Loc: for<'a> TranslatorDisplay<'expr, 'a> + 'expr,
    Val: for<'a> TranslatorDisplay<'expr, 'a> + 'expr,
    Var: for<'a> TranslatorDisplay<'expr, 'a> + 'expr,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.expr.fmt_l12_with(f, self)
    }
}

impl<'expr, 'trans, Loc, Val, Var> TranslatorDisplay<'expr, 'trans> for ExprT<Loc, Val, Var>
where
    Loc: for<'a> TranslatorDisplay<'expr, 'a> + 'expr,
    Val: for<'a> TranslatorDisplay<'expr, 'a> + 'expr,
    Var: for<'a> TranslatorDisplay<'expr, 'a> + 'expr,
{
    type Target = ExprTFormatter<'expr, 'trans, Loc, Val, Var>;

    fn display_full(&'expr self, fmt: Cow<'trans, TranslatorFormatter<'trans>>) -> Self::Target {
        ExprTFormatter { expr: self, fmt }
    }
}

impl<Loc, Var> From<BitVec> for ExprT<Loc, BitVec, Var> {
    fn from(val: BitVec) -> Self {
        Self::Val(val)
    }
}

impl<Loc, Val> From<Var> for ExprT<Loc, Val, Var> {
    fn from(var: Var) -> Self {
        Self::Var(var)
    }
}

impl<'z, Loc> FromSpace<'z, VarnodeData> for ExprT<Loc, BitVec, Var> {
    fn from_space_with(t: VarnodeData, _arena: &'_ IRBuilderArena, manager: &SpaceManager) -> Self {
        ExprT::from_space(t, manager)
    }

    fn from_space(vnd: VarnodeData, manager: &SpaceManager) -> ExprT<Loc, BitVec, Var> {
        let space = manager.unchecked_space_by_id(vnd.space());
        if space.is_constant() {
            ExprT::from(BitVec::from_u64(vnd.offset(), vnd.size() * 8))
        } else {
            // if space.is_unique() || space.is_register() {
            ExprT::from(Var::from(vnd))
        } /* else {
              // address-like: the vnd size is what it points to
              let asz = space.address_size() * 8;
              let val = BitVec::from_u64(vnd.offset(), asz);
              let src = if space.word_size() > 1 {
                  let s = ExprT::from(val);
                  let bits = s.bits();

                  let w = ExprT::from(BitVec::from_usize(space.word_size(), bits));

                  ExprT::int_mul(s, w)
              } else {
                  ExprT::from(val)
              };
              // TODO: we should preserve this information!!!
              //ExprT::Cast(Box::new(src), Cast::Pointer(Box::new(Cast::Void), asz))
              ExprT::load(
                  src,
                  vnd.space().address_size() * 8,
                  vnd.space(),
              )
          }
          */
    }
}

impl<Loc, Val, Var> BitSize for ExprT<Loc, Val, Var>
where
    Val: BitSize,
    Var: BitSize,
{
    fn bits(&self) -> usize {
        match self {
            Self::UnRel(_, _) | Self::BinRel(_, _, _) => 1,
            Self::UnOp(_, e) | Self::BinOp(_, e, _) => e.bits(),
            Self::Cast(_, cast) => cast.bits(),
            Self::Load(_, bits, _) => *bits,
            Self::Extract(_, lsb, msb) => *msb - *lsb,
            Self::ExtractHigh(_, bits) | Self::ExtractLow(_, bits) => *bits,
            Self::Concat(l, r) => l.bits() + r.bits(),
            Self::IfElse(_, e, _) => e.bits(),
            Self::Call(_, _, bits) => *bits,
            Self::Intrinsic(_, _, bits) => *bits,
            Self::Val(bv) => bv.bits(),
            Self::Var(var) => var.bits(),
        }
    }
}

impl<Loc, Val, Var> ExprT<Loc, Val, Var>
where
    Val: BitSize,
    Var: BitSize,
{
    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Cast(_, Cast::Bool))
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Self::Cast(_, Cast::Float(_)))
    }

    pub fn is_float_format(&self, format: &FloatFormat) -> bool {
        matches!(self, Self::Cast(_, Cast::Float(f)) if &**f == format)
    }

    pub fn is_signed(&self) -> bool {
        matches!(self, Self::Cast(_, Cast::Signed(_)))
    }

    pub fn is_signed_bits(&self, bits: usize) -> bool {
        matches!(self, Self::Cast(_, Cast::Signed(sz)) if *sz == bits)
    }

    pub fn is_unsigned(&self) -> bool {
        matches!(self, Self::Cast(_, Cast::Unsigned(_) | Cast::Pointer(_, _)))
            || !matches!(self, Self::Cast(_, _))
    }

    pub fn is_unsigned_bits(&self, bits: usize) -> bool {
        matches!(self, Self::Cast(_, Cast::Unsigned(sz) | Cast::Pointer(_, sz)) if *sz == bits)
            || (!matches!(self, Self::Cast(_, _)) && self.bits() == bits)
    }

    pub fn value(&self) -> Option<&Val> {
        if let Self::Val(ref v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn cast_bool<E>(expr: E) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        if expr.is_bool() {
            expr
        } else {
            Self::Cast(expr.into(), Cast::Bool)
        }
    }

    pub fn cast_signed<E>(expr: E, bits: usize) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        if expr.is_signed_bits(bits) {
            expr
        } else {
            Self::Cast(expr.into(), Cast::Signed(bits))
        }
    }

    pub fn cast_unsigned<E>(expr: E, bits: usize) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        if expr.is_unsigned_bits(bits) {
            expr
        } else {
            Self::Cast(expr.into(), Cast::Unsigned(bits))
        }
    }

    pub fn cast_float<E>(expr: E, format: Arc<FloatFormat>) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        if expr.is_float_format(&*format) {
            expr
        } else {
            Self::Cast(expr.into(), Cast::Float(format))
        }
    }

    pub fn cast<E>(expr: E, bits: usize) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        Self::Cast(expr.into(), Cast::Unsigned(bits))
    }

    pub fn extract_high<E>(expr: E, bits: usize) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        if expr.is_unsigned_bits(bits) {
            expr
        } else {
            Self::ExtractHigh(expr.into(), bits)
        }
    }

    pub fn extract_low<E>(expr: E, bits: usize) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        if expr.is_unsigned_bits(bits) {
            expr
        } else {
            Self::ExtractLow(expr.into(), bits)
        }
    }

    pub fn concat<E1, E2>(lhs: E1, rhs: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        let lhs = lhs.into();
        let rhs = rhs.into();
        Self::Concat(Box::new(lhs), Box::new(rhs))
    }

    pub(crate) fn unary_op<E>(op: UnOp, expr: E) -> Self
    where
        E: Into<Self>,
    {
        Self::UnOp(op, Box::new(expr.into()))
    }

    pub(crate) fn unary_rel<E>(rel: UnRel, expr: E) -> Self
    where
        E: Into<Self>,
    {
        Self::cast_bool(Self::UnRel(rel, Box::new(expr.into())))
    }

    pub(crate) fn binary_op<E1, E2>(op: BinOp, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::BinOp(op, Box::new(expr1.into()), Box::new(expr2.into()))
    }

    pub(crate) fn binary_op_promote_as<E1, E2, F>(op: BinOp, expr1: E1, expr2: E2, cast: F) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
        F: Fn(Self, usize) -> Self,
    {
        let e1 = expr1.into();
        let e2 = expr2.into();
        let bits = e1.bits().max(e2.bits());

        Self::binary_op(op, cast(e1, bits), cast(e2, bits))
    }

    pub(crate) fn binary_op_promote<E1, E2>(op: BinOp, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote_as(op, expr1, expr2, |e, sz| Self::cast_unsigned(e, sz))
    }

    pub(crate) fn binary_op_promote_bool<E1, E2>(op: BinOp, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote_as(op, expr1, expr2, |e, _sz| Self::cast_bool(e))
    }

    pub(crate) fn binary_op_promote_signed<E1, E2>(op: BinOp, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote_as(op, expr1, expr2, |e, sz| Self::cast_signed(e, sz))
    }

    pub(crate) fn binary_op_promote_float<E1, E2>(
        op: BinOp,
        expr1: E1,
        expr2: E2,
        formats: &FloatFormats,
    ) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote_as(op, expr1, expr2, |e, sz| {
            Self::cast_float(Self::cast_signed(e, sz), formats[&sz].clone())
        })
    }

    pub(crate) fn binary_rel<E1, E2>(rel: BinRel, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::cast_bool(Self::BinRel(
            rel,
            Box::new(expr1.into()),
            Box::new(expr2.into()),
        ))
    }

    pub(crate) fn binary_rel_promote_as<E1, E2, F>(
        op: BinRel,
        expr1: E1,
        expr2: E2,
        cast: F,
    ) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
        F: Fn(Self, usize) -> Self,
    {
        let e1 = expr1.into();
        let e2 = expr2.into();
        let bits = e1.bits().max(e2.bits());

        Self::binary_rel(op, cast(e1, bits), cast(e2, bits))
    }

    pub(crate) fn binary_rel_promote<E1, E2>(op: BinRel, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote_as(op, expr1, expr2, |e, sz| Self::cast_unsigned(e, sz))
    }

    pub(crate) fn binary_rel_promote_float<E1, E2>(
        op: BinRel,
        expr1: E1,
        expr2: E2,
        formats: &FloatFormats,
    ) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote_as(op, expr1, expr2, |e, sz| {
            Self::cast_float(Self::cast_signed(e, sz), formats[&sz].clone())
        })
    }

    pub(crate) fn binary_rel_promote_signed<E1, E2>(op: BinRel, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote_as(op, expr1, expr2, |e, sz| Self::cast_signed(e, sz))
    }

    pub(crate) fn binary_rel_promote_bool<E1, E2>(op: BinRel, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote_as(op, expr1, expr2, |e, _sz| Self::cast_bool(e))
    }

    pub fn load<E>(expr: E, size: usize, space: &AddressSpace) -> Self
    where
        E: Into<Self>,
    {
        Self::Load(
            Box::new(Self::cast_unsigned(expr, space.address_size() * 8)),
            size,
            space.id(),
        )
    }

    pub fn call<T>(target: T, bits: usize) -> Self
    where
        T: Into<BranchTargetT<Loc, Val, Var>>,
    {
        Self::Call(Box::new(target.into()), Default::default(), bits)
    }

    pub fn call_with<T, I, E>(target: T, arguments: I, bits: usize) -> Self
    where
        T: Into<BranchTargetT<Loc, Val, Var>>,
        I: ExactSizeIterator<Item = E>,
        E: Into<Self>,
    {
        let mut args = SmallVec::with_capacity(arguments.len());
        for arg in arguments {
            args.push(Box::new(arg.into()));
        }

        Self::Call(Box::new(target.into()), args, bits)
    }

    pub fn intrinsic<N, I, E>(name: N, arguments: I, bits: usize) -> Self
    where
        N: Into<Ustr>,
        I: ExactSizeIterator<Item = E>,
        E: Into<Self>,
    {
        let mut args = SmallVec::with_capacity(arguments.len());
        for arg in arguments {
            args.push(Box::new(arg.into()));
        }

        Self::Intrinsic(name.into(), args, bits)
    }

    pub fn extract<E>(expr: E, loff: usize, moff: usize) -> Self
    where
        E: Into<Self>,
    {
        Self::Extract(Box::new(expr.into()), loff, moff)
    }

    pub fn ite<C, E1, E2>(cond: C, texpr: E1, fexpr: E2) -> Self
    where
        C: Into<Self>,
        E1: Into<Self>,
        E2: Into<Self>,
    {
        let e1 = texpr.into();
        let e2 = fexpr.into();
        let bits = e1.bits().max(e2.bits());

        Self::IfElse(
            Box::new(Self::cast_bool(cond)),
            Box::new(Self::cast_unsigned(e1, bits)),
            Box::new(Self::cast_unsigned(e2, bits)),
        )
    }

    pub fn bool_not<E>(expr: E) -> Self
    where
        E: Into<Self>,
    {
        Self::unary_op(UnOp::NOT, Self::cast_bool(expr))
    }

    pub fn bool_eq<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote_bool(BinRel::EQ, expr1, expr2)
    }

    pub fn bool_neq<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote_bool(BinRel::NEQ, expr1, expr2)
    }

    pub fn bool_and<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote_bool(BinOp::AND, expr1, expr2)
    }

    pub fn bool_or<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote_bool(BinOp::OR, expr1, expr2)
    }

    pub fn bool_xor<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote_bool(BinOp::XOR, expr1, expr2)
    }

    pub fn float_nan<E>(expr: E, formats: &FloatFormats) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        let bits = expr.bits();

        let format = formats[&bits].clone();

        Self::unary_rel(
            UnRel::NAN,
            ExprT::cast_float(ExprT::cast_signed(expr, bits), format),
        )
    }

    pub fn float_neg<E>(expr: E, formats: &FloatFormats) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits].clone();

        Self::unary_op(
            UnOp::NEG,
            ExprT::cast_float(ExprT::cast_signed(expr, bits), format),
        )
    }

    pub fn float_abs<E>(expr: E, formats: &FloatFormats) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits].clone();

        Self::unary_op(
            UnOp::ABS,
            ExprT::cast_float(ExprT::cast_signed(expr, bits), format),
        )
    }

    pub fn float_sqrt<E>(expr: E, formats: &FloatFormats) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits].clone();

        Self::unary_op(
            UnOp::SQRT,
            ExprT::cast_float(ExprT::cast_signed(expr, bits), format),
        )
    }

    pub fn float_ceiling<E>(expr: E, formats: &FloatFormats) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits].clone();

        Self::unary_op(
            UnOp::CEILING,
            ExprT::cast_float(ExprT::cast_signed(expr, bits), format),
        )
    }

    pub fn float_round<E>(expr: E, formats: &FloatFormats) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits].clone();

        Self::unary_op(
            UnOp::ROUND,
            ExprT::cast_float(ExprT::cast_signed(expr, bits), format),
        )
    }

    pub fn float_floor<E>(expr: E, formats: &FloatFormats) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits].clone();

        Self::unary_op(
            UnOp::FLOOR,
            ExprT::cast_float(ExprT::cast_signed(expr, bits), format),
        )
    }

    pub fn float_eq<E1, E2>(expr1: E1, expr2: E2, formats: &FloatFormats) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote_float(BinRel::EQ, expr1, expr2, formats)
    }

    pub fn float_neq<E1, E2>(expr1: E1, expr2: E2, formats: &FloatFormats) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote_float(BinRel::NEQ, expr1, expr2, formats)
    }

    pub fn float_lt<E1, E2>(expr1: E1, expr2: E2, formats: &FloatFormats) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote_float(BinRel::LT, expr1, expr2, formats)
    }

    pub fn float_le<E1, E2>(expr1: E1, expr2: E2, formats: &FloatFormats) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote_float(BinRel::LE, expr1, expr2, formats)
    }

    pub fn float_add<E1, E2>(expr1: E1, expr2: E2, formats: &FloatFormats) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote_float(BinOp::ADD, expr1, expr2, formats)
    }

    pub fn float_sub<E1, E2>(expr1: E1, expr2: E2, formats: &FloatFormats) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote_float(BinOp::SUB, expr1, expr2, formats)
    }

    pub fn float_div<E1, E2>(expr1: E1, expr2: E2, formats: &FloatFormats) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote_float(BinOp::DIV, expr1, expr2, formats)
    }

    pub fn float_mul<E1, E2>(expr1: E1, expr2: E2, formats: &FloatFormats) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote_float(BinOp::MUL, expr1, expr2, formats)
    }

    pub fn count_ones<E>(expr: E) -> Self
    where
        E: Into<Self>,
    {
        Self::unary_op(UnOp::POPCOUNT, expr.into())
    }

    pub fn count_leading_zeros<E>(expr: E) -> Self
    where
        E: Into<Self>,
    {
        Self::unary_op(UnOp::LZCOUNT, expr.into())
    }

    pub fn int_neg<E>(expr: E) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        let size = expr.bits();
        Self::unary_op(UnOp::NEG, Self::cast_signed(expr, size))
    }

    pub fn int_not<E>(expr: E) -> Self
    where
        E: Into<Self>,
    {
        let expr = expr.into();
        let size = expr.bits();
        Self::unary_op(UnOp::NOT, Self::cast_unsigned(expr, size))
    }

    pub fn int_eq<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote(BinRel::EQ, expr1, expr2)
    }

    pub fn int_neq<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote(BinRel::NEQ, expr1, expr2)
    }

    pub fn int_lt<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote(BinRel::LT, expr1, expr2)
    }

    pub fn int_le<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote(BinRel::LE, expr1, expr2)
    }

    pub fn int_slt<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote_signed(BinRel::SLT, expr1, expr2)
    }

    pub fn int_sle<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote_signed(BinRel::SLE, expr1, expr2)
    }

    pub fn int_carry<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote(BinRel::CARRY, expr1, expr2)
    }

    pub fn int_scarry<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote_signed(BinRel::SCARRY, expr1, expr2)
    }

    pub fn int_sborrow<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_rel_promote_signed(BinRel::SBORROW, expr1, expr2)
    }

    pub fn int_add<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote(BinOp::ADD, expr1, expr2)
    }

    pub fn int_sub<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote(BinOp::SUB, expr1, expr2)
    }

    pub fn int_mul<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote(BinOp::MUL, expr1, expr2)
    }

    pub fn int_div<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote(BinOp::DIV, expr1, expr2)
    }

    pub fn int_sdiv<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote_signed(BinOp::SDIV, expr1, expr2)
    }

    pub fn int_rem<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote(BinOp::REM, expr1, expr2)
    }

    pub fn int_srem<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote_signed(BinOp::SREM, expr1, expr2)
    }

    pub fn int_shl<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote(BinOp::SHL, expr1, expr2)
    }

    pub fn int_shr<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote(BinOp::SHR, expr1, expr2)
    }

    pub fn int_sar<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote_signed(BinOp::SAR, expr1, expr2)
    }

    pub fn int_and<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote(BinOp::AND, expr1, expr2)
    }

    pub fn int_or<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote(BinOp::OR, expr1, expr2)
    }

    pub fn int_xor<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Self>,
        E2: Into<Self>,
    {
        Self::binary_op_promote(BinOp::XOR, expr1, expr2)
    }
}

impl<Loc, Val, Var> ExprT<Loc, Val, Var> {
    pub fn translate<T: TranslateIR<Loc, Val, Var>>(
        self,
        t: &T,
    ) -> ExprT<T::TLoc, T::TVal, T::TVar> {
        match self {
            ExprT::Val(val) => ExprT::Val(t.translate_val(val)),
            ExprT::Var(var) => ExprT::Var(t.translate_var(var)),
            ExprT::UnOp(op, e) => ExprT::UnOp(op, Box::new(e.translate(t))),
            ExprT::UnRel(op, e) => ExprT::UnRel(op, Box::new(e.translate(t))),
            ExprT::BinOp(op, e1, e2) => {
                ExprT::BinOp(op, Box::new(e1.translate(t)), Box::new(e2.translate(t)))
            }
            ExprT::BinRel(op, e1, e2) => {
                ExprT::BinRel(op, Box::new(e1.translate(t)), Box::new(e2.translate(t)))
            }
            ExprT::Cast(e, c) => ExprT::Cast(Box::new(e.translate(t)), c),
            ExprT::Load(e, sz, spc) => ExprT::Load(Box::new(e.translate(t)), sz, spc),
            ExprT::IfElse(c, et, ef) => ExprT::IfElse(
                Box::new(c.translate(t)),
                Box::new(et.translate(t)),
                Box::new(ef.translate(t)),
            ),
            ExprT::Concat(e1, e2) => {
                ExprT::Concat(Box::new(e1.translate(t)), Box::new(e2.translate(t)))
            }
            ExprT::Extract(e, l, m) => ExprT::Extract(Box::new(e.translate(t)), l, m),
            ExprT::ExtractHigh(e, b) => ExprT::ExtractHigh(Box::new(e.translate(t)), b),
            ExprT::ExtractLow(e, b) => ExprT::ExtractLow(Box::new(e.translate(t)), b),
            ExprT::Intrinsic(name, args, sz) => ExprT::Intrinsic(
                name,
                args.into_iter().map(|e| Box::new(e.translate(t))).collect(),
                sz,
            ),
            ExprT::Call(bt, args, sz) => ExprT::Call(
                Box::new(bt.translate(t)),
                args.into_iter().map(|e| Box::new(e.translate(t))).collect(),
                sz,
            ),
        }
    }
}

impl<Loc, Val, Var> StmtT<Loc, Val, Var> {
    pub fn translate<T: TranslateIR<Loc, Val, Var>>(
        self,
        t: &T,
    ) -> StmtT<T::TLoc, T::TVal, T::TVar> {
        match self {
            StmtT::Assign(v, e) => StmtT::Assign(t.translate_var(v), e.translate(t)),
            StmtT::Store(e1, e2, sz, spc) => {
                StmtT::Store(e1.translate(t), e2.translate(t), sz, spc)
            }
            StmtT::Branch(bt) => StmtT::Branch(bt.translate(t)),
            StmtT::CBranch(c, bt) => StmtT::CBranch(c.translate(t), bt.translate(t)),
            StmtT::Call(bt, args) => StmtT::Call(
                bt.translate(t),
                args.into_iter().map(|e| e.translate(t)).collect(),
            ),
            StmtT::Intrinsic(name, args) => {
                StmtT::Intrinsic(name, args.into_iter().map(|e| e.translate(t)).collect())
            }
            StmtT::Return(bt) => StmtT::Return(bt.translate(t)),
            StmtT::Skip => StmtT::Skip,
        }
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
pub enum StmtT<Loc, Val, Var> {
    Assign(Var, ExprT<Loc, Val, Var>),

    Store(
        ExprT<Loc, Val, Var>,
        ExprT<Loc, Val, Var>,
        usize,
        AddressSpaceId,
    ), // SPACE[T]:SIZE <- T

    Branch(BranchTargetT<Loc, Val, Var>),
    CBranch(ExprT<Loc, Val, Var>, BranchTargetT<Loc, Val, Var>),

    Call(
        BranchTargetT<Loc, Val, Var>,
        SmallVec<[ExprT<Loc, Val, Var>; 4]>,
    ),
    Return(BranchTargetT<Loc, Val, Var>),

    Skip, // NO-OP

    Intrinsic(Ustr, SmallVec<[ExprT<Loc, Val, Var>; 4]>), // no output intrinsic
}

impl<Loc, Val, Var> fmt::Display for StmtT<Loc, Val, Var>
where
    Loc: fmt::Display,
    Val: fmt::Display,
    Var: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assign(dest, src) => write!(f, "{} ← {}", dest, src),
            Self::Store(dest, src, size, spc) => {
                write!(f, "space[{}][{}]:{} ← {}", spc.index(), dest, size, src)
            }
            Self::Branch(target) => write!(f, "goto {}", target),
            Self::CBranch(cond, target) => write!(f, "goto {} if {}", target, cond),
            Self::Call(target, args) => {
                if !args.is_empty() {
                    write!(f, "call {}(", target)?;
                    write!(f, "{}", args[0])?;
                    for arg in &args[1..] {
                        write!(f, ", {}", arg)?;
                    }
                    write!(f, ")")
                } else {
                    write!(f, "call {}", target)
                }
            }
            Self::Return(target) => write!(f, "return {}", target),
            Self::Skip => write!(f, "skip"),
            Self::Intrinsic(name, args) => {
                write!(f, "{}(", name)?;
                if !args.is_empty() {
                    write!(f, "{}", args[0])?;
                    for arg in &args[1..] {
                        write!(f, ", {}", arg)?;
                    }
                }
                write!(f, ")")
            }
        }
    }
}

impl<'stmt, 'trans, Loc, Val, Var> fmt::Display for StmtTFormatter<'stmt, 'trans, Loc, Val, Var>
where
    Loc: for<'a> TranslatorDisplay<'stmt, 'a>,
    Var: for<'a> TranslatorDisplay<'stmt, 'a>,
    Val: for<'a> TranslatorDisplay<'stmt, 'a>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.stmt {
            StmtT::Assign(dest, src) => write!(
                f,
                "{} {}←{} {}",
                dest.display_full(Cow::Borrowed(&*self.fmt)),
                self.fmt.keyword_start,
                self.fmt.keyword_end,
                src.display_full(Cow::Borrowed(&*self.fmt))
            ),
            StmtT::Store(dest, src, size, spc) => {
                if let Some(trans) = self.fmt.translator {
                    let space = trans.manager().unchecked_space_by_id(*spc);
                    write!(
                        f,
                        "{}{}{}[{}]:{}{}{} {}←{} {}",
                        self.fmt.variable_start,
                        space.name(),
                        self.fmt.variable_end,
                        dest.display_full(Cow::Borrowed(&*self.fmt)),
                        self.fmt.value_start,
                        size,
                        self.fmt.value_end,
                        self.fmt.keyword_start,
                        self.fmt.keyword_end,
                        src.display_full(Cow::Borrowed(&*self.fmt))
                    )
                } else {
                    write!(
                        f,
                        "{}space{}[{}{}{}][{}]:{}{}{} {}←{} {}",
                        self.fmt.variable_start,
                        self.fmt.variable_end,
                        self.fmt.value_start,
                        spc.index(),
                        self.fmt.value_end,
                        dest.display_full(Cow::Borrowed(&*self.fmt)),
                        self.fmt.value_start,
                        size,
                        self.fmt.value_end,
                        self.fmt.keyword_start,
                        self.fmt.keyword_end,
                        src.display_full(Cow::Borrowed(&*self.fmt))
                    )
                }
            }
            StmtT::Branch(target) => {
                write!(
                    f,
                    "{}goto{} {}",
                    self.fmt.keyword_start,
                    self.fmt.keyword_end,
                    target.display_full(Cow::Borrowed(&*self.fmt)),
                )
            }
            StmtT::CBranch(cond, target) => write!(
                f,
                "{}goto{} {} {}if{} {}",
                self.fmt.keyword_start,
                self.fmt.keyword_end,
                target.display_full(Cow::Borrowed(&*self.fmt)),
                self.fmt.keyword_start,
                self.fmt.keyword_end,
                cond.display_full(Cow::Borrowed(&*self.fmt))
            ),
            StmtT::Call(target, args) => {
                if !args.is_empty() {
                    write!(
                        f,
                        "{}call{} {}(",
                        self.fmt.keyword_start,
                        self.fmt.keyword_end,
                        target.display_full(Cow::Borrowed(&*self.fmt))
                    )?;
                    write!(f, "{}", args[0].display_full(Cow::Borrowed(&*self.fmt)))?;
                    for arg in &args[1..] {
                        write!(f, ", {}", arg.display_full(Cow::Borrowed(&*self.fmt)))?;
                    }
                    write!(f, ")")
                } else {
                    write!(
                        f,
                        "{}call{} {}",
                        self.fmt.keyword_start,
                        self.fmt.keyword_end,
                        target.display_full(Cow::Borrowed(&*self.fmt))
                    )
                }
            }
            StmtT::Return(target) => {
                write!(
                    f,
                    "{}return{} {}",
                    self.fmt.keyword_start,
                    self.fmt.keyword_end,
                    target.display_full(Cow::Borrowed(&*self.fmt))
                )
            }
            StmtT::Skip => write!(f, "{}skip{}", self.fmt.keyword_start, self.fmt.keyword_end),
            StmtT::Intrinsic(name, args) => {
                write!(f, "{}(", name)?;
                if !args.is_empty() {
                    write!(f, "{}", args[0].display_full(Cow::Borrowed(&*self.fmt)))?;
                    for arg in &args[1..] {
                        write!(f, ", {}", arg.display_full(Cow::Borrowed(&*self.fmt)))?;
                    }
                }
                write!(f, ")")
            }
        }
    }
}

pub struct StmtTFormatter<'stmt, 'trans, Loc, Val, Var> {
    stmt: &'stmt StmtT<Loc, Val, Var>,
    fmt: Cow<'trans, TranslatorFormatter<'trans>>,
}

impl<'stmt, 'trans, Loc, Val, Var> TranslatorDisplay<'stmt, 'trans> for StmtT<Loc, Val, Var>
where
    Loc: for<'a> TranslatorDisplay<'stmt, 'a> + 'stmt,
    Var: for<'a> TranslatorDisplay<'stmt, 'a> + 'stmt,
    Val: for<'a> TranslatorDisplay<'stmt, 'a> + 'stmt,
{
    type Target = StmtTFormatter<'stmt, 'trans, Loc, Val, Var>;

    fn display_full(&'stmt self, fmt: Cow<'trans, TranslatorFormatter<'trans>>) -> Self::Target {
        StmtTFormatter { stmt: self, fmt }
    }
}

impl StmtT<Location, BitVec, Var> {
    pub fn from_parts<I: ExactSizeIterator<Item = VarnodeData>>(
        manager: &SpaceManager,
        float_formats: &FloatFormats,
        user_ops: &[UserOpStr],
        address: &AddressValue,
        position: usize,
        opcode: Opcode,
        inputs: I,
        output: Option<VarnodeData>,
    ) -> Self {
        let mut inputs = inputs.into_iter();
        let spaces = manager.spaces();
        match opcode {
            Opcode::Copy => Self::assign(
                output.unwrap(),
                ExprT::from_space(inputs.next().unwrap(), manager),
            ),
            Opcode::Load => {
                let space = &spaces[inputs.next().unwrap().offset() as usize];
                let destination = output.unwrap();
                let source = inputs.next().unwrap().into_space(manager);
                let size = destination.size() * 8;

                let src = if space.word_size() > 1 {
                    let s = ExprT::from(source);
                    let bits = s.bits();

                    let w = ExprT::from(BitVec::from_usize(space.word_size(), bits));

                    ExprT::int_mul(s, w)
                } else {
                    source
                };

                Self::assign(destination, ExprT::load(src, size, space))
            }
            Opcode::Store => {
                let space = &spaces[inputs.next().unwrap().offset() as usize];
                let destination = inputs.next().unwrap().into_space(manager);
                let source = inputs.next().unwrap();
                let size = source.size() * 8;

                let dest = if space.word_size() > 1 {
                    let d = ExprT::from(destination);
                    let bits = d.bits();

                    let w = ExprT::from(BitVec::from_usize(space.word_size(), bits));

                    ExprT::int_mul(d, w)
                } else {
                    destination
                };

                Self::store(dest, ExprT::from_space(source, manager), size, space)
            }
            Opcode::Branch => {
                let mut target = Location::from_space(inputs.next().unwrap(), manager);
                target.absolute_from(address.to_owned(), position);

                Self::branch(target)
            }
            Opcode::CBranch => {
                let mut target = Location::from_space(inputs.next().unwrap(), manager);
                target.absolute_from(address.to_owned(), position);

                let condition = ExprT::from_space(inputs.next().unwrap(), manager);

                Self::branch_conditional(condition, target)
            }
            Opcode::IBranch => {
                let target = ExprT::from_space(inputs.next().unwrap(), manager);
                let space = manager.unchecked_space_by_id(address.space());

                Self::branch_indirect(target, space)
            }
            Opcode::Call => {
                let mut target = Location::from_space(inputs.next().unwrap(), manager);
                target.absolute_from(address.to_owned(), position);

                Self::call(target)
            }
            Opcode::ICall => {
                let target = ExprT::from_space(inputs.next().unwrap(), manager);
                let space = manager.unchecked_space_by_id(address.space());

                Self::call_indirect(target, space)
            }
            Opcode::CallOther => {
                // TODO: eliminate this allocation
                let name = user_ops[inputs.next().unwrap().offset() as usize].clone();
                if let Some(output) = output {
                    let output = Var::from(output);
                    let bits = output.bits();
                    Self::assign(
                        output,
                        ExprT::intrinsic(
                            &*name,
                            inputs.into_iter().map(|v| ExprT::from_space(v, manager)),
                            bits,
                        ),
                    )
                } else {
                    Self::intrinsic(
                        &*name,
                        inputs.into_iter().map(|v| ExprT::from_space(v, manager)),
                    )
                }
            }
            Opcode::Return => {
                let target = ExprT::from_space(inputs.next().unwrap(), manager);
                let space = manager.unchecked_space_by_id(address.space());

                Self::return_(target, space)
            }
            Opcode::Subpiece => {
                let source = ExprT::from_space(inputs.next().unwrap(), manager);
                let src_size = source.bits();

                let output = output.unwrap();
                let out_size = output.size() * 8;

                let loff = inputs.next().unwrap().offset() as usize * 8;
                let trun_size = src_size.checked_sub(loff).unwrap_or(0);

                let trun = if out_size > trun_size {
                    // extract high + expand
                    let source_htrun = ExprT::extract_high(source, trun_size);
                    ExprT::cast_unsigned(source_htrun, out_size)
                } else {
                    // extract
                    let hoff = loff + out_size;
                    ExprT::extract(source, loff, hoff)
                };

                Self::assign(output, trun)
            }
            Opcode::PopCount => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = Var::from(output.unwrap());

                let size = output.bits();
                let popcount = ExprT::count_ones(input);

                Self::assign(output, ExprT::cast_unsigned(popcount, size))
            },
            Opcode::LZCount => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = Var::from(output.unwrap());

                let size = output.bits();
                let lzcount = ExprT::count_leading_zeros(input);

                Self::assign(output, ExprT::cast_unsigned(lzcount, size))
            }
            Opcode::BoolNot => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::bool_not(input))
            }
            Opcode::BoolAnd => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::bool_and(input1, input2))
            }
            Opcode::BoolOr => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::bool_or(input1, input2))
            }
            Opcode::BoolXor => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::bool_xor(input1, input2))
            }
            Opcode::IntNeg => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_neg(input))
            }
            Opcode::IntNot => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_not(input))
            }
            Opcode::IntSExt => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();
                let size = output.size() * 8;

                Self::assign(output, ExprT::cast_signed(input, size))
            }
            Opcode::IntZExt => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();
                let size = output.size() * 8;

                Self::assign(output, ExprT::cast_unsigned(input, size))
            }
            Opcode::IntEq => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_eq(input1, input2))
            }
            Opcode::IntNotEq => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_neq(input1, input2))
            }
            Opcode::IntLess => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_lt(input1, input2))
            }
            Opcode::IntLessEq => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_le(input1, input2))
            }
            Opcode::IntSLess => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_slt(input1, input2))
            }
            Opcode::IntSLessEq => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_sle(input1, input2))
            }
            Opcode::IntCarry => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_carry(input1, input2))
            }
            Opcode::IntSCarry => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_scarry(input1, input2))
            }
            Opcode::IntSBorrow => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_sborrow(input1, input2))
            }
            Opcode::IntAdd => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_add(input1, input2))
            }
            Opcode::IntSub => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_sub(input1, input2))
            }
            Opcode::IntDiv => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_div(input1, input2))
            }
            Opcode::IntSDiv => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_sdiv(input1, input2))
            }
            Opcode::IntMul => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_mul(input1, input2))
            }
            Opcode::IntRem => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_rem(input1, input2))
            }
            Opcode::IntSRem => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_srem(input1, input2))
            }
            Opcode::IntLShift => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_shl(input1, input2))
            }
            Opcode::IntRShift => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_shr(input1, input2))
            }
            Opcode::IntSRShift => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_sar(input1, input2))
            }
            Opcode::IntAnd => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_and(input1, input2))
            }
            Opcode::IntOr => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_or(input1, input2))
            }
            Opcode::IntXor => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::int_xor(input1, input2))
            }
            Opcode::FloatIsNaN => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_nan(input, float_formats))
            }
            Opcode::FloatAbs => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_abs(input, float_formats))
            }
            Opcode::FloatNeg => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_neg(input, float_formats))
            }
            Opcode::FloatSqrt => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_sqrt(input, float_formats))
            }
            Opcode::FloatFloor => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_floor(input, float_formats))
            }
            Opcode::FloatCeiling => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_ceiling(input, float_formats))
            }
            Opcode::FloatRound => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_round(input, float_formats))
            }
            Opcode::FloatEq => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_eq(input1, input2, float_formats))
            }
            Opcode::FloatNotEq => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_neq(input1, input2, float_formats))
            }
            Opcode::FloatLess => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_lt(input1, input2, float_formats))
            }
            Opcode::FloatLessEq => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_le(input1, input2, float_formats))
            }
            Opcode::FloatAdd => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_add(input1, input2, float_formats))
            }
            Opcode::FloatSub => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_sub(input1, input2, float_formats))
            }
            Opcode::FloatDiv => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_div(input1, input2, float_formats))
            }
            Opcode::FloatMul => {
                let input1 = ExprT::from_space(inputs.next().unwrap(), manager);
                let input2 = ExprT::from_space(inputs.next().unwrap(), manager);
                let output = output.unwrap();

                Self::assign(output, ExprT::float_mul(input1, input2, float_formats))
            }
            Opcode::FloatOfFloat => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let input_size = input.bits();

                let output = Var::from(output.unwrap());
                let output_size = output.bits();

                let input_format = float_formats[&input_size].clone();
                let output_format = float_formats[&output_size].clone();

                Self::assign(
                    output,
                    ExprT::cast_float(ExprT::cast_float(input, input_format), output_format),
                )
            }
            Opcode::FloatOfInt => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let input_size = input.bits();

                let output = Var::from(output.unwrap());
                let output_size = output.bits();

                let format = float_formats[&output_size].clone();
                Self::assign(
                    output,
                    ExprT::cast_float(ExprT::cast_signed(input, input_size), format),
                )
            }
            Opcode::FloatTruncate => {
                let input = ExprT::from_space(inputs.next().unwrap(), manager);
                let input_size = input.bits();

                let output = Var::from(output.unwrap());
                let output_size = output.bits();

                let format = float_formats[&input_size].clone();
                Self::assign(
                    output,
                    ExprT::cast_signed(ExprT::cast_float(input, format), output_size),
                )
            }
            Opcode::Label => Self::skip(),
            Opcode::Build
            | Opcode::CrossBuild
            | Opcode::CPoolRef
            | Opcode::Piece
            | Opcode::Extract
            | Opcode::DelaySlot
            | Opcode::New
            | Opcode::Insert
            | Opcode::Cast
            | Opcode::SegmentOp => {
                panic!("unimplemented due to spec.")
            }
        }
    }
}

impl<Loc, Val, Var> StmtT<Loc, Val, Var>
where
    Val: BitSize,
    Var: BitSize,
{
    pub fn assign<D, S>(destination: D, source: S) -> Self
    where
        D: Into<Var>,
        S: Into<ExprT<Loc, Val, Var>>,
    {
        let dest = destination.into();
        let bits = dest.bits();
        Self::Assign(dest, ExprT::cast_unsigned(source, bits))
    }

    pub fn store<D, S>(destination: D, source: S, size: usize, space: &AddressSpace) -> Self
    where
        D: Into<ExprT<Loc, Val, Var>>,
        S: Into<ExprT<Loc, Val, Var>>,
    {
        Self::Store(
            ExprT::cast_unsigned(destination.into(), space.address_size() * 8),
            source.into(),
            size,
            space.id(),
        )
    }

    pub fn branch<T>(target: T) -> Self
    where
        T: Into<BranchTargetT<Loc, Val, Var>>,
    {
        Self::Branch(target.into())
    }

    pub fn branch_conditional<C, T>(condition: C, target: T) -> Self
    where
        C: Into<ExprT<Loc, Val, Var>>,
        T: Into<BranchTargetT<Loc, Val, Var>>,
    {
        Self::CBranch(ExprT::cast_bool(condition), target.into())
    }

    pub fn branch_indirect<T>(target: T, space: &AddressSpace) -> Self
    where
        T: Into<ExprT<Loc, Val, Var>>,
    {
        let vptr = Cast::Pointer(Box::new(Cast::Void), space.address_size() * 8);

        Self::Branch(BranchTargetT::computed(ExprT::Cast(
            Box::new(target.into()),
            vptr,
        )))
        /*
        Self::Branch(BranchTargetT::computed(ExprT::load(
            target,
            space.address_size() * 8,
            space,
        )))
        */
    }

    pub fn call<T>(target: T) -> Self
    where
        T: Into<BranchTargetT<Loc, Val, Var>>,
    {
        Self::Call(target.into(), Default::default())
    }

    pub fn call_indirect<T>(target: T, space: &AddressSpace) -> Self
    where
        T: Into<ExprT<Loc, Val, Var>>,
    {
        let fptr = Cast::Pointer(
            Box::new(Cast::Function(Box::new(Cast::Void), SmallVec::new())),
            space.address_size() * 8,
        );

        Self::Call(
            BranchTargetT::computed(ExprT::Cast(Box::new(target.into()), fptr)),
            Default::default(),
        )
    }

    pub fn call_with<T, I, E>(target: T, arguments: I) -> Self
    where
        T: Into<BranchTargetT<Loc, Val, Var>>,
        I: ExactSizeIterator<Item = E>,
        E: Into<ExprT<Loc, Val, Var>>,
    {
        let mut args = SmallVec::with_capacity(arguments.len());
        for arg in arguments.map(|e| e.into()) {
            args.push(arg);
        }

        Self::Call(target.into(), args)
    }

    pub fn call_indirect_with<T, I, E>(target: T, space: &AddressSpace, arguments: I) -> Self
    where
        T: Into<ExprT<Loc, Val, Var>>,
        I: ExactSizeIterator<Item = E>,
        E: Into<ExprT<Loc, Val, Var>>,
    {
        let mut args = SmallVec::with_capacity(arguments.len());
        for arg in arguments.map(|e| e.into()) {
            args.push(arg);
        }

        let fptr = Cast::Pointer(
            Box::new(Cast::Function(Box::new(Cast::Void), SmallVec::new())),
            space.address_size() * 8,
        );

        Self::Call(
            //BranchTargetT::computed(ExprT::load(target, space.address_size() * 8, space)),
            BranchTargetT::computed(ExprT::Cast(Box::new(target.into()), fptr)),
            args,
        )
    }

    pub fn return_<T>(target: T, space: &AddressSpace) -> Self
    where
        T: Into<ExprT<Loc, Val, Var>>,
    {
        let vptr = Cast::Pointer(Box::new(Cast::Void), space.address_size() * 8);

        Self::Return(BranchTargetT::computed(ExprT::Cast(
            Box::new(target.into()),
            vptr,
        )))
        /*
            target,
            space.address_size() * 8,
            space,
        )))
        */
    }

    pub fn skip() -> Self {
        Self::Skip
    }

    pub fn intrinsic<N, I, E>(name: N, arguments: I) -> Self
    where
        N: Into<Ustr>,
        I: ExactSizeIterator<Item = E>,
        E: Into<ExprT<Loc, Val, Var>>,
    {
        let mut args = SmallVec::with_capacity(arguments.len());
        for arg in arguments.map(|e| e.into()) {
            args.push(arg);
        }

        Self::Intrinsic(name.into(), args)
    }
}

impl<'z> FromSpace<'z, Operand> for Var {
    fn from_space_with(t: Operand, _arena: &'z IRBuilderArena, manager: &SpaceManager) -> Self {
        Var::from_space(t, manager)
    }

    fn from_space(operand: Operand, manager: &SpaceManager) -> Self {
        match operand {
            Operand::Address { value, size } => Var {
                offset: value.offset(),
                space: manager.default_space_id(),
                bits: size * 8,
                generation: 0,
            },
            Operand::Register { offset, size, .. } => Var {
                offset,
                space: manager.register_space_id(),
                bits: size * 8,
                generation: 0,
            },
            Operand::Variable {
                offset,
                space,
                size,
            } => Var {
                offset,
                space,
                bits: size * 8,
                generation: 0,
            },
            _ => panic!("cannot create Var from Operand::Constant"),
        }
    }
}

impl<'z, Loc> FromSpace<'z, Operand> for ExprT<Loc, BitVec, Var> {
    fn from_space_with(
        operand: Operand,
        _arena: &'z IRBuilderArena,
        manager: &SpaceManager,
    ) -> Self {
        ExprT::from_space(operand, manager)
    }

    fn from_space(operand: Operand, manager: &SpaceManager) -> Self {
        if let Operand::Constant { value, size, .. } = operand {
            ExprT::Val(BitVec::from_u64(value, size * 8))
        } else {
            Var::from_space(operand, manager).into()
        }
    }
}

impl<'z> FromSpace<'z, Operand> for Location {
    fn from_space_with(t: Operand, _arena: &'z IRBuilderArena, manager: &SpaceManager) -> Self {
        Location::from_space(t, manager)
    }

    fn from_space(operand: Operand, manager: &SpaceManager) -> Self {
        match operand {
            Operand::Address { value, .. } => Location {
                address: value.into_space(manager),
                position: 0,
            },
            Operand::Constant { value, .. } => Location {
                address: AddressValue::new(manager.constant_space_ref(), value),
                position: 0,
            },
            Operand::Register { offset, .. } => Location {
                address: AddressValue::new(manager.register_space_ref(), offset),
                position: 0,
            },
            Operand::Variable { offset, space, .. } => Location {
                address: AddressValue::new(manager.unchecked_space_by_id(space), offset),
                position: 0,
            },
        }
    }
}

impl StmtT<Location, BitVec, Var> {
    pub fn from_pcode(
        translator: &Translator,
        pcode: PCodeOp,
        address: &AddressValue,
        position: usize,
    ) -> Self {
        let manager = translator.manager();
        let formats = translator.float_formats();

        match pcode {
            PCodeOp::Copy {
                destination,
                source,
            } => Self::assign(
                Var::from_space(destination, manager),
                ExprT::from_space(source, manager),
            ),
            PCodeOp::Load {
                destination,
                source,
                space,
            } => {
                let space = manager.unchecked_space_by_id(space);
                let size = destination.size() * 8;
                let src = if space.word_size() > 1 {
                    let s = ExprT::from_space(source, manager);
                    let bits = s.bits();

                    let w = ExprT::from(BitVec::from_usize(space.word_size(), bits));

                    ExprT::int_mul(s, w)
                } else {
                    ExprT::from_space(source, manager)
                };

                Self::assign(
                    Var::from_space(destination, manager),
                    ExprT::load(src, size, space),
                )
            }
            PCodeOp::Store {
                destination,
                source,
                space,
            } => {
                let space = manager.unchecked_space_by_id(space);
                let size = source.size() * 8;

                let dest = if space.word_size() > 1 {
                    let d = ExprT::from_space(destination, manager);
                    let bits = d.bits();

                    let w = ExprT::from(BitVec::from_usize(space.word_size(), bits));

                    ExprT::int_mul(d, w)
                } else {
                    ExprT::from_space(destination, manager)
                };

                Self::store(dest, ExprT::from_space(source, manager), size, space)
            }
            PCodeOp::Branch { destination } => {
                let mut target = Location::from_space(destination, manager);
                target.absolute_from(address.to_owned(), position);

                Self::branch(target)
            }
            PCodeOp::CBranch {
                condition,
                destination,
            } => {
                let mut target = Location::from_space(destination, manager);
                target.absolute_from(address.to_owned(), position);

                Self::branch_conditional(ExprT::from_space(condition, manager), target)
            }
            PCodeOp::IBranch { destination } => {
                let space = manager.unchecked_space_by_id(address.space());

                Self::branch_indirect(ExprT::from_space(destination, manager), space)
            }
            PCodeOp::Call { destination } => {
                let mut target = Location::from_space(destination, manager);
                target.absolute_from(address.to_owned(), position);

                Self::call(target)
            }
            PCodeOp::ICall { destination } => {
                let space = manager.unchecked_space_by_id(address.space());

                Self::call_indirect(ExprT::from_space(destination, manager), space)
            }
            PCodeOp::Intrinsic {
                name,
                operands,
                result,
            } => {
                if let Some(result) = result {
                    let output = Var::from_space(result, manager);
                    let bits = output.bits();
                    Self::assign(
                        output,
                        ExprT::intrinsic(
                            &*name,
                            operands.into_iter().map(|v| ExprT::from_space(v, manager)),
                            bits,
                        ),
                    )
                } else {
                    Self::intrinsic(
                        &*name,
                        operands.into_iter().map(|v| ExprT::from_space(v, manager)),
                    )
                }
            }
            PCodeOp::Return { destination } => {
                let space = manager.unchecked_space_by_id(address.space());

                Self::return_(ExprT::from_space(destination, manager), space)
            }
            PCodeOp::Subpiece {
                operand,
                amount,
                result,
            } => {
                let source = ExprT::from_space(operand, manager);
                let src_size = source.bits();
                let out_size = result.size() * 8;

                let loff = amount.offset() as usize * 8;
                let trun_size = src_size.checked_sub(loff).unwrap_or(0);

                let trun = if out_size > trun_size {
                    // extract high + expand
                    let source_htrun = ExprT::extract_high(source, trun_size);
                    ExprT::cast_unsigned(source_htrun, out_size)
                } else {
                    // extract
                    let hoff = loff + out_size;
                    ExprT::extract(source, loff, hoff)
                };

                Self::assign(Var::from_space(result, manager), trun)
            }
            PCodeOp::PopCount { result, operand } => {
                let output = Var::from_space(result, manager);

                let size = output.bits();
                let popcount = ExprT::unary_op(UnOp::POPCOUNT, ExprT::from_space(operand, manager));

                Self::assign(output, ExprT::cast_unsigned(popcount, size))
            }
            PCodeOp::LZCount { result, operand } => {
                let output = Var::from_space(result, manager);

                let size = output.bits();
                let lzcount = ExprT::unary_op(UnOp::LZCOUNT, ExprT::from_space(operand, manager));

                Self::assign(output, ExprT::cast_unsigned(lzcount, size))
            }
            PCodeOp::BoolNot { result, operand } => Self::assign(
                Var::from_space(result, manager),
                ExprT::bool_not(ExprT::from_space(operand, manager)),
            ),
            PCodeOp::BoolAnd {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::bool_and(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::BoolOr {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::bool_or(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::BoolXor {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::bool_xor(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntNeg { result, operand } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_neg(ExprT::from_space(operand, manager)),
            ),
            PCodeOp::IntNot { result, operand } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_not(ExprT::from_space(operand, manager)),
            ),
            PCodeOp::IntSExt { result, operand } => {
                let size = result.size() * 8;
                Self::assign(
                    Var::from_space(result, manager),
                    ExprT::cast_signed(ExprT::from_space(operand, manager), size),
                )
            }
            PCodeOp::IntZExt { result, operand } => {
                let size = result.size() * 8;
                Self::assign(
                    Var::from_space(result, manager),
                    ExprT::cast_unsigned(ExprT::from_space(operand, manager), size),
                )
            }
            PCodeOp::IntEq {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_eq(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntNotEq {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_neq(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntLess {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_lt(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntLessEq {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_le(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntSLess {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_slt(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntSLessEq {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_sle(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntCarry {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_carry(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntSCarry {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_scarry(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntSBorrow {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_sborrow(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntAdd {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_add(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntSub {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_sub(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntDiv {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_div(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntSDiv {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_sdiv(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntMul {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_mul(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntRem {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_rem(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntSRem {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_srem(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntLeftShift {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_shl(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntRightShift {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_shr(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntSRightShift {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_sar(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntAnd {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_and(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntOr {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_or(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::IntXor {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::int_xor(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                ),
            ),
            PCodeOp::FloatIsNaN { result, operand } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_nan(ExprT::from_space(operand, manager), &formats),
            ),
            PCodeOp::FloatAbs { result, operand } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_abs(ExprT::from_space(operand, manager), &formats),
            ),
            PCodeOp::FloatNeg { result, operand } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_neg(ExprT::from_space(operand, manager), &formats),
            ),
            PCodeOp::FloatSqrt { result, operand } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_sqrt(ExprT::from_space(operand, manager), &formats),
            ),
            PCodeOp::FloatFloor { result, operand } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_floor(ExprT::from_space(operand, manager), &formats),
            ),
            PCodeOp::FloatCeiling { result, operand } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_ceiling(ExprT::from_space(operand, manager), &formats),
            ),
            PCodeOp::FloatRound { result, operand } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_round(ExprT::from_space(operand, manager), &formats),
            ),
            PCodeOp::FloatEq {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_eq(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                    &formats,
                ),
            ),
            PCodeOp::FloatNotEq {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_neq(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                    &formats,
                ),
            ),
            PCodeOp::FloatLess {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_lt(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                    &formats,
                ),
            ),
            PCodeOp::FloatLessEq {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_le(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                    &formats,
                ),
            ),
            PCodeOp::FloatAdd {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_add(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                    &formats,
                ),
            ),
            PCodeOp::FloatSub {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_sub(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                    &formats,
                ),
            ),
            PCodeOp::FloatDiv {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_div(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                    &formats,
                ),
            ),
            PCodeOp::FloatMul {
                result,
                operands: [operand1, operand2],
            } => Self::assign(
                Var::from_space(result, manager),
                ExprT::float_mul(
                    ExprT::from_space(operand1, manager),
                    ExprT::from_space(operand2, manager),
                    &formats,
                ),
            ),
            PCodeOp::FloatOfFloat { result, operand } => {
                let input = ExprT::from_space(operand, manager);
                let input_size = input.bits();

                let output = Var::from_space(result, manager);
                let output_size = output.bits();

                let input_format = formats[&input_size].clone();
                let output_format = formats[&output_size].clone();

                Self::assign(
                    output,
                    ExprT::cast_float(ExprT::cast_float(input, input_format), output_format),
                )
            }
            PCodeOp::FloatOfInt { result, operand } => {
                let input = ExprT::from_space(operand, manager);
                let input_size = input.bits();

                let output = Var::from_space(result, manager);
                let output_size = output.bits();

                let format = formats[&output_size].clone();
                Self::assign(
                    output,
                    ExprT::cast_float(ExprT::cast_signed(input, input_size), format),
                )
            }
            PCodeOp::FloatTruncate { result, operand } => {
                let input = ExprT::from_space(operand, manager);
                let input_size = input.bits();

                let output = Var::from_space(result, manager);
                let output_size = output.bits();

                let format = formats[&input_size].clone();
                Self::assign(
                    output,
                    ExprT::cast_signed(ExprT::cast_float(input, format), output_size),
                )
            }
            PCodeOp::Skip => Self::skip(),
        }
    }
}

pub type BranchTarget = BranchTargetT<Location, BitVec, Var>;
pub type Expr = ExprT<Location, BitVec, Var>;
pub type Stmt = StmtT<Location, BitVec, Var>;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize)]
pub struct ECode {
    pub address: AddressValue,
    pub operations: SmallVec<[Stmt; 8]>,
    pub delay_slots: usize,
    pub length: usize,
}

impl ECode {
    pub fn nop(address: AddressValue, length: usize) -> Self {
        Self {
            address,
            operations: smallvec![StmtT::skip()],
            delay_slots: 0,
            length,
        }
    }

    pub fn address(&self) -> AddressValue {
        self.address.clone()
    }

    pub fn operations(&self) -> &[Stmt] {
        self.operations.as_ref()
    }

    pub fn operations_mut(&mut self) -> &mut SmallVec<[Stmt; 8]> {
        &mut self.operations
    }

    pub fn delay_slots(&self) -> usize {
        self.delay_slots
    }

    pub fn length(&self) -> usize {
        self.length
    }
}

impl ECode {
    pub fn from_pcode(translator: &Translator, pcode: PCode) -> Self {
        let address = pcode.address;
        let mut operations = SmallVec::with_capacity(pcode.operations.len());

        for (i, op) in pcode.operations.into_iter().enumerate() {
            operations.push(StmtT::from_pcode(translator, op, &address, i));
        }

        Self {
            operations,
            address,
            delay_slots: pcode.delay_slots,
            length: pcode.length,
        }
    }
}

impl fmt::Display for ECode {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let len = self.operations.len();
        if len > 0 {
            for (i, op) in self.operations.iter().enumerate() {
                write!(
                    f,
                    "{}.{:02}: {}{}",
                    self.address,
                    i,
                    op,
                    if i == len - 1 { "" } else { "\n" }
                )?;
            }
            Ok(())
        } else {
            write!(f, "{}.00: skip", self.address)
        }
    }
}

pub struct ECodeFormatter<'ecode, 'trans> {
    ecode: &'ecode ECode,
    fmt: Cow<'trans, TranslatorFormatter<'trans>>,
}

impl<'v, 't> TranslatorDisplay<'v, 't> for ECode {
    type Target = ECodeFormatter<'v, 't>;

    fn display_full(
        &'v self,
        fmt: Cow<'t, TranslatorFormatter<'t>>,
    ) -> Self::Target {
        ECodeFormatter {
            ecode: self,
            fmt,
        }
    }
}

impl<'ecode, 'trans> fmt::Display for ECodeFormatter<'ecode, 'trans> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let len = self.ecode.operations.len();
        if len > 0 {
            for (i, op) in self.ecode.operations.iter().enumerate() {
                write!(
                    f,
                    "{}{}{}.{}{:02}{}: {}{}",
                    self.fmt.location_start,
                    self.ecode.address,
                    self.fmt.location_end,
                    self.fmt.location_start,
                    i,
                    self.fmt.location_end,
                    op.display_full(Cow::Borrowed(&*self.fmt)),
                    if i == len - 1 { "" } else { "\n" }
                )?;
            }
            Ok(())
        } else {
            write!(
                f,
                "{}{}{}.{}00{}: skip",
                self.fmt.location_start,
                self.fmt.location_end,
                self.ecode.address,
                self.fmt.location_start,
                self.fmt.location_end
            )
        }
    }
}
