use fugue_ir::disassembly::construct::{
    ConstTpl, ConstructTpl, HandleKind, HandleTpl, OpTpl, VarnodeTpl,
};
use fugue_ir::disassembly::opcode::Opcode;
use fugue_ir::Translator;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

pub struct TplAdaptor<'a, T> {
    translator: &'a Translator,
    tpl: &'a T,
}

impl<'a, T> TplAdaptor<'a, T> {
    pub fn new(translator: &'a Translator, tpl: &'a T) -> Self {
        Self { translator, tpl }
    }

    pub fn wrap<U>(&self, tpl: &'a U) -> TplAdaptor<'a, U> {
        TplAdaptor {
            translator: &self.translator,
            tpl,
        }
    }
}

impl<'a> ToTokens for TplAdaptor<'a, ConstructTpl> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let delay_slot = self.tpl.delay_slot();
        let labels = self.tpl.labels() as u8;
        let result = self.tpl.result().map_or_else(
            || quote! { None },
            |tpl| {
                let tpl = self.wrap(tpl);
                quote! { Some(#tpl) }
            },
        );
        let operations = self.tpl.operations().iter().map(|tpl| self.wrap(tpl));

        let tpl = quote! {
            fugue_lifter_runtime::template::ConstructTpl {
                delay_slot: #delay_slot,
                labels: #labels,
                result: #result,
                operations: &[#(#operations),*],
            }
        };

        tpl.to_tokens(tokens);
    }
}

impl<'a> ToTokens for TplAdaptor<'a, HandleTpl> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let space = self.wrap(self.tpl.space());
        let size = self.wrap(self.tpl.size());
        let ptr_space = self.wrap(self.tpl.ptr_space());
        let ptr_offset = self.wrap(self.tpl.ptr_offset());
        let ptr_size = self.wrap(self.tpl.ptr_size());
        let tmp_space = self.wrap(self.tpl.tmp_space());
        let tmp_offset = self.wrap(self.tpl.tmp_offset());

        let tpl = quote! {
            fugue_lifter_runtime::template::HandleTpl {
                space: #space,
                size: #size,
                ptr_space: #ptr_space,
                ptr_offset: #ptr_offset,
                ptr_size: #ptr_size,
                tmp_space: #tmp_space,
                tmp_offset: #tmp_offset,
            }
        };

        tpl.to_tokens(tokens);
    }
}

