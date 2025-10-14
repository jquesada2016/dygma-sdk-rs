use quote::{ToTokens, quote};

use crate::{Ir, ir};

pub struct ImplFromKeyTableEnumForU16<'a> {
    name: &'a syn::Ident,
}

impl<'a> From<&'a ir::KeyTable> for ImplFromKeyTableEnumForU16<'a> {
    fn from(table: &'a ir::KeyTable) -> Self {
        let ir::KeyTable {
            doc: _,
            with_modifiers: _,
            with_dual_functions: _,
            name,
            keys: _,
        } = table;

        Self { name }
    }
}

impl<'a> ToTokens for ImplFromKeyTableEnumForU16<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { name } = self;

        let token_stream = quote! {
          impl From<[<#name:camel>]> for u16 {
            fn from(kind: [<#name:camel>]) -> Self {
              kind as u16
            }
          }
        };

        token_stream.to_tokens(tokens)
    }
}

pub struct ImplFromKeyKindEnumForU16<'a> {
    match_arms: Vec<KeyKindMatchArm<'a>>,
}

impl<'a> From<&'a Ir> for ImplFromKeyKindEnumForU16<'a> {
    fn from(it: &'a Ir) -> Self {
        let match_arms = it.0.iter().map(KeyKindMatchArm::from).collect();

        Self { match_arms }
    }
}

impl<'a> ToTokens for ImplFromKeyKindEnumForU16<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { match_arms } = self;

        let token_stream = quote! {
          impl From<KeyKind> for u16 {
            fn from(kind: KeyKind) -> Self {
              match kind {
                #( #match_arms ),*,
                KeyKind::Unknown(other) => other
              }
            }
          }
        };

        token_stream.to_tokens(tokens);
    }
}

struct KeyKindMatchArm<'a> {
    name: &'a syn::Ident,
}

impl<'a> From<&'a ir::KeyTable> for KeyKindMatchArm<'a> {
    fn from(table: &'a ir::KeyTable) -> Self {
        let ir::KeyTable {
            doc: _,
            with_modifiers: _,
            with_dual_functions: _,
            name,
            keys: _,
        } = table;

        Self { name }
    }
}

impl<'a> ToTokens for KeyKindMatchArm<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { name } = self;

        let token_stream = quote! {
          KeyKind::[<#name:camel>](key) => key.into()
        };

        token_stream.to_tokens(tokens);
    }
}
