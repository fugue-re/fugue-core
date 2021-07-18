use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use crate::address::AddressValue;
use crate::disassembly::{Opcode, VarnodeData};
use crate::float_format::FloatFormat;
use crate::space::AddressSpace;
use crate::space_manager::SpaceManager;
use crate::Translator;

use fnv::FnvHashMap as Map;
use fugue_bv::BitVec;
use smallvec::{smallvec, SmallVec};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Var {
    space: Arc<AddressSpace>,
    offset: u64,
    bits: usize,
    generation: usize,
}

impl Var {
    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn bits(&self) -> usize {
        self.bits
    }

    pub fn generation(&self) -> usize {
        self.generation
    }

    pub fn with_generation(&self, generation: usize) -> Self {
        Self {
            space: self.space.clone(),
            generation,
            ..*self
        }
    }

    pub fn space(&self) -> Arc<AddressSpace> {
        self.space.clone()
    }
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display(None))
    }
}

impl<'var, 'trans> fmt::Display for VarFormatter<'var, 'trans> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.translator.is_some() && self.var.space().is_register() {
            write!(f, "{}:{}", self.translator.unwrap().registers()[&(self.var.offset(), self.var.bits() / 8)], self.var.bits())
        } else {
            write!(f, "{}[{:#x}]:{}", self.var.space().name(), self.var.offset(), self.var.bits())
        }
    }
}

pub struct VarFormatter<'var, 'trans> {
    var: &'var Var,
    translator: Option<&'trans Translator>,
}

impl Var {
    fn display<'var, 'trans>(&'var self, translator: Option<&'trans Translator>) -> VarFormatter<'var, 'trans> {
        VarFormatter {
            var: self,
            translator,
        }
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Location {
    address: AddressValue,
    position: usize,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.address, self.position)
    }
}

impl Location {
    pub fn new<A>(address: A, position: usize) -> Location
    where A: Into<AddressValue> {
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

    pub fn space(&self) -> Arc<AddressSpace> {
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

impl From<VarnodeData> for Location {
    fn from(vnd: VarnodeData) -> Self {
        Self {
            address: AddressValue::new(vnd.space(), vnd.offset()),
            position: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BranchTarget {
    Location(Location),
    Computed(Expr),
}

impl fmt::Display for BranchTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display(None))
    }
}

pub struct BranchTargetFormatter<'target, 'trans> {
    target: &'target BranchTarget,
    translator: Option<&'trans Translator>,
}

impl<'target, 'trans> fmt::Display for BranchTargetFormatter<'target, 'trans> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.target {
            BranchTarget::Location(loc) => write!(f, "{}", loc),
            BranchTarget::Computed(expr) => write!(f, "{}", expr.display(self.translator.clone())),
        }
    }
}

impl BranchTarget {
    fn display<'target, 'trans>(&'target self, translator: Option<&'trans Translator>) -> BranchTargetFormatter<'target, 'trans> {
        BranchTargetFormatter {
            target: self,
            translator,
        }
    }
}

impl From<Location> for BranchTarget {
    fn from(t: Location) -> Self {
        Self::Location(t)
    }
}

impl BranchTarget {
    pub fn computed<E: Into<Expr>>(expr: E) -> Self {
        Self::Computed(expr.into())
    }

    pub fn is_computed(&self) -> bool {
        matches!(self, Self::Computed(_))
    }

