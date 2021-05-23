use std::borrow::Cow;
use std::fmt;

use crate::float_format::FloatFormat;
use crate::space::AddressSpace;
use crate::space_manager::SpaceManager;
use crate::{Address, Opcode, Translator, VarnodeData};

use fnv::FnvHashMap as Map;
use fugue_bv::BitVec;
use smallvec::SmallVec;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Var<'space> {
    space: &'space AddressSpace,
    offset: u64,
    bits: usize,
    generation: usize,
}

impl<'space> Var<'space> {
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
            generation,
            ..*self
        }
    }

    pub fn space(&self) -> &'space AddressSpace {
        self.space
    }
}

impl<'space> fmt::Display for Var<'space> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display(None))
    }
}

impl<'var, 'space> fmt::Display for VarFormatter<'var, 'space> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.translator.is_some() && self.var.space().is_register() {
            write!(f, "{}:{}", self.translator.unwrap().registers()[&(self.var.offset(), self.var.bits() / 8)], self.var.bits())
        } else {
            write!(f, "{}[{:#x}]:{}", self.var.space().name(), self.var.offset(), self.var.bits())
        }
    }
}

pub struct VarFormatter<'var, 'space> {
    var: &'var Var<'space>,
    translator: Option<&'space Translator>,
}

impl<'space> Var<'space> {
    fn display<'var>(&'var self, translator: Option<&'space Translator>) -> VarFormatter<'var, 'space> {
        VarFormatter {
            var: self,
            translator,
        }
    }
}

impl<'space> From<VarnodeData<'space>> for Var<'space> {
    fn from(vnd: VarnodeData<'space>) -> Self {
        Self {
            space: vnd.space(),
            offset: vnd.offset(),
            bits: vnd.size() * 8,
            generation: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Location<'space> {
    address: Address<'space>,
    position: usize,
}

impl<'space> fmt::Display for Location<'space> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.address, self.position)
    }
}

impl<'space> Location<'space> {
    pub fn address(&self) -> Cow<Address<'space>> {
        Cow::Borrowed(&self.address)
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn space(&self) -> &'space AddressSpace {
        self.address.space()
    }

    pub fn is_relative(&self) -> bool {
        self.space().is_constant() && self.position == 0
    }

    pub fn is_absolute(&self) -> bool {
        !self.is_relative()
    }

    pub(crate) fn absolute_from<A>(&mut self, address: A, position: usize)
    where
        A: Into<Address<'space>>,
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

impl<'space> From<VarnodeData<'space>> for Location<'space> {
    fn from(vnd: VarnodeData<'space>) -> Self {
        Self {
            address: Address::new(vnd.space(), vnd.offset()),
            position: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BranchTarget<'space> {
    Location(Location<'space>),
    Computed(Expr<'space>, &'space AddressSpace),
}

impl<'space> fmt::Display for BranchTarget<'space> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display(None))
    }
}

pub struct BranchTargetFormatter<'target, 'space> {
    target: &'target BranchTarget<'space>,
    translator: Option<&'space Translator>,
}

impl<'target, 'space> fmt::Display for BranchTargetFormatter<'target, 'space> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.target {
            BranchTarget::Location(loc) => write!(f, "{}", loc),
            BranchTarget::Computed(expr, spc) => write!(f, "{}[{}]", spc.name(), expr.display(self.translator.clone())),
        }
    }
}

impl<'space> BranchTarget<'space> {
    fn display<'target>(&'target self, translator: Option<&'space Translator>) -> BranchTargetFormatter<'target, 'space> {
        BranchTargetFormatter {
            target: self,
            translator,
        }
    }
}

impl<'space> From<Location<'space>> for BranchTarget<'space> {
    fn from(t: Location<'space>) -> Self {
        Self::Location(t)
    }
}

