// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The derive macro of `coqui-builder`, reached through that crate's `derive` feature.
//!
//! `#[derive(Describe)]` writes everything a settings type needs from one declaration:
//! `Describe` (what each key is), the fill that a `Partial` holds, `Inspect` (reading a key
//! of a value), and `Fields` for a struct of named fields.
//!
//! | declaration | setting |
//! | --- | --- |
//! | struct with named fields | a group, one key per field |
//! | newtype struct, or `#[map(transparent)]` | the field's own setting |
//! | enum of unit variants | a choice of names (`Leaf`) |
//! | enum with `#[map(tag = "k")]` | a group whose fields depend on the arm the tag names |
//! | enum with `#[map(untagged)]` | a union told apart by the kind of value (`Leaf`) |
//!
//! A tuple struct of two or more fields, a unit struct, a union, a generic enum and a type that
//! holds itself are refused with a sentence that says what to do instead. The spellings are
//! the ones `guatiao`'s derives read, so a type can move between the two; the vectors in
//! `tests/vectors/attributes.json` hold both to the same meaning.

#![forbid(unsafe_code)]

mod attrs;
mod enums;
mod fields;
mod rename;
mod structs;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, GenericParam};

use crate::attrs::ContainerAttrs;
use crate::structs::Ctx;

/// Describes a settings type: see the crate documentation of `coqui-builder`.
#[proc_macro_derive(Describe, attributes(map, schema, builder, leaf))]
pub fn describe(input: TokenStream) -> TokenStream {
    expand(input.into()).into()
}

/// Makes a type a `Leaf`, one setting that is one text: see the crate documentation of
/// `coqui-builder`.
///
/// An enum of unit variants reads and writes its variant names (`rename`, `rename_all`);
/// any other type reads and writes its `FromStr` and `Display`. `#[leaf(name = "..")]` names
/// the type a datum shows, `#[leaf(ordered)]` takes its order from `PartialOrd`, and
/// `#[leaf(display)]` makes a unit-only enum use `Display` and `FromStr` as well.
#[proc_macro_derive(Leaf, attributes(leaf, map, schema))]
pub fn leaf(input: TokenStream) -> TokenStream {
    match try_leaf(input.into()) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
    .into()
}

fn try_leaf(input: TokenStream2) -> syn::Result<TokenStream2> {
    let ast: DeriveInput = syn::parse2(input)?;
    let mut name = ast.ident.to_string();
    let (mut ordered, mut display) = (false, false);
    for attr in ast.attrs.iter().filter(|a| a.path().is_ident("leaf")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                name = meta.value()?.parse::<syn::LitStr>()?.value();
            } else if meta.path.is_ident("ordered") {
                ordered = true;
            } else if meta.path.is_ident("display") {
                display = true;
            } else {
                return Err(meta.error(
                    "unrecognised `#[leaf(...)]` option. `#[derive(Leaf)]` knows `name = \"..\"`, \
                     `ordered` and `display`",
                ));
            }
            Ok(())
        })?;
    }
    let container = ContainerAttrs::read(&ast.attrs, matches!(ast.data, Data::Enum(_)))?;
    let ident = &ast.ident;
    let all_units = matches!(&ast.data, Data::Enum(data) if data.variants.iter().all(|v| {
        matches!(v.fields, syn::Fields::Unit)
            || v.attrs.iter().any(|a| a.path().is_ident("map"))
    }));
    let order = ordered.then(|| {
        quote! {
            const ORDERED: bool = true;
            fn order(
                a: &Self,
                b: &Self,
            ) -> ::core::option::Option<::core::cmp::Ordering> {
                ::core::cmp::PartialOrd::partial_cmp(a, b)
            }
        }
    });
    let body = match &ast.data {
        Data::Enum(data) if all_units && !display => enums::unit_enum(
            ident,
            &name,
            &container,
            data,
            container.doc.as_ref(),
            order.unwrap_or_default(),
        )?,
        _ => {
            let (impl_g, ty_g, where_g) = ast.generics.split_for_impl();
            quote! {
                #[automatically_derived]
                impl #impl_g __cb::Leaf for #ident #ty_g #where_g {
                    const TYPE: &'static str = #name;
                    #order
                    fn to_text(&self) -> __cb::__private::String {
                        __cb::__private::ToString::to_string(self)
                    }
                    fn from_text(
                        text: &str,
                    ) -> ::core::result::Result<Self, __cb::describe::LeafError> {
                        text.parse::<Self>().map_err(|_| __cb::describe::LeafError::Invalid {
                            type_name: #name,
                        })
                    }
                }
            }
        }
    };
    let krate = container
        .krate
        .as_ref()
        .map_or_else(|| quote!(::coqui_builder), |path| quote!(#path));
    Ok(quote! {
        #[allow(non_camel_case_types, unused_qualifications, unused_variables, dead_code, clippy::all)]
        const _: () = {
            use #krate as __cb;
            #body
        };
    })
}

/// The expansion, over [`proc_macro2::TokenStream`] so a `#[test]` can call it.
pub(crate) fn expand(input: TokenStream2) -> TokenStream2 {
    match try_expand(input) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn try_expand(input: TokenStream2) -> syn::Result<TokenStream2> {
    let ast: DeriveInput = syn::parse2(input)?;
    for param in &ast.generics.params {
        match param {
            GenericParam::Type(_) => {}
            GenericParam::Lifetime(l) => {
                return Err(syn::Error::new_spanned(
                    l,
                    "`#[derive(Describe)]` describes settings that own their data: a type \
                     with a lifetime parameter borrows from somewhere else.",
                ));
            }
            GenericParam::Const(c) => {
                return Err(syn::Error::new_spanned(
                    c,
                    "`#[derive(Describe)]` does not take a const generic parameter: a key \
                     cannot depend on one.",
                ));
            }
        }
    }
    let is_enum = matches!(ast.data, Data::Enum(_));
    let container = ContainerAttrs::read(&ast.attrs, is_enum)?;
    let ctx = Ctx {
        name: &ast.ident,
        generics: &ast.generics,
        container: &container,
    };
    let doc = container.doc.as_ref();
    let body = match &ast.data {
        Data::Struct(data) => structs::expand(&ctx, &data.fields, doc)?,
        Data::Enum(data) => enums::expand(&ctx, data, doc)?,
        Data::Union(data) => {
            return Err(syn::Error::new_spanned(
                data.union_token,
                "`#[derive(Describe)]` supports structs with named fields and enums. A union \
                 has no readable field: which arm is live is not knowable from the type.",
            ));
        }
    };
    let krate = container
        .krate
        .as_ref()
        .map_or_else(|| quote!(::coqui_builder), |path| quote!(#path));
    Ok(quote! {
        #[allow(
            non_camel_case_types,
            unused_qualifications,
            unused_mut,
            unused_variables,
            dead_code,
            clippy::all
        )]
        const _: () = {
            use #krate as __cb;
            #body
        };
    })
}

#[cfg(test)]
mod tests;