    pub fn location<L: Into<Location>>(location: L) -> Self {
        Self::from(location.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cast {
    Bool,                       // T -> Bool
    Float(Arc<FloatFormat>),    // T -> FloatFormat::T

    Signed(usize),   // sign-extension
    Unsigned(usize), // zero-extension

    High(usize), // truncate keep MSBs
    Low(usize),  // truncate keep LSBs
}

impl fmt::Display for Cast {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool => write!(f, "bool"),
            Self::Float(format) => write!(f, "float{}", format.bits()),
            Self::Signed(bits) => write!(f, "i{}", bits),
            Self::Unsigned(bits) => write!(f, "u{}", bits),
            Self::High(bits) => write!(f, "u{}", bits),
            Self::Low(bits) => write!(f, "u{}", bits),
        }
    }
}

impl Cast {
    pub fn bits(&self) -> usize {
        match self {
            Self::Bool => 1,
            Self::Float(format) => format.bits(),
            Self::Signed(bits) | Self::Unsigned(bits) | Self::Low(bits) | Self::High(bits) => *bits,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnOp {
    NOT,
    NEG,

    ABS,
    SQRT,
    CEILING,
    FLOOR,
    ROUND,

    POPCOUNT,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnRel {
    NAN
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Expr {
    UnRel(UnRel, Box<Expr>),                      // T -> bool
    BinRel(BinRel, Box<Expr>, Box<Expr>), // T * T -> bool

    UnOp(UnOp, Box<Expr>),                      // T -> T
    BinOp(BinOp, Box<Expr>, Box<Expr>), // T * T -> T

    Cast(Box<Expr>, Cast), // T -> Cast::T
    Load(Box<Expr>, usize, Arc<AddressSpace>), // SPACE[T]:SIZE -> T

    Extract(Box<Expr>, usize, usize), // T T[LSB..MSB) -> T
    Concat(Box<Expr>, Box<Expr>), // T * T -> T

    Intrinsic(Arc<str>, SmallVec<[Box<Expr>; 4]>, usize),

    Val(BitVec),      // BitVec -> T
    Var(Var), // String * usize -> T
}

impl Expr {
    fn fmt_l1<'trans>(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'trans Translator>) -> fmt::Result {
        match self {
            Expr::Val(v) => write!(f, "{:#x}", v),
            Expr::Var(v) => write!(f, "{}", v.display(translator.clone())),

            Expr::Intrinsic(name, args, _) => {
                write!(f, "{}(", name)?;
                if !args.is_empty() {
                    write!(f, "{}", args[0])?;
                    for arg in &args[1..] {
                        write!(f, ", {}", arg)?;
                    }
                }
                write!(f, ")")
            }

            Expr::Cast(expr, Cast::High(bits)) => write!(f, "extract-msb({}, bits={})", expr.display(translator.clone()), bits),
            Expr::Cast(expr, Cast::Low(bits)) => write!(f, "extract-lsb({}, bits={})", expr.display(translator.clone()), bits),

            Expr::Cast(expr, Cast::Bool) => { expr.fmt_l1(f, translator)?; write!(f, " as bool") },
            Expr::Cast(expr, Cast::Signed(bits)) => { expr.fmt_l1(f, translator)?;  write!(f, " as i{}", bits) },
            Expr::Cast(expr, Cast::Unsigned(bits)) => { expr.fmt_l1(f, translator)?; write!(f, " as u{}", bits) },
            Expr::Cast(expr, Cast::Float(format)) => { expr.fmt_l1(f, translator)?; write!(f, " as f{}", format.bits()) },

            Expr::Load(expr, bits, space) => write!(f, "{}[{}]:{}", space.name(), expr.display(translator.clone()), bits),

            Expr::Extract(expr, lsb, msb) => write!(f, "extract({}, from={}, to={})", expr.display(translator.clone()), lsb, msb),
            Expr::Concat(e1, e2) => write!(f, "concat({}, {})", e1.display(translator.clone()), e2.display(translator.clone())),

            Expr::UnOp(UnOp::ABS, expr) => write!(f, "abs({})", expr.display(translator.clone())),
            Expr::UnOp(UnOp::SQRT, expr) => write!(f, "sqrt({})", expr.display(translator.clone())),
            Expr::UnOp(UnOp::ROUND, expr) => write!(f, "round({})", expr.display(translator.clone())),
            Expr::UnOp(UnOp::CEILING, expr) => write!(f, "ceiling({})", expr.display(translator.clone())),
            Expr::UnOp(UnOp::FLOOR, expr) => write!(f, "floor({})", expr.display(translator.clone())),
            Expr::UnOp(UnOp::POPCOUNT, expr) => write!(f, "popcount({})", expr.display(translator.clone())),

            Expr::UnRel(UnRel::NAN, expr) => write!(f, "is-nan({})", expr.display(translator.clone())),

            Expr::BinRel(BinRel::CARRY, e1, e2) => write!(f, "carry({}, {})", e1.display(translator.clone()), e2.display(translator.clone())),
            Expr::BinRel(BinRel::SCARRY, e1, e2) => write!(f, "scarry({}, {})", e1.display(translator.clone()), e2.display(translator.clone())),
            Expr::BinRel(BinRel::SBORROW, e1, e2) => write!(f, "sborrow({}, {})", e1.display(translator.clone()), e2.display(translator.clone())),

            expr => write!(f, "({})", expr.display(translator)),
        }
    }

    fn fmt_l2<'trans>(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'trans Translator>) -> fmt::Result {
        match self {
            Expr::UnOp(UnOp::NEG, expr) => { write!(f, "-")?; expr.fmt_l1(f, translator) },
            Expr::UnOp(UnOp::NOT, expr) => { write!(f, "!")?; expr.fmt_l1(f, translator) },
            expr => expr.fmt_l1(f, translator)
        }
    }

    fn fmt_l3<'trans>(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'trans Translator>) -> fmt::Result {
        match self {
            Expr::BinOp(BinOp::MUL, e1, e2) => { e1.fmt_l3(f, translator.clone())?; write!(f, " * ")?; e2.fmt_l2(f, translator) }
            Expr::BinOp(BinOp::DIV, e1, e2) => { e1.fmt_l3(f, translator.clone())?; write!(f, " / ")?; e2.fmt_l2(f, translator) }
            Expr::BinOp(BinOp::SDIV, e1, e2) => { e1.fmt_l3(f, translator.clone())?; write!(f, " s/ ")?; e2.fmt_l2(f, translator) }
            Expr::BinOp(BinOp::REM, e1, e2) => { e1.fmt_l3(f, translator.clone())?; write!(f, " % ")?; e2.fmt_l2(f, translator) }
            Expr::BinOp(BinOp::SREM, e1, e2) => { e1.fmt_l3(f, translator.clone())?; write!(f, " s% ")?; e2.fmt_l2(f, translator) }
            expr => expr.fmt_l2(f, translator)
        }
    }

    fn fmt_l4<'trans>(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'trans Translator>) -> fmt::Result {
        match self {
            Expr::BinOp(BinOp::ADD, e1, e2) => { e1.fmt_l4(f, translator.clone())?; write!(f, " + ")?; e2.fmt_l3(f, translator) },
            Expr::BinOp(BinOp::SUB, e1, e2) => { e1.fmt_l4(f, translator.clone())?; write!(f, " - ")?; e2.fmt_l3(f, translator) },
            expr => expr.fmt_l3(f, translator)
        }
    }

    fn fmt_l5<'trans>(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'trans Translator>) -> fmt::Result {
        match self {
            Expr::BinOp(BinOp::SHL, e1, e2) => { e1.fmt_l5(f, translator.clone())?; write!(f, " << ")?; e2.fmt_l4(f, translator) },
            Expr::BinOp(BinOp::SHR, e1, e2) => { e1.fmt_l5(f, translator.clone())?; write!(f, " >> ")?; e2.fmt_l4(f, translator) },
            Expr::BinOp(BinOp::SAR, e1, e2) => { e1.fmt_l5(f, translator.clone())?; write!(f, " s>> ")?; e2.fmt_l4(f, translator) },
            expr => expr.fmt_l4(f, translator)
        }
    }