impl<'space> BranchTarget<'space> {
    pub fn computed<E: Into<Expr<'space>>>(expr: E, space: &'space AddressSpace) -> Self {
        Self::Computed(expr.into(), space)
    }

    pub fn is_computed(&self) -> bool {
        matches!(self, Self::Computed(_, _))
    }

    pub fn location<L: Into<Location<'space>>>(location: L) -> Self {
        Self::from(location.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cast<'space> {
    Bool,                       // T -> Bool
    Float(&'space FloatFormat), // T -> FloatFormat::T

    Signed(usize),   // sign-extension
    Unsigned(usize), // zero-extension

    High(usize), // truncate keep MSBs
    Low(usize),  // truncate keep LSBs
}

impl<'space> fmt::Display for Cast<'space> {
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

impl<'space> Cast<'space> {
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
pub enum Expr<'space> {
    UnRel(UnRel, Box<Expr<'space>>),                      // T -> bool
    BinRel(BinRel, Box<Expr<'space>>, Box<Expr<'space>>), // T * T -> bool

    UnOp(UnOp, Box<Expr<'space>>),                      // T -> T
    BinOp(BinOp, Box<Expr<'space>>, Box<Expr<'space>>), // T * T -> T

    Cast(Box<Expr<'space>>, Cast<'space>), // T -> Cast::T
    Load(Box<Expr<'space>>, usize, &'space AddressSpace), // SPACE[T]:SIZE -> T

    Extract(Box<Expr<'space>>, usize, usize), // T T[LSB..MSB) -> T
    Concat(Box<Expr<'space>>, Box<Expr<'space>>), // T * T -> T

    Intrinsic(&'space str, SmallVec<[Box<Expr<'space>>; 4]>, usize),

    Val(BitVec),      // BitVec -> T
    Var(Var<'space>), // String * usize -> T
}

impl<'space> Expr<'space> {
    fn fmt_l1(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'space Translator>) -> fmt::Result {
        match self {
            Expr::Val(v) => write!(f, "{}", v),
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

    fn fmt_l2(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'space Translator>) -> fmt::Result {
        match self {
            Expr::UnOp(UnOp::NEG, expr) => { write!(f, "-")?; expr.fmt_l1(f, translator) },
            Expr::UnOp(UnOp::NOT, expr) => { write!(f, "!")?; expr.fmt_l1(f, translator) },
            expr => expr.fmt_l1(f, translator)
        }
    }

    fn fmt_l3(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'space Translator>) -> fmt::Result {
        match self {
            Expr::BinOp(BinOp::MUL, e1, e2) => { e1.fmt_l3(f, translator.clone())?; write!(f, " * ")?; e2.fmt_l2(f, translator) }
            Expr::BinOp(BinOp::DIV, e1, e2) => { e1.fmt_l3(f, translator.clone())?; write!(f, " / ")?; e2.fmt_l2(f, translator) }
            Expr::BinOp(BinOp::SDIV, e1, e2) => { e1.fmt_l3(f, translator.clone())?; write!(f, " s/ ")?; e2.fmt_l2(f, translator) }
            Expr::BinOp(BinOp::REM, e1, e2) => { e1.fmt_l3(f, translator.clone())?; write!(f, " % ")?; e2.fmt_l2(f, translator) }
            Expr::BinOp(BinOp::SREM, e1, e2) => { e1.fmt_l3(f, translator.clone())?; write!(f, " s% ")?; e2.fmt_l2(f, translator) }
            expr => expr.fmt_l2(f, translator)
        }
    }

    fn fmt_l4(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'space Translator>) -> fmt::Result {
        match self {
            Expr::BinOp(BinOp::ADD, e1, e2) => { e1.fmt_l4(f, translator.clone())?; write!(f, " + ")?; e2.fmt_l3(f, translator) },
            Expr::BinOp(BinOp::SUB, e1, e2) => { e1.fmt_l4(f, translator.clone())?; write!(f, " - ")?; e2.fmt_l3(f, translator) },
            expr => expr.fmt_l3(f, translator)
        }
    }

    fn fmt_l5(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'space Translator>) -> fmt::Result {
        match self {
            Expr::BinOp(BinOp::SHL, e1, e2) => { e1.fmt_l5(f, translator.clone())?; write!(f, " << ")?; e2.fmt_l4(f, translator) },
            Expr::BinOp(BinOp::SHR, e1, e2) => { e1.fmt_l5(f, translator.clone())?; write!(f, " >> ")?; e2.fmt_l4(f, translator) },
            Expr::BinOp(BinOp::SAR, e1, e2) => { e1.fmt_l5(f, translator.clone())?; write!(f, " s>> ")?; e2.fmt_l4(f, translator) },
            expr => expr.fmt_l4(f, translator)
        }
    }

    fn fmt_l6(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'space Translator>) -> fmt::Result {
        match self {
            Expr::BinRel(BinRel::LT, e1, e2) => { e1.fmt_l6(f, translator.clone())?; write!(f, " < ")?; e2.fmt_l5(f, translator) },
            Expr::BinRel(BinRel::LE, e1, e2) => { e1.fmt_l6(f, translator.clone())?; write!(f, " <= ")?; e2.fmt_l5(f, translator) },
            Expr::BinRel(BinRel::SLT, e1, e2) => { e1.fmt_l6(f, translator.clone())?; write!(f, " s< ")?; e2.fmt_l5(f, translator) },
            Expr::BinRel(BinRel::SLE, e1, e2) => { e1.fmt_l6(f, translator.clone())?; write!(f, " s<= ")?; e2.fmt_l5(f, translator) },
            expr => expr.fmt_l5(f, translator)
        }
    }

    fn fmt_l7(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'space Translator>) -> fmt::Result {
        match self {
            Expr::BinRel(BinRel::EQ, e1, e2) => { e1.fmt_l7(f, translator.clone())?; write!(f, " == ")?; e2.fmt_l6(f, translator) },
            Expr::BinRel(BinRel::NEQ, e1, e2) => { e1.fmt_l7(f, translator.clone())?; write!(f, " != ")?; e2.fmt_l6(f, translator) },
            expr => expr.fmt_l6(f, translator)
        }
    }

    fn fmt_l8(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'space Translator>) -> fmt::Result {
        if let Expr::BinOp(BinOp::AND, e1, e2) = self {
            e1.fmt_l8(f, translator.clone())?;
            write!(f, " & ")?;
            e2.fmt_l7(f, translator)
        } else {
            self.fmt_l7(f, translator)
        }
    }

    fn fmt_l9(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'space Translator>) -> fmt::Result {
        if let Expr::BinOp(BinOp::XOR, e1, e2) = self {
            e1.fmt_l9(f, translator.clone())?;
            write!(f, " ^ ")?;
            e2.fmt_l8(f, translator)
        } else {
            self.fmt_l8(f, translator)
        }
    }

    fn fmt_l10(&self, f: &mut fmt::Formatter<'_>, translator: Option<&'space Translator>) -> fmt::Result {
        if let Expr::BinOp(BinOp::OR, e1, e2) = self {
            e1.fmt_l10(f, translator.clone())?;
            write!(f, " | ")?;
            e2.fmt_l9(f, translator)
        } else {
            self.fmt_l9(f, translator)
        }
    }
}

impl<'space> fmt::Display for Expr<'space> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_l10(f, None)
    }
}

pub struct ExprFormatter<'expr, 'space> {
    expr: &'expr Expr<'space>,
    translator: Option<&'space Translator>,
}

impl<'expr, 'space> fmt::Display for ExprFormatter<'expr, 'space> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.expr.fmt_l10(f, self.translator.clone())
    }
}

impl<'space> Expr<'space> {
    pub fn display<'expr>(&'expr self, translator: Option<&'space Translator>) -> ExprFormatter<'expr, 'space> {
        ExprFormatter {
            expr: self,
            translator,
        }
    }
}

impl<'space> From<BitVec> for Expr<'space> {
    fn from(val: BitVec) -> Self {
        Self::Val(val)
    }
}

impl<'space> From<Var<'space>> for Expr<'space> {
    fn from(var: Var<'space>) -> Self {
        Self::Var(var)
    }
}

impl<'space> From<VarnodeData<'space>> for Expr<'space> {
    fn from(vnd: VarnodeData<'space>) -> Self {
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
            Self::load(
                src,
                vnd.space().address_size(),
                vnd.space(),
            )
        }
    }
}

