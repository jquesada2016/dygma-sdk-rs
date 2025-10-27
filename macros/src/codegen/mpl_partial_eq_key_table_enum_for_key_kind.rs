use quote::{ToTokens, quote};

use crate::ir;

pub struct ImplPartialEqKeyTableForKeyKind<'a> {
    name: &'a syn::Ident,
}

impl<'a> From<&'a ir::KeyTable> for ImplPartialEqKeyTableForKeyKind<'a> {
    fn from(table: &'a ir::KeyTable) -> Self {
        let ir::KeyTable {
            doc: _,
            name,
            keys: _,
            keys_with_modifiers: _,
            keys_with_dual_functions: _,
        } = table;

        Self { name }
    }
}

impl<'a> ToTokens for ImplPartialEqKeyTableForKeyKind<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { name } = self;

        let token_stream = quote! {
          impl PartialEq<[<#name:camel>]> for KeyKind {
            fn eq(&self, other: &[<#name:camel>]) -> bool {
              if let Self::[<#name:camel>](key) = self {
                *key == *other
              } else {
                false              }
            }
          }

          impl PartialEq<KeyKind> for [<#name:camel>] {
            fn eq(&self, other: &KeyKind) -> bool {
              *other == *self
            }
          }
        };

        token_stream.to_tokens(tokens);
    }
}