    fn fmt_l6<'trans>(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'trans Translator>) -> fmt::Result {
        match self {
            Expr::BinRel(BinRel::LT, e1, e2) => { e1.fmt_l6(f, translator.clone())?; write!(f, " < ")?; e2.fmt_l5(f, translator) },
            Expr::BinRel(BinRel::LE, e1, e2) => { e1.fmt_l6(f, translator.clone())?; write!(f, " <= ")?; e2.fmt_l5(f, translator) },
            Expr::BinRel(BinRel::SLT, e1, e2) => { e1.fmt_l6(f, translator.clone())?; write!(f, " s< ")?; e2.fmt_l5(f, translator) },
            Expr::BinRel(BinRel::SLE, e1, e2) => { e1.fmt_l6(f, translator.clone())?; write!(f, " s<= ")?; e2.fmt_l5(f, translator) },
            expr => expr.fmt_l5(f, translator)
        }
    }

    fn fmt_l7<'trans>(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'trans Translator>) -> fmt::Result {
        match self {
            Expr::BinRel(BinRel::EQ, e1, e2) => { e1.fmt_l7(f, translator.clone())?; write!(f, " == ")?; e2.fmt_l6(f, translator) },
            Expr::BinRel(BinRel::NEQ, e1, e2) => { e1.fmt_l7(f, translator.clone())?; write!(f, " != ")?; e2.fmt_l6(f, translator) },
            expr => expr.fmt_l6(f, translator)
        }
    }

    fn fmt_l8<'trans>(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'trans Translator>) -> fmt::Result {
        if let Expr::BinOp(BinOp::AND, e1, e2) = self {
            e1.fmt_l8(f, translator.clone())?;
            write!(f, " & ")?;
            e2.fmt_l7(f, translator)
        } else {
            self.fmt_l7(f, translator)
        }
    }

    fn fmt_l9<'trans>(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'trans Translator>) -> fmt::Result {
        if let Expr::BinOp(BinOp::XOR, e1, e2) = self {
            e1.fmt_l9(f, translator.clone())?;
            write!(f, " ^ ")?;
            e2.fmt_l8(f, translator)
        } else {
            self.fmt_l8(f, translator)
        }
    }

    fn fmt_l10<'trans>(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'trans Translator>) -> fmt::Result {
        if let Expr::BinOp(BinOp::OR, e1, e2) = self {
            e1.fmt_l10(f, translator.clone())?;
            write!(f, " | ")?;
            e2.fmt_l9(f, translator)
        } else {
            self.fmt_l9(f, translator)
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_l10(f, None)
    }
}

pub struct ExprFormatter<'expr, 'trans> {
    expr: &'expr Expr,
    translator: Option<&'trans Translator>,
}

impl<'expr, 'trans> fmt::Display for ExprFormatter<'expr, 'trans> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.expr.fmt_l10(f, self.translator.clone())
    }
}

impl Expr {
    pub fn display<'expr, 'trans>(&'expr self, translator: Option<&'trans Translator>) -> ExprFormatter<'expr, 'trans> {
        ExprFormatter {
            expr: self,
            translator,
        }
    }
}

impl From<BitVec> for Expr {
    fn from(val: BitVec) -> Self {
        Self::Val(val)
    }
}

impl From<Var> for Expr {
    fn from(var: Var) -> Self {
        Self::Var(var)
    }
}

impl From<VarnodeData> for Expr {
    fn from(vnd: VarnodeData) -> Self {
        if vnd.space().is_constant() {
            Self::from(BitVec::from_u64(vnd.offset(), vnd.size() * 8))
        } else if vnd.space().is_unique() || vnd.space().is_register() {
            Self::from(Var::from(vnd))
        } else {
            let val = BitVec::from_u64(vnd.offset(), vnd.size() * 8);
            let src = if vnd.space().word_size() > 1 {
                let s = Expr::from(val);
                let bits = s.bits();

                let w = Expr::from(BitVec::from_usize(vnd.space().word_size(), bits));

                Expr::int_mul(s, w)
            } else {
                Expr::from(val)
            };
            // address-like
            src
            /*
            Self::load(
                src,
                vnd.space().address_size() * 8,
                vnd.space(),
            )
            */
        }
    }
}