impl<'space> Expr<'space> {
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
        matches!(self, Self::Cast(_, Cast::Float(f)) if *f == format)
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
        E: Into<Expr<'space>>,
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
        E: Into<Expr<'space>>,
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
        E: Into<Expr<'space>>,
    {
        let expr = expr.into();
        if expr.is_unsigned_bits(bits) {
            expr
        } else {
            Self::Cast(Box::new(expr.into()), Cast::Unsigned(bits))
        }
    }

    pub fn cast_float<E>(expr: E, format: &'space FloatFormat) -> Self
    where
        E: Into<Expr<'space>>,
    {
        let expr = expr.into();
        if expr.is_float_format(format) {
            expr
        } else {
            Self::Cast(Box::new(expr.into()), Cast::Float(format))
        }
    }

    pub fn extract_high<E>(expr: E, bits: usize) -> Self
    where
        E: Into<Expr<'space>>,
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
        E: Into<Expr<'space>>,
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
        E: Into<Expr<'space>>,
    {
        Self::UnOp(op, Box::new(expr.into()))
    }

    pub(crate) fn unary_rel<E>(rel: UnRel, expr: E) -> Self
    where
        E: Into<Expr<'space>>,
    {
        Self::cast_bool(Self::UnRel(rel, Box::new(expr.into())))
    }

    pub(crate) fn binary_op<E1, E2>(op: BinOp, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::BinOp(op, Box::new(expr1.into()), Box::new(expr2.into()))
    }

    pub(crate) fn binary_op_promote_as<E1, E2, F>(op: BinOp, expr1: E1, expr2: E2, cast: F) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
        F: Fn(Expr<'space>, usize) -> Expr<'space>,
    {
        let e1 = expr1.into();
        let e2 = expr2.into();
        let bits = e1.bits().max(e2.bits());

        Self::binary_op(op, cast(e1, bits), cast(e2, bits))
    }

    pub(crate) fn binary_op_promote<E1, E2>(op: BinOp, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote_as(op, expr1, expr2, |e, sz| Self::cast_unsigned(e, sz))
    }

    pub(crate) fn binary_op_promote_bool<E1, E2>(op: BinOp, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote_as(op, expr1, expr2, |e, _sz| Self::cast_bool(e))
    }

    pub(crate) fn binary_op_promote_signed<E1, E2>(op: BinOp, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote_as(op, expr1, expr2, |e, sz| Self::cast_signed(e, sz))
    }

    pub(crate) fn binary_op_promote_float<E1, E2>(
        op: BinOp,
        expr1: E1,
        expr2: E2,
        formats: &Map<usize, &'space FloatFormat>,
    ) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote_as(op, expr1, expr2, |e, sz| {
            Self::cast_float(Self::cast_signed(e, sz), formats[&sz])
        })
    }

    pub(crate) fn binary_rel<E1, E2>(rel: BinRel, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
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
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
        F: Fn(Expr<'space>, usize) -> Expr<'space>,
    {
        let e1 = expr1.into();
        let e2 = expr2.into();
        let bits = e1.bits().max(e2.bits());

        Self::binary_rel(op, cast(e1, bits), cast(e2, bits))
    }

    pub(crate) fn binary_rel_promote<E1, E2>(op: BinRel, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote_as(op, expr1, expr2, |e, sz| Self::cast_unsigned(e, sz))
    }

    pub(crate) fn binary_rel_promote_float<E1, E2>(
        op: BinRel,
        expr1: E1,
        expr2: E2,
        formats: &Map<usize, &'space FloatFormat>,
    ) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote_as(op, expr1, expr2, |e, sz| {
            Self::cast_float(Self::cast_signed(e, sz), formats[&sz])
        })
    }

    pub(crate) fn binary_rel_promote_signed<E1, E2>(op: BinRel, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote_as(op, expr1, expr2, |e, sz| Self::cast_signed(e, sz))
    }

    pub(crate) fn binary_rel_promote_bool<E1, E2>(op: BinRel, expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote_as(op, expr1, expr2, |e, _sz| Self::cast_bool(e))
    }

    pub fn load<E>(expr: E, size: usize, space: &'space AddressSpace) -> Self
    where
        E: Into<Expr<'space>>,
    {
        Self::Load(
            Box::new(Self::cast_unsigned(expr, space.address_size())),
            size,
            space,
        )
    }

    pub fn intrinsic<N, I, E>(name: N, arguments: I, bits: usize) -> Self
    where
        N: Into<&'space str>,
        I: Iterator<Item = E>,
        E: Into<Expr<'space>>,
    {
        Self::Intrinsic(
            name.into(),
            arguments.map(|e| Box::new(e.into())).collect(),
            bits,
        )
    }

    pub fn extract<E>(expr: E, loff: usize, moff: usize) -> Self
    where
        E: Into<Expr<'space>>,
    {
        Self::Extract(Box::new(expr.into()), loff, moff)
    }
}

impl<'space> Expr<'space> {
    pub fn bool_not<E>(expr: E) -> Self
    where
        E: Into<Expr<'space>>,
    {
        Self::unary_op(UnOp::NOT, Self::cast_bool(expr))
    }

    pub fn bool_eq<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote_bool(BinRel::EQ, expr1, expr2)
    }

    pub fn bool_neq<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote_bool(BinRel::NEQ, expr1, expr2)
    }

    pub fn bool_and<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote_bool(BinOp::AND, expr1, expr2)
    }

