use crate::{Ir, ir};
use quote::{ToTokens, quote};

pub struct ImplFromU16ForKeyKind<'a> {
    tables: Vec<KeyTable<'a>>,
}

impl<'a> From<&'a Ir> for ImplFromU16ForKeyKind<'a> {
    fn from(ir: &'a Ir) -> Self {
        let tables = ir.0.iter().map(KeyTable::from).collect();

        Self { tables }
    }
}

impl<'a> ToTokens for ImplFromU16ForKeyKind<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { tables } = self;

        let token_stream = quote! {
          impl From<u16> for KeyKind {
            fn from(code: u16) -> Self {
              match code {
                #( #tables ),*,
                other => Self::Unknown(other)
              }
            }
          }
        };

        token_stream.to_tokens(tokens)
    }
}

struct KeyTable<'a> {
    name: &'a syn::Ident,
    keys: Vec<Key<'a>>,
}

impl<'a> From<&'a ir::KeyTable> for KeyTable<'a> {
    fn from(table: &'a ir::KeyTable) -> Self {
        let ir::KeyTable {
            doc: _,
            name,
            keys,
            keys_with_modifiers,
            keys_with_dual_functions,
        } = table;

        let all_keys = keys
            .iter()
            .map(Key::from)
            .chain(keys_with_modifiers.iter().map(Key::from))
            .chain(keys_with_dual_functions.iter().map(Key::from))
            .collect();

        Self {
            name,
            keys: all_keys,
        }
    }
}

impl<'a> ToTokens for KeyTable<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            name: table_name,
            keys,
        } = self;

        let match_arms = keys.iter().map(|key| {
            let Key {
                name: key_name,
                match_arm_literal,
            } = key;

            quote! {
              #match_arm_literal => Self::[<#table_name:camel>]([<#table_name:camel>]::#key_name)
            }
        });

        let token_stream = quote! {
          #( #match_arms ),*
        };

        token_stream.to_tokens(tokens)
    }
}

struct Key<'a> {
    name: &'a syn::Ident,
    match_arm_literal: &'a syn::LitInt,
}

impl<'a> From<&'a ir::Key> for Key<'a> {
    fn from(key: &'a ir::Key) -> Self {
        let ir::Key {
            meta: _,
            name,
            code,
        } = key;

        let match_arm_literal = code;

        Key {
            name,
            match_arm_literal,
        }
    }
}