impl Expr {
    pub fn bits(&self) -> usize {
        match self {
            Self::UnRel(_, _) | Self::BinRel(_, _, _) => 1,
            Self::UnOp(_, e) | Self::BinOp(_, e, _) => e.bits(),
            Self::Cast(_, cast) => cast.bits(),
            Self::Load(_, bits, _) => *bits,
            Self::Extract(_, lsb, msb) => *msb - *lsb,
            Self::Concat(l, r) => l.bits() + r.bits(),
            Self::Intrinsic(_, _, bits) => *bits,
            Self::Val(bv) => bv.bits(),
            Self::Var(var) => var.bits(),
        }
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Cast(_, Cast::Bool))
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Self::Cast(_, Cast::Float(_)))
    }

    pub fn is_float_format(&self, format: &FloatFormat) -> bool {
        matches!(self, Self::Cast(_, Cast::Float(f)) if f.as_ref() == format)
    }

    pub fn is_signed(&self) -> bool {
        matches!(self, Self::Cast(_, Cast::Signed(_)))
    }

    pub fn is_signed_bits(&self, bits: usize) -> bool {
        matches!(self, Self::Cast(_, Cast::Signed(sz)) if *sz == bits)
    }

    pub fn is_unsigned(&self) -> bool {
        matches!(self, Self::Cast(_, Cast::Unsigned(_))) || !matches!(self, Self::Cast(_, _))
    }

    pub fn is_unsigned_bits(&self, bits: usize) -> bool {
        matches!(self, Self::Cast(_, Cast::Unsigned(sz)) if *sz == bits)
            || (!matches!(self, Self::Cast(_, _)) && self.bits() == bits)
    }

    pub fn cast_bool<E>(expr: E) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        if expr.is_bool() {
            expr
        } else {
            Self::Cast(Box::new(expr.into()), Cast::Bool)
        }
    }

    pub fn cast_signed<E>(expr: E, bits: usize) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        if expr.is_signed_bits(bits) {
            expr
        } else {
            Self::Cast(Box::new(expr.into()), Cast::Signed(bits))
        }
    }

    pub fn cast_unsigned<E>(expr: E, bits: usize) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        if expr.is_unsigned_bits(bits) {
            expr
        } else {
            Self::Cast(Box::new(expr.into()), Cast::Unsigned(bits))
        }
    }

    pub fn cast_float<E>(expr: E, format: Arc<FloatFormat>) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        if expr.is_float_format(format.as_ref()) {
            expr
        } else {
            Self::Cast(Box::new(expr.into()), Cast::Float(format))
        }
    }

    pub fn extract_high<E>(expr: E, bits: usize) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        if expr.is_unsigned_bits(bits) {
            expr
        } else {
            Self::Cast(Box::new(expr.into()), Cast::High(bits))
        }
    }

    pub fn extract_low<E>(expr: E, bits: usize) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        if expr.is_unsigned_bits(bits) {
            expr
        } else {
            Self::Cast(Box::new(expr.into()), Cast::Low(bits))
        }
    }

    pub(crate) fn unary_op<E>(op: UnOp, expr: E) -> Self
    where
        E: Into<Expr>,
    {
        Self::UnOp(op, Box::new(expr.into()))
    }

    pub(crate) fn unary_rel<E>(rel: UnRel, expr: E) -> Self
    where
        E: Into<Expr>,
    {
        Self::cast_bool(Self::UnRel(rel, Box::new(expr.into())))
    }

    pub(crate) fn binary_op<E1, E2>(op: BinOp, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::BinOp(op, Box::new(expr1.into()), Box::new(expr2.into()))
    }

    pub(crate) fn binary_op_promote_as<E1, E2, F>(op: BinOp, expr1: E1, expr2: E2, cast: F) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
        F: Fn(Expr, usize) -> Expr,
    {
        let e1 = expr1.into();
        let e2 = expr2.into();
        let bits = e1.bits().max(e2.bits());

        Self::binary_op(op, cast(e1, bits), cast(e2, bits))
    }

    pub(crate) fn binary_op_promote<E1, E2>(op: BinOp, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote_as(op, expr1, expr2, |e, sz| Self::cast_unsigned(e, sz))
    }

    pub(crate) fn binary_op_promote_bool<E1, E2>(op: BinOp, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote_as(op, expr1, expr2, |e, _sz| Self::cast_bool(e))
    }

    pub(crate) fn binary_op_promote_signed<E1, E2>(op: BinOp, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote_as(op, expr1, expr2, |e, sz| Self::cast_signed(e, sz))
    }

    pub(crate) fn binary_op_promote_float<E1, E2>(
        op: BinOp,
        expr1: E1,
        expr2: E2,
        formats: &Map<usize, Arc<FloatFormat>>,
    ) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote_as(op, expr1, expr2, |e, sz| {
            Self::cast_float(Self::cast_signed(e, sz), formats[&sz].clone())
        })
    }

    pub(crate) fn binary_rel<E1, E2>(rel: BinRel, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
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
        E1: Into<Expr>,
        E2: Into<Expr>,
        F: Fn(Expr, usize) -> Expr,
    {
        let e1 = expr1.into();
        let e2 = expr2.into();
        let bits = e1.bits().max(e2.bits());

        Self::binary_rel(op, cast(e1, bits), cast(e2, bits))
    }

    pub(crate) fn binary_rel_promote<E1, E2>(op: BinRel, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote_as(op, expr1, expr2, |e, sz| Self::cast_unsigned(e, sz))
    }

    pub(crate) fn binary_rel_promote_float<E1, E2>(
        op: BinRel,
        expr1: E1,
        expr2: E2,
        formats: &Map<usize, Arc<FloatFormat>>,
    ) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote_as(op, expr1, expr2, |e, sz| {
            Self::cast_float(Self::cast_signed(e, sz), formats[&sz].clone())
        })
    }

    pub(crate) fn binary_rel_promote_signed<E1, E2>(op: BinRel, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote_as(op, expr1, expr2, |e, sz| Self::cast_signed(e, sz))
    }

    pub(crate) fn binary_rel_promote_bool<E1, E2>(op: BinRel, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote_as(op, expr1, expr2, |e, _sz| Self::cast_bool(e))
    }

    pub fn load<E>(expr: E, size: usize, space: Arc<AddressSpace>) -> Self
    where
        E: Into<Expr>,
    {
        Self::Load(
            Box::new(Self::cast_unsigned(expr, space.address_size() * 8)),
            size,
            space,
        )
    }

    pub fn intrinsic<I, E>(name: Arc<str>, arguments: I, bits: usize) -> Self
    where
        I: Iterator<Item = E>,
        E: Into<Expr>,
    {
        Self::Intrinsic(
            name.into(),
            arguments.map(|e| Box::new(e.into())).collect(),
            bits,
        )
    }

    pub fn extract<E>(expr: E, loff: usize, moff: usize) -> Self
    where
        E: Into<Expr>,
    {
        Self::Extract(Box::new(expr.into()), loff, moff)
    }
}

impl Expr {
    pub fn bool_not<E>(expr: E) -> Self
    where
        E: Into<Expr>,
    {
        Self::unary_op(UnOp::NOT, Self::cast_bool(expr))
    }