    pub fn bool_or<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote_bool(BinOp::OR, expr1, expr2)
    }

    pub fn bool_xor<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote_bool(BinOp::XOR, expr1, expr2)
    }

    pub fn float_nan<E>(expr: E, formats: &Map<usize, &'space FloatFormat>) -> Self
    where
        E: Into<Expr<'space>>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits];

        Self::unary_rel(
            UnRel::NAN,
            Expr::cast_float(Expr::cast_signed(expr, bits), format),
        )
    }

    pub fn float_neg<E>(expr: E, formats: &Map<usize, &'space FloatFormat>) -> Self
    where
        E: Into<Expr<'space>>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits];

        Self::unary_op(
            UnOp::NEG,
            Expr::cast_float(Expr::cast_signed(expr, bits), format),
        )
    }

    pub fn float_abs<E>(expr: E, formats: &Map<usize, &'space FloatFormat>) -> Self
    where
        E: Into<Expr<'space>>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits];

        Self::unary_op(
            UnOp::ABS,
            Expr::cast_float(Expr::cast_signed(expr, bits), format),
        )
    }

    pub fn float_sqrt<E>(expr: E, formats: &Map<usize, &'space FloatFormat>) -> Self
    where
        E: Into<Expr<'space>>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits];

        Self::unary_op(
            UnOp::SQRT,
            Expr::cast_float(Expr::cast_signed(expr, bits), format),
        )
    }

    pub fn float_ceiling<E>(expr: E, formats: &Map<usize, &'space FloatFormat>) -> Self
    where
        E: Into<Expr<'space>>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits];

        Self::unary_op(
            UnOp::CEILING,
            Expr::cast_float(Expr::cast_signed(expr, bits), format),
        )
    }

    pub fn float_round<E>(expr: E, formats: &Map<usize, &'space FloatFormat>) -> Self
    where
        E: Into<Expr<'space>>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits];

        Self::unary_op(
            UnOp::ROUND,
            Expr::cast_float(Expr::cast_signed(expr, bits), format),
        )
    }

    pub fn float_floor<E>(expr: E, formats: &Map<usize, &'space FloatFormat>) -> Self
    where
        E: Into<Expr<'space>>,
    {
        let expr = expr.into();
        let bits = expr.bits();
        let format = formats[&bits];

        Self::unary_op(
            UnOp::FLOOR,
            Expr::cast_float(Expr::cast_signed(expr, bits), format),
        )
    }

    pub fn float_eq<E1, E2>(expr1: E1, expr2: E2, formats: &Map<usize, &'space FloatFormat>) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote_float(BinRel::EQ, expr1, expr2, formats)
    }

    pub fn float_neq<E1, E2>(
        expr1: E1,
        expr2: E2,
        formats: &Map<usize, &'space FloatFormat>,
    ) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote_float(BinRel::NEQ, expr1, expr2, formats)
    }

    pub fn float_lt<E1, E2>(expr1: E1, expr2: E2, formats: &Map<usize, &'space FloatFormat>) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote_float(BinRel::LT, expr1, expr2, formats)
    }

    pub fn float_le<E1, E2>(expr1: E1, expr2: E2, formats: &Map<usize, &'space FloatFormat>) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote_float(BinRel::LE, expr1, expr2, formats)
    }

    pub fn float_add<E1, E2>(
        expr1: E1,
        expr2: E2,
        formats: &Map<usize, &'space FloatFormat>,
    ) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote_float(BinOp::ADD, expr1, expr2, formats)
    }

    pub fn float_sub<E1, E2>(
        expr1: E1,
        expr2: E2,
        formats: &Map<usize, &'space FloatFormat>,
    ) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote_float(BinOp::SUB, expr1, expr2, formats)
    }

    pub fn float_div<E1, E2>(
        expr1: E1,
        expr2: E2,
        formats: &Map<usize, &'space FloatFormat>,
    ) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote_float(BinOp::DIV, expr1, expr2, formats)
    }

    pub fn float_mul<E1, E2>(
        expr1: E1,
        expr2: E2,
        formats: &Map<usize, &'space FloatFormat>,
    ) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote_float(BinOp::MUL, expr1, expr2, formats)
    }

    pub fn int_neg<E>(expr: E) -> Self
    where
        E: Into<Expr<'space>>,
    {
        let expr = expr.into();
        let size = expr.bits();
        Self::unary_op(UnOp::NEG, Self::cast_signed(expr, size))
    }

    pub fn int_not<E>(expr: E) -> Self
    where
        E: Into<Expr<'space>>,
    {
        let expr = expr.into();
        let size = expr.bits();
        Self::unary_op(UnOp::NOT, Self::cast_unsigned(expr, size))
    }

    pub fn int_eq<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote(BinRel::EQ, expr1, expr2)
    }

    pub fn int_neq<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote(BinRel::NEQ, expr1, expr2)
    }

    pub fn int_lt<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote(BinRel::LT, expr1, expr2)
    }

    pub fn int_le<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote(BinRel::LE, expr1, expr2)
    }

    pub fn int_slt<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote_signed(BinRel::SLT, expr1, expr2)
    }

    pub fn int_sle<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote_signed(BinRel::SLE, expr1, expr2)
    }

    pub fn int_carry<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote(BinRel::CARRY, expr1, expr2)
    }

    pub fn int_scarry<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote_signed(BinRel::SCARRY, expr1, expr2)
    }

    pub fn int_sborrow<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_rel_promote_signed(BinRel::SBORROW, expr1, expr2)
    }

    pub fn int_add<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote(BinOp::ADD, expr1, expr2)
    }

    pub fn int_sub<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote(BinOp::SUB, expr1, expr2)
    }

    pub fn int_mul<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote(BinOp::MUL, expr1, expr2)
    }

    pub fn int_div<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote(BinOp::DIV, expr1, expr2)
    }

    pub fn int_sdiv<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote_signed(BinOp::SDIV, expr1, expr2)
    }

    pub fn int_rem<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote(BinOp::REM, expr1, expr2)
    }

    pub fn int_srem<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote_signed(BinOp::SREM, expr1, expr2)
    }

    pub fn int_shl<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote(BinOp::SHL, expr1, expr2)
    }

    pub fn int_shr<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote(BinOp::SHR, expr1, expr2)
    }

    pub fn int_sar<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote_signed(BinOp::SAR, expr1, expr2)
    }

    pub fn int_and<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote(BinOp::AND, expr1, expr2)
    }

    pub fn int_or<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote(BinOp::OR, expr1, expr2)
    }

    pub fn int_xor<E1, E2>(expr1: E1, expr2: E2) -> Self
    where
        E1: Into<Expr<'space>>,
        E2: Into<Expr<'space>>,
    {
        Self::binary_op_promote(BinOp::XOR, expr1, expr2)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stmt<'space> {
    Assign(Var<'space>, Expr<'space>),

    Store(Expr<'space>, Expr<'space>, usize, &'space AddressSpace), // SPACE[T]:SIZE <- T

    Branch(BranchTarget<'space>),
    CBranch(Expr<'space>, BranchTarget<'space>),

    Call(BranchTarget<'space>),
    Return(BranchTarget<'space>),

    Skip, // NO-OP

    Intrinsic(&'space str, SmallVec<[Box<Expr<'space>>; 4]>), // no output intrinsic
}

impl<'space> fmt::Display for Stmt<'space> {
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

impl<'stmt, 'space> fmt::Display for StmtFormatter<'stmt, 'space> {
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

pub struct StmtFormatter<'stmt, 'space> {
    stmt: &'stmt Stmt<'space>,
    translator: Option<&'space Translator>,
}

impl<'space> Stmt<'space> {
    pub fn display<'stmt>(&'stmt self, translator: Option<&'space Translator>) -> StmtFormatter<'stmt, 'space> {
        StmtFormatter {
            stmt: self,
            translator,
        }
    }
}

impl<'space> Stmt<'space> {
    pub fn from_parts(
        manager: &'space SpaceManager,
        float_formats: &Map<usize, &'space FloatFormat>,
        user_ops: &'space [&'space str],
        address: &Address<'space>,
        position: usize,
        opcode: Opcode,
        inputs: SmallVec<[VarnodeData<'space>; 16]>,
        output: Option<VarnodeData<'space>>,
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

                Self::assign(destination, Expr::load(src, size, space))
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

                Self::store(dest, source, size, space)
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
                let name = user_ops[inputs.next().unwrap().offset() as usize];
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
                let size = inputs.next().unwrap().offset() as usize;
                let output = output.unwrap();

                Self::assign(output, Expr::cast_signed(input, size))
            }
            Opcode::IntZExt => {
                let input = inputs.next().unwrap();
                let size = inputs.next().unwrap().offset() as usize;
                let output = output.unwrap();

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

                let input_format = float_formats[&input_size];
                let output_format = float_formats[&output_size];

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

                let format = float_formats[&output_size];
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

                let format = float_formats[&input_size];
                Self::assign(
                    output,
                    Expr::cast_signed(Expr::cast_float(input, format), output_size),
                )
            }
            Opcode::Build
            | Opcode::CrossBuild
            | Opcode::CPoolRef
            | Opcode::Piece
            | Opcode::Extract
            | Opcode::DelaySlot
            | Opcode::New
            | Opcode::Insert
            | Opcode::Cast
            | Opcode::Label
            | Opcode::SegmentOp => {
                panic!("unimplemented due to spec.")
            }
        }
    }

    pub fn assign<D, S>(destination: D, source: S) -> Self
    where
        D: Into<Var<'space>>,
        S: Into<Expr<'space>>,
    {
        let dest = destination.into();
        let bits = dest.bits();
        Self::Assign(dest, Expr::cast_unsigned(source, bits))
    }

    pub fn store<D, S>(destination: D, source: S, size: usize, space: &'space AddressSpace) -> Self
    where
        D: Into<Expr<'space>>,
        S: Into<Expr<'space>>,
    {
        Self::Store(destination.into(), source.into(), size, space)
    }

    pub fn branch<T>(target: T) -> Self
    where
        T: Into<BranchTarget<'space>>,
    {
        Self::Branch(target.into())
    }

    pub fn branch_conditional<C, T>(condition: C, target: T) -> Self
    where
        C: Into<Expr<'space>>,
        T: Into<BranchTarget<'space>>,
    {
        Self::CBranch(Expr::cast_bool(condition), target.into())
    }

    pub fn branch_indirect<T>(target: T, space: &'space AddressSpace) -> Self
    where
        T: Into<Expr<'space>>,
    {
        Self::Branch(BranchTarget::computed(
            Expr::load(target, space.address_size(), space),
            space,
        ))
    }

    pub fn call<T>(target: T) -> Self
    where
        T: Into<BranchTarget<'space>>,
    {
        Self::Call(target.into())
    }

    pub fn call_indirect<T>(target: T, space: &'space AddressSpace) -> Self
    where
        T: Into<Expr<'space>>,
    {
        Self::Call(BranchTarget::computed(
            Expr::load(target, space.address_size(), space),
            space,
        ))
    }

    pub fn return_<T>(target: T, space: &'space AddressSpace) -> Self
    where
        T: Into<Expr<'space>>,
    {
        Self::Return(BranchTarget::computed(
            Expr::load(target, space.address_size(), space),
            space,
        ))
    }

    pub fn skip() -> Self {
        Self::Skip
    }

    pub fn intrinsic<N, I, E>(name: N, arguments: I) -> Self
    where
        N: Into<&'space str>,
        I: Iterator<Item = E>,
        E: Into<Expr<'space>>,
    {
        Self::Intrinsic(name.into(), arguments.map(|e| Box::new(e.into())).collect())
    }
}