impl<'a> ToTokens for TplAdaptor<'a, ConstTpl> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use ConstTpl as C;
        use HandleKind as H;

        let tpl = match self.tpl {
            C::Real(val) => quote! { fugue_lifter_runtime::template::ConstTpl::Real(#val) },
            C::Handle(index, kind) => {
                let kind = match kind {
                    H::Space => quote! { fugue_lifter_runtime::template::HandleKind::Space },
                    H::Offset => quote! { fugue_lifter_runtime::template::HandleKind::Offset },
                    H::Size => quote! { fugue_lifter_runtime::template::HandleKind::Size },
                    H::OffsetPlus(val) => {
                        quote! { fugue_lifter_runtime::template::HandleKind::OffsetPlus(#val) }
                    }
                };
                quote! { fugue_lifter_runtime::template::ConstTpl::Handle(#index, #kind) }
            }
            C::Start => quote! { fugue_lifter_runtime::template::ConstTpl::Start },
            C::Next => quote! { fugue_lifter_runtime::template::ConstTpl::Next },
            C::Next2 => quote! { fugue_lifter_runtime::template::ConstTpl::Next2 },
            C::CurrentSpace => quote! { fugue_lifter_runtime::template::ConstTpl::CurrentSpace },
            C::CurrentSpaceSize => {
                quote! { fugue_lifter_runtime::template::ConstTpl::CurrentSpaceSize }
            }
            C::SpaceId(id) => {
                let id = id.index() as u8;
                quote! { fugue_lifter_runtime::template::ConstTpl::SpaceId(#id) }
            }
            C::Relative(val) => {
                quote! { fugue_lifter_runtime::template::ConstTpl::Relative(#val) }
            }
            _ => unimplemented!("flow operations not supported"),
        };

        tpl.to_tokens(tokens);
    }
}

impl<'a> ToTokens for TplAdaptor<'a, OpTpl> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use Opcode as O;

        let op = match self.tpl.opcode() {
            O::Copy => quote! { fugue_lifter_runtime::template::Op::Copy },
            O::Load => quote! { fugue_lifter_runtime::template::Op::Load },
            O::Store => quote! { fugue_lifter_runtime::template::Op::Store },
            O::Branch => quote! { fugue_lifter_runtime::template::Op::Branch },
            O::CBranch => quote! { fugue_lifter_runtime::template::Op::CBranch },
            O::IBranch => quote! { fugue_lifter_runtime::template::Op::IBranch },
            O::Call => quote! { fugue_lifter_runtime::template::Op::Call },
            O::ICall => quote! { fugue_lifter_runtime::template::Op::ICall },
            O::CallOther => quote! { fugue_lifter_runtime::template::Op::CallOther },
            O::Return => quote! { fugue_lifter_runtime::template::Op::Return },
            O::IntEq => quote! { fugue_lifter_runtime::template::Op::IntEq },
            O::IntNotEq => quote! { fugue_lifter_runtime::template::Op::IntNotEq },
            O::IntSLess => quote! { fugue_lifter_runtime::template::Op::IntSLess },
            O::IntSLessEq => quote! { fugue_lifter_runtime::template::Op::IntSLessEq },
            O::IntLess => quote! { fugue_lifter_runtime::template::Op::IntLess },
            O::IntLessEq => quote! { fugue_lifter_runtime::template::Op::IntLessEq },
            O::IntZExt => quote! { fugue_lifter_runtime::template::Op::IntZExt },
            O::IntSExt => quote! { fugue_lifter_runtime::template::Op::IntSExt },
            O::IntNeg => quote! { fugue_lifter_runtime::template::Op::IntNeg },
            O::IntNot => quote! { fugue_lifter_runtime::template::Op::IntNot },
            O::IntAdd => quote! { fugue_lifter_runtime::template::Op::IntAdd },
            O::IntSub => quote! { fugue_lifter_runtime::template::Op::IntSub },
            O::IntMul => quote! { fugue_lifter_runtime::template::Op::IntMul },
            O::IntDiv => quote! { fugue_lifter_runtime::template::Op::IntDiv },
            O::IntSDiv => quote! { fugue_lifter_runtime::template::Op::IntSDiv },
            O::IntRem => quote! { fugue_lifter_runtime::template::Op::IntRem },
            O::IntSRem => quote! { fugue_lifter_runtime::template::Op::IntSRem },
            O::IntCarry => quote! { fugue_lifter_runtime::template::Op::IntCarry },
            O::IntSCarry => quote! { fugue_lifter_runtime::template::Op::IntSCarry },
            O::IntSBorrow => quote! { fugue_lifter_runtime::template::Op::IntSBorrow },
            O::IntAnd => quote! { fugue_lifter_runtime::template::Op::IntAnd },
            O::IntOr => quote! { fugue_lifter_runtime::template::Op::IntOr },
            O::IntXor => quote! { fugue_lifter_runtime::template::Op::IntXor },
            O::IntLShift => quote! { fugue_lifter_runtime::template::Op::IntLShift },
            O::IntRShift => quote! { fugue_lifter_runtime::template::Op::IntRShift },
            O::IntSRShift => quote! { fugue_lifter_runtime::template::Op::IntSRShift },
            O::BoolNot => quote! { fugue_lifter_runtime::template::Op::BoolNot },
            O::BoolAnd => quote! { fugue_lifter_runtime::template::Op::BoolAnd },
            O::BoolOr => quote! { fugue_lifter_runtime::template::Op::BoolOr },
            O::BoolXor => quote! { fugue_lifter_runtime::template::Op::BoolXor },
            O::FloatEq => quote! { fugue_lifter_runtime::template::Op::FloatEq },
            O::FloatNotEq => quote! { fugue_lifter_runtime::template::Op::FloatNotEq },
            O::FloatLess => quote! { fugue_lifter_runtime::template::Op::FloatLess },
            O::FloatLessEq => quote! { fugue_lifter_runtime::template::Op::FloatLessEq },
            O::FloatIsNaN => quote! { fugue_lifter_runtime::template::Op::FloatIsNaN },
            O::FloatAdd => quote! { fugue_lifter_runtime::template::Op::FloatAdd },
            O::FloatSub => quote! { fugue_lifter_runtime::template::Op::FloatSub },
            O::FloatMul => quote! { fugue_lifter_runtime::template::Op::FloatMul },
            O::FloatDiv => quote! { fugue_lifter_runtime::template::Op::FloatDiv },
            O::FloatNeg => quote! { fugue_lifter_runtime::template::Op::FloatNeg },
            O::FloatAbs => quote! { fugue_lifter_runtime::template::Op::FloatAbs },
            O::FloatSqrt => quote! { fugue_lifter_runtime::template::Op::FloatSqrt },
            O::FloatOfInt => quote! { fugue_lifter_runtime::template::Op::FloatOfInt },
            O::FloatOfFloat => quote! { fugue_lifter_runtime::template::Op::FloatOfFloat },
            O::FloatTruncate => quote! { fugue_lifter_runtime::template::Op::FloatTruncate },
            O::FloatCeiling => quote! { fugue_lifter_runtime::template::Op::FloatCeiling },
            O::FloatFloor => quote! { fugue_lifter_runtime::template::Op::FloatFloor },
            O::FloatRound => quote! { fugue_lifter_runtime::template::Op::FloatRound },
            O::Build => quote! { fugue_lifter_runtime::template::Op::Build },
            O::DelaySlot => quote! { fugue_lifter_runtime::template::Op::DelaySlot },
            O::Piece => quote! { fugue_lifter_runtime::template::Op::Piece },
            O::Subpiece => quote! { fugue_lifter_runtime::template::Op::Subpiece },
            O::Cast => quote! { fugue_lifter_runtime::template::Op::Cast },
            O::Label => quote! { fugue_lifter_runtime::template::Op::Label },
            O::CrossBuild => quote! { fugue_lifter_runtime::template::Op::CrossBuild },
            O::SegmentOp => quote! { fugue_lifter_runtime::template::Op::SegmentOp },
            O::CPoolRef => quote! { fugue_lifter_runtime::template::Op::CPoolRef },
            O::New => quote! { fugue_lifter_runtime::template::Op::New },
            O::Insert => quote! { fugue_lifter_runtime::template::Op::Insert },
            O::Extract => quote! { fugue_lifter_runtime::template::Op::Extract },
            O::PopCount => quote! { fugue_lifter_runtime::template::Op::PopCount },
            O::LZCount => quote! { fugue_lifter_runtime::template::Op::LZCount },
        };

        let inputs = self.tpl.inputs().iter().map(|tpl| self.wrap(tpl));
        let output = self.tpl.output().map_or_else(
            || quote! { None },
            |tpl| {
                let tpl = self.wrap(tpl);
                quote! { Some(#tpl) }
            },
        );

        let tpl = quote! {
            fugue_lifter_runtime::template::OpTpl {
                op: #op,
                inputs: &[#(#inputs),*],
                output: #output,
            }
        };

        tpl.to_tokens(tokens);
    }
}

impl<'a> ToTokens for TplAdaptor<'a, VarnodeTpl> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let space = self.wrap(self.tpl.space());
        let offset = self.wrap(self.tpl.offset());
        let size = self.wrap(self.tpl.size());

        let tpl = quote! {
            fugue_lifter_runtime::template::VarnodeTpl {
                space: #space,
                offset: #offset,
                size: #size,
            }
        };

        tpl.to_tokens(tokens);
    }
}