    pub fn bool_eq<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote_bool(BinRel::EQ, expr1, expr2)
    }

    pub fn bool_neq<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote_bool(BinRel::NEQ, expr1, expr2)
    }

    pub fn bool_and<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote_bool(BinOp::AND, expr1, expr2)
    }

    pub fn bool_or<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote_bool(BinOp::OR, expr1, expr2)
    }

    pub fn bool_xor<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote_bool(BinOp::XOR, expr1, expr2)
    }

    pub fn float_nan<E>(expr: E, formats: &Map<usize, Arc<FloatFormat>>) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        let bits = expr.bits();

        let format = formats[&bits].clone();

        Self::unary_rel(
            UnRel::NAN,
            Expr::cast_float(Expr::cast_signed(expr, bits), format),
        )
    }

    pub fn float_neg<E>(expr: E, formats: &Map<usize, Arc<FloatFormat>>) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits].clone();

        Self::unary_op(
            UnOp::NEG,
            Expr::cast_float(Expr::cast_signed(expr, bits), format),
        )
    }

    pub fn float_abs<E>(expr: E, formats: &Map<usize, Arc<FloatFormat>>) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits].clone();

        Self::unary_op(
            UnOp::ABS,
            Expr::cast_float(Expr::cast_signed(expr, bits), format),
        )
    }

    pub fn float_sqrt<E>(expr: E, formats: &Map<usize, Arc<FloatFormat>>) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits].clone();

        Self::unary_op(
            UnOp::SQRT,
            Expr::cast_float(Expr::cast_signed(expr, bits), format),
        )
    }

    pub fn float_ceiling<E>(expr: E, formats: &Map<usize, Arc<FloatFormat>>) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits].clone();

        Self::unary_op(
            UnOp::CEILING,
            Expr::cast_float(Expr::cast_signed(expr, bits), format),
        )
    }

    pub fn float_round<E>(expr: E, formats: &Map<usize, Arc<FloatFormat>>) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits].clone();

        Self::unary_op(
            UnOp::ROUND,
            Expr::cast_float(Expr::cast_signed(expr, bits), format),
        )
    }

    pub fn float_floor<E>(expr: E, formats: &Map<usize, Arc<FloatFormat>>) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits].clone();

        Self::unary_op(
            UnOp::FLOOR,
            Expr::cast_float(Expr::cast_signed(expr, bits), format),
        )
    }

    pub fn float_eq<E1, E2>(expr1: E1, expr2: E2, formats: &Map<usize, Arc<FloatFormat>>) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote_float(BinRel::EQ, expr1, expr2, formats)
    }

    pub fn float_neq<E1, E2>(
        expr1: E1,
        expr2: E2,
        formats: &Map<usize, Arc<FloatFormat>>,
    ) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote_float(BinRel::NEQ, expr1, expr2, formats)
    }

    pub fn float_lt<E1, E2>(expr1: E1, expr2: E2, formats: &Map<usize, Arc<FloatFormat>>) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote_float(BinRel::LT, expr1, expr2, formats)
    }

    pub fn float_le<E1, E2>(expr1: E1, expr2: E2, formats: &Map<usize, Arc<FloatFormat>>) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote_float(BinRel::LE, expr1, expr2, formats)
    }

    pub fn float_add<E1, E2>(
        expr1: E1,
        expr2: E2,
        formats: &Map<usize, Arc<FloatFormat>>,
    ) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote_float(BinOp::ADD, expr1, expr2, formats)
    }

    pub fn float_sub<E1, E2>(
        expr1: E1,
        expr2: E2,
        formats: &Map<usize, Arc<FloatFormat>>,
    ) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote_float(BinOp::SUB, expr1, expr2, formats)
    }

    pub fn float_div<E1, E2>(
        expr1: E1,
        expr2: E2,
        formats: &Map<usize, Arc<FloatFormat>>,
    ) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote_float(BinOp::DIV, expr1, expr2, formats)
    }

    pub fn float_mul<E1, E2>(
        expr1: E1,
        expr2: E2,
        formats: &Map<usize, Arc<FloatFormat>>,
    ) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote_float(BinOp::MUL, expr1, expr2, formats)
    }

    pub fn int_neg<E>(expr: E) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        let size = expr.bits();
        Self::unary_op(UnOp::NEG, Self::cast_signed(expr, size))
    }

    pub fn int_not<E>(expr: E) -> Self
    where
        E: Into<Expr>,
    {
        let expr = expr.into();
        let size = expr.bits();
        Self::unary_op(UnOp::NOT, Self::cast_unsigned(expr, size))
    }

    pub fn int_eq<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote(BinRel::EQ, expr1, expr2)
    }

    pub fn int_neq<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote(BinRel::NEQ, expr1, expr2)
    }

    pub fn int_lt<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote(BinRel::LT, expr1, expr2)
    }

    pub fn int_le<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote(BinRel::LE, expr1, expr2)
    }

    pub fn int_slt<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote_signed(BinRel::SLT, expr1, expr2)
    }

    pub fn int_sle<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote_signed(BinRel::SLE, expr1, expr2)
    }

    pub fn int_carry<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote(BinRel::CARRY, expr1, expr2)
    }

    pub fn int_scarry<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote_signed(BinRel::SCARRY, expr1, expr2)
    }

    pub fn int_sborrow<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_rel_promote_signed(BinRel::SBORROW, expr1, expr2)
    }

    pub fn int_add<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote(BinOp::ADD, expr1, expr2)
    }

    pub fn int_sub<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote(BinOp::SUB, expr1, expr2)
    }

    pub fn int_mul<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote(BinOp::MUL, expr1, expr2)
    }

    pub fn int_div<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote(BinOp::DIV, expr1, expr2)
    }

    pub fn int_sdiv<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote_signed(BinOp::SDIV, expr1, expr2)
    }

    pub fn int_rem<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote(BinOp::REM, expr1, expr2)
    }

    pub fn int_srem<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote_signed(BinOp::SREM, expr1, expr2)
    }

    pub fn int_shl<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote(BinOp::SHL, expr1, expr2)
    }

    pub fn int_shr<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote(BinOp::SHR, expr1, expr2)
    }

    pub fn int_sar<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote_signed(BinOp::SAR, expr1, expr2)
    }

    pub fn int_and<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote(BinOp::AND, expr1, expr2)
    }

    pub fn int_or<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote(BinOp::OR, expr1, expr2)
    }

    pub fn int_xor<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr>,
        E2: Into<Expr>,
    {
        Self::binary_op_promote(BinOp::XOR, expr1, expr2)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stmt {
    Assign(Var, Expr),

    Store(Expr, Expr, usize, Arc<AddressSpace>), // SPACE[T]:SIZE <- T

    Branch(BranchTarget),
    CBranch(Expr, BranchTarget),

    Call(BranchTarget),
    Return(BranchTarget),

    Skip, // NO-OP

    Intrinsic(Arc<str>, SmallVec<[Expr; 4]>), // no output intrinsic
}

impl fmt::Display for Stmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assign(dest, src) => write!(f, "{} ← {}", dest, src),
            Self::Store(dest, src, size, spc) => write!(f, "{}[{}]:{} ← {}", spc.name(), dest, size, src),
            Self::Branch(target) => write!(f, "goto {}", target),
            Self::CBranch(cond, target) => write!(f, "goto {} if {}", target, cond),
            Self::Call(target) => write!(f, "call {}", target),
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

impl<'stmt, 'trans> fmt::Display for StmtFormatter<'stmt, 'trans> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.stmt {
            Stmt::Assign(dest, src) => write!(f, "{} ← {}", dest.display(self.translator.clone()), src.display(self.translator.clone())),
            Stmt::Store(dest, src, size, spc) => write!(f, "{}[{}]:{} ← {}", spc.name(), dest.display(self.translator.clone()), size, src.display(self.translator.clone())),
            Stmt::Branch(target) => write!(f, "goto {}", target.display(self.translator.clone())),
            Stmt::CBranch(cond, target) => write!(f, "goto {} if {}", target.display(self.translator.clone()), cond.display(self.translator.clone())),
            Stmt::Call(target) => write!(f, "call {}", target.display(self.translator.clone())),
            Stmt::Return(target) => write!(f, "return {}", target.display(self.translator.clone())),
            Stmt::Skip => write!(f, "skip"),
            Stmt::Intrinsic(name, args) => {
                write!(f, "{}(", name)?;
                if !args.is_empty() {
                    write!(f, "{}", args[0].display(self.translator.clone()))?;
                    for arg in &args[1..] {
                        write!(f, ", {}", arg.display(self.translator.clone()))?;
                    }
                }
                write!(f, ")")
            }
        }
    }
}

