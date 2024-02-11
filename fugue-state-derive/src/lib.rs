extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Ident, Token};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;

use itertools::Itertools;

#[derive(Clone)]
enum Transform {
    Nothing,
    Single(syn::ExprClosure),
    Pair(syn::ExprClosure, syn::ExprClosure),
}

fn parse_transform(t: ParseStream) -> syn::Result<Transform> {
    let first = syn::ExprClosure::parse(t)?;
    let peek = t.lookahead1();
    if peek.peek(Token![,]) {
        let _ = t.parse::<Token![,]>()?;
        Ok(Transform::Pair(first, syn::ExprClosure::parse(t)?))
    } else {
        Ok(Transform::Single(first))
    }
}

#[proc_macro_derive(AsState, attributes(fugue))]
pub fn derive_as_state(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let span = input.span();
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let marked_fields = if let syn::Data::Struct(struc) = input.data {
        let fields = struc.fields;
        let marked = fields.into_iter()
            .enumerate()
            .filter_map(|(i, field)| {
                field.attrs.iter()
                    .find_map(|attr| if attr.path().is_ident("fugue") {
                        if attr.meta.require_list().is_ok_and(|m| !m.tokens.is_empty()) {
                            let attr_map = attr.parse_args_with(parse_transform);
                            Some(attr_map.map(|map| (i, map, field.clone())))
                        } else {
                            Some(Ok((i, Transform::Nothing, field.clone())))
                        }
                    } else {
                        None
                    })
            })
            .collect::<syn::Result<Vec<_>>>();

        match marked {
            Ok(marked) => marked,
            Err(e) => return e.into_compile_error().into(),
        }
    } else {
        return syn::Error::new(span, "only structs are supported")
            .into_compile_error()
            .into()
    };

    let marked_powerset = marked_fields.into_iter()
        .powerset()
        .filter(|v| !v.is_empty());

    marked_powerset.into_iter().flat_map(|v| { let n = v.len(); v.into_iter().permutations(n).collect::<Vec<_>>() }).map(|fields| {
        let types = fields.iter().map(|(_, _, f)| f.ty.clone()).collect::<Vec<_>>();
        let (type_tokens, refs, muts) = if types.len() == 1 {
            let ty = &types[0];
            (
                quote! { #ty },
                quote! { &#ty },
                quote! { &mut #ty },
            )
        } else {
            (
                quote! { #(#types),* },
                quote! { (#(&#types),*) },
                quote! { (#(&mut #types),*) },
            )
        };

        let (accessors, accessors_mut) = fields.iter()
            .map(|(i, ff, f)| if let Some(ref ident) = f.ident {
                match ff {
                    Transform::Nothing => (quote! { &self.#ident }, quote! { &mut self.#ident }),
                    Transform::Single(ff) => (quote! { (#ff)(&self.#ident) }, quote! { (#ff)(&mut self.#ident) }),
                    Transform::Pair(ffr, ffm) => (quote! { (#ffr)(&self.#ident) }, quote! { (#ffm)(&mut self.#ident) }),
                }
            } else {
                match ff {
                    Transform::Nothing => (quote! { &self.#i }, quote! { &mut self.#i }),
                    Transform::Single(ff) => (quote! { (#ff)(&self.#i) }, quote! { (#ff)(&mut self.#i) }),
                    Transform::Pair(ffr, ffm) => (quote! { (#ffr)(&self.#i) }, quote! { (#ffm)(&mut self.#i) }),
                }
            })
            .unzip::<_, _, Vec<_>, Vec<_>>();

        let arity = types.len();
        let impl_fn = if arity > 1 { Ident::new(&format!("state{}_ref", arity), span) } else { Ident::new("state_ref", span) };
        let impl_fn_mut = if arity > 1 { Ident::new(&format!("state{}_mut", arity), span) } else { Ident::new("state_mut", span) };
        let impl_trait = if arity > 1 { Ident::new(&format!("AsState{}", types.len()), span) } else { Ident::new("AsState", span) };

        quote! {
            impl #impl_generics ::fugue_state::#impl_trait<#type_tokens> for #ident #ty_generics #where_clause {
                fn #impl_fn(&self) -> #refs {
                    (#(#accessors),*)
                }

                fn #impl_fn_mut(&mut self) -> #muts {
                    (#(#accessors_mut),*)
                }
            }
        }
    })
    .collect::<TokenStream2>()
    .into()
}