pub struct StmtFormatter<'stmt, 'trans> {
    stmt: &'stmt Stmt,
    translator: Option<&'trans Translator>,
}

impl Stmt {
    pub fn display<'stmt, 'trans>(&'stmt self, translator: Option<&'trans Translator>) -> StmtFormatter<'stmt, 'trans> {
        StmtFormatter {
            stmt: self,
            translator,
        }
    }
}

impl Stmt {
    pub fn from_parts(
        manager: &SpaceManager,
        float_formats: &Map<usize, Arc<FloatFormat>>,
        user_ops: &[Arc<str>],
        address: &AddressValue,
        position: usize,
        opcode: Opcode,
        inputs: SmallVec<[VarnodeData; 16]>,
        output: Option<VarnodeData>,
    ) -> Self {
        let mut inputs = inputs.into_iter();
        let spaces = manager.spaces();
        match opcode {
            Opcode::Copy => Self::assign(output.unwrap(), inputs.next().unwrap()),
            Opcode::Load => {
                let space = &spaces[inputs.next().unwrap().offset() as usize];
                let destination = output.unwrap();
                let source = inputs.next().unwrap();
                let size = destination.size() * 8;

                let src = if space.word_size() > 1 {
                    let s = Expr::from(source);
                    let bits = s.bits();

                    let w = Expr::from(BitVec::from_usize(space.word_size(), bits));

                    Expr::int_mul(s, w)
                } else {
                    source.into()
                };

                Self::assign(destination, Expr::load(src, size, space.clone()))
            }
            Opcode::Store => {
                let space = &spaces[inputs.next().unwrap().offset() as usize];
                let destination = inputs.next().unwrap();
                let source = inputs.next().unwrap();
                let size = source.size() * 8;

                let dest = if space.word_size() > 1 {
                    let d = Expr::from(destination);
                    let bits = d.bits();

                    let w = Expr::from(BitVec::from_usize(space.word_size(), bits));

                    Expr::int_mul(d, w)
                } else {
                    destination.into()
                };

                Self::store(dest, source, size, space.clone())
            }
            Opcode::Branch => {
                let mut target = Location::from(inputs.next().unwrap());
                target.absolute_from(address.to_owned(), position);

                Self::branch(target)
            }
            Opcode::CBranch => {
                let mut target = Location::from(inputs.next().unwrap());
                target.absolute_from(address.to_owned(), position);

                let condition = inputs.next().unwrap();

                Self::branch_conditional(condition, target)
            }
            Opcode::IBranch => {
                let target = inputs.next().unwrap();
                let space = address.space();

                Self::branch_indirect(target, space)
            }
            Opcode::Call => {
                let mut target = Location::from(inputs.next().unwrap());
                target.absolute_from(address.to_owned(), position);

                Self::call(target)
            }
            Opcode::ICall => {
                let target = inputs.next().unwrap();
                let space = address.space();

                Self::call_indirect(target, space)
            }
            Opcode::CallOther => {
                let name = user_ops[inputs.next().unwrap().offset() as usize].clone();
                if let Some(output) = output {
                    let output = Var::from(output);
                    let bits = output.bits();
                    Self::assign(output, Expr::intrinsic(name, inputs, bits))
                } else {
                    Self::intrinsic(name, inputs)
                }
            }
            Opcode::Return => {
                let target = inputs.next().unwrap();
                let space = address.space();

                Self::return_(target, space)
            }
            Opcode::Subpiece => {
                let source = Expr::from(inputs.next().unwrap());
                let src_size = source.bits();

                let output = output.unwrap();
                let out_size = output.size() * 8;

                let loff = inputs.next().unwrap().offset() as usize * 8;
                let trun_size = src_size.checked_sub(loff).unwrap_or(0);

                let trun = if out_size > trun_size {
                    // extract high + expand
                    let source_htrun = Expr::extract_high(source, trun_size);
                    Expr::cast_unsigned(source_htrun, out_size)
                } else {
                    // extract
                    let hoff = loff + out_size;
                    Expr::extract(source, loff, hoff)
                };

                Self::assign(output, trun)
            }
            Opcode::PopCount => {
                let input = inputs.next().unwrap();
                let output = Var::from(output.unwrap());

                let size = output.bits();
                let popcount = Expr::unary_op(UnOp::POPCOUNT, input);

                Self::assign(output, Expr::cast_unsigned(popcount, size))
            }
            Opcode::BoolNot => {
                let input = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::bool_not(input))
            }
            Opcode::BoolAnd => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::bool_and(input1, input2))
            }
            Opcode::BoolOr => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::bool_or(input1, input2))
            }
            Opcode::BoolXor => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::bool_xor(input1, input2))
            }
            Opcode::IntNeg => {
                let input = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_neg(input))
            }
            Opcode::IntNot => {
                let input = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_not(input))
            }
            Opcode::IntSExt => {
                let input = inputs.next().unwrap();
                let output = output.unwrap();
                let size = output.size() * 8;

                Self::assign(output, Expr::cast_signed(input, size))
            }
            Opcode::IntZExt => {
                let input = inputs.next().unwrap();
                let output = output.unwrap();
                let size = output.size() * 8;

                Self::assign(output, Expr::cast_unsigned(input, size))
            }
            Opcode::IntEq => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_eq(input1, input2))
            }
            Opcode::IntNotEq => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_neq(input1, input2))
            }
            Opcode::IntLess => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_lt(input1, input2))
            }
            Opcode::IntLessEq => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_le(input1, input2))
            }
            Opcode::IntSLess => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_slt(input1, input2))
            }
            Opcode::IntSLessEq => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_sle(input1, input2))
            }
            Opcode::IntCarry => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_carry(input1, input2))
            }
            Opcode::IntSCarry => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_scarry(input1, input2))
            }
            Opcode::IntSBorrow => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_sborrow(input1, input2))
            }
            Opcode::IntAdd => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_add(input1, input2))
            }
            Opcode::IntSub => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_sub(input1, input2))
            }
            Opcode::IntDiv => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_div(input1, input2))
            }
            Opcode::IntSDiv => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_sdiv(input1, input2))
            }
            Opcode::IntMul => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_mul(input1, input2))
            }
            Opcode::IntRem => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_rem(input1, input2))
            }
            Opcode::IntSRem => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_srem(input1, input2))
            }
            Opcode::IntLShift => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_shl(input1, input2))
            }
            Opcode::IntRShift => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_shr(input1, input2))
            }
            Opcode::IntSRShift => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_sar(input1, input2))
            }
            Opcode::IntAnd => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_and(input1, input2))
            }
            Opcode::IntOr => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_or(input1, input2))
            }
            Opcode::IntXor => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::int_xor(input1, input2))
            }
            Opcode::FloatIsNaN => {
                let input = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_nan(input, float_formats))
            }
            Opcode::FloatAbs => {
                let input = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_abs(input, float_formats))
            }
            Opcode::FloatNeg => {
                let input = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_neg(input, float_formats))
            }
            Opcode::FloatSqrt => {
                let input = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_sqrt(input, float_formats))
            }
            Opcode::FloatFloor => {
                let input = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_floor(input, float_formats))
            }
            Opcode::FloatCeiling => {
                let input = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_ceiling(input, float_formats))
            }
            Opcode::FloatRound => {
                let input = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_round(input, float_formats))
            }
            Opcode::FloatEq => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_eq(input1, input2, float_formats))
            }
            Opcode::FloatNotEq => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_neq(input1, input2, float_formats))
            }
            Opcode::FloatLess => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_lt(input1, input2, float_formats))
            }
            Opcode::FloatLessEq => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_le(input1, input2, float_formats))
            }
            Opcode::FloatAdd => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_add(input1, input2, float_formats))
            }
            Opcode::FloatSub => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_sub(input1, input2, float_formats))
            }
            Opcode::FloatDiv => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_div(input1, input2, float_formats))
            }
            Opcode::FloatMul => {
                let input1 = inputs.next().unwrap();
                let input2 = inputs.next().unwrap();
                let output = output.unwrap();

                Self::assign(output, Expr::float_mul(input1, input2, float_formats))
            }
            Opcode::FloatOfFloat => {
                let input = Expr::from(inputs.next().unwrap());
                let input_size = input.bits();

                let output = Var::from(output.unwrap());
                let output_size = output.bits();

                let input_format = float_formats[&input_size].clone();
                let output_format = float_formats[&output_size].clone();

                Self::assign(
                    output,
                    Expr::cast_float(Expr::cast_float(input, input_format), output_format),
                )
            }
            Opcode::FloatOfInt => {
                let input = Expr::from(inputs.next().unwrap());
                let input_size = input.bits();

                let output = Var::from(output.unwrap());
                let output_size = output.bits();

                let format = float_formats[&output_size].clone();
                Self::assign(
                    output,
                    Expr::cast_float(Expr::cast_signed(input, input_size), format),
                )
            }
            Opcode::FloatTruncate => {
                let input = Expr::from(inputs.next().unwrap());
                let input_size = input.bits();

                let output = Var::from(output.unwrap());
                let output_size = output.bits();

                let format = float_formats[&input_size].clone();
                Self::assign(
                    output,
                    Expr::cast_signed(Expr::cast_float(input, format), output_size),
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

    pub fn assign<D, S>(destination: D, source: S) -> Self
    where
        D: Into<Var>,
        S: Into<Expr>,
    {
        let dest = destination.into();
        let bits = dest.bits();
        Self::Assign(dest, Expr::cast_unsigned(source, bits))
    }

    pub fn store<D, S>(destination: D, source: S, size: usize, space: Arc<AddressSpace>) -> Self
    where
        D: Into<Expr>,
        S: Into<Expr>,
    {
        Self::Store(destination.into(), source.into(), size, space)
    }

    pub fn branch<T>(target: T) -> Self
    where
        T: Into<BranchTarget>,
    {
        Self::Branch(target.into())
    }

    pub fn branch_conditional<C, T>(condition: C, target: T) -> Self
    where
        C: Into<Expr>,
        T: Into<BranchTarget>,
    {
        Self::CBranch(Expr::cast_bool(condition), target.into())
    }

    pub fn branch_indirect<T>(target: T, space: Arc<AddressSpace>) -> Self
    where
        T: Into<Expr>,
    {
        Self::Branch(BranchTarget::computed(
            Expr::load(target, space.address_size() * 8, space.clone()),
        ))
    }

    pub fn call<T>(target: T) -> Self
    where
        T: Into<BranchTarget>,
    {
        Self::Call(target.into())
    }

    pub fn call_indirect<T>(target: T, space: Arc<AddressSpace>) -> Self
    where
        T: Into<Expr>,
    {
        Self::Call(BranchTarget::computed(
            Expr::load(target, space.address_size() * 8, space.clone()),
        ))
    }

    pub fn return_<T>(target: T, space: Arc<AddressSpace>) -> Self
    where
        T: Into<Expr>,
    {
        Self::Return(BranchTarget::computed(
            Expr::load(target, space.address_size() * 8, space.clone()),
        ))
    }

    pub fn skip() -> Self {
        Self::Skip
    }

    pub fn intrinsic<I, E>(name: Arc<str>, arguments: I) -> Self
    where
        I: Iterator<Item = E>,
        E: Into<Expr>,
    {
        Self::Intrinsic(name, arguments.map(|e| e.into()).collect())
    }
}

#[derive(Debug, Clone)]
pub struct Entity<V> {
    location: Location,
    value: V,
}

impl<V> Entity<V> {
    pub fn new(location: Location, value: V) -> Self {
        Self {
            location,
            value,
        }
    }

    pub fn location(&self) -> &Location {
        &self.location
    }

    pub fn location_mut(&self) -> &Location {
        &self.location
    }

    pub fn value(&self) -> &V {
        &self.value
    }

    pub fn value_mut(&mut self) -> &mut V {
        &mut self.value
    }

    pub fn into_value(self) -> V {
        self.value
    }

    pub fn into_parts(self) -> (Location, V) {
        (self.location, self.value)
    }
}

#[derive(Debug, Clone)]
pub struct ECode {
    pub address: AddressValue,
    pub operations: SmallVec<[Stmt; 16]>,
    pub delay_slots: usize,
    pub length: usize,
}

impl ECode {
    pub fn nop(address: AddressValue, length: usize) -> Self {
        Self {
            address,
            operations: smallvec![Stmt::skip()],
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

    pub fn delay_slots(&self) -> usize {
        self.delay_slots
    }

    pub fn length(&self) -> usize {
        self.length
    }

    pub fn display<'ecode, 'trans>(&'ecode self, translator: &'trans Translator) -> ECodeFormatter<'ecode, 'trans> {
        ECodeFormatter {
            ecode: self,
            translator,
        }
    }
}

impl fmt::Display for ECode {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let len =  self.operations.len();
        if len > 0 {
            for (i, op) in self.operations.iter().enumerate() {
                write!(f, "{}.{:02}: {}{}",
                       self.address,
                       i,
                       op,
                       if i == len - 1 { "" } else { "\n" })?;
            }
            Ok(())
        } else {
            write!(f, "{}.00: skip", self.address)
        }
    }
}

pub struct ECodeFormatter<'ecode, 'trans> {
    ecode: &'ecode ECode,
    translator: &'trans Translator,
}

impl<'ecode, 'trans> fmt::Display for ECodeFormatter<'ecode, 'trans> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let len =  self.ecode.operations.len();
        if len > 0 {
            for (i, op) in self.ecode.operations.iter().enumerate() {
                write!(f, "{}.{:02}: {}{}", self.ecode.address, i,
                       op.display(Some(self.translator)),
                       if i == len - 1 { "" } else { "\n" })?;
            }
            Ok(())
        } else {
            write!(f, "{}.00: skip", self.ecode.address)
        }
    }
}
