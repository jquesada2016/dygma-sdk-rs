use crate::{
    Ast, ast,
    codegen::{DualFunctionModifier, Modifier, lit_int_to_u16},
};
use quote::{ToTokens, quote};

pub struct ImplFromU16ForKeyKind<'a> {
    tables: Vec<KeyTable<'a>>,
}

impl<'a> From<&'a Ast> for ImplFromU16ForKeyKind<'a> {
    fn from(value: &'a Ast) -> Self {
        let tables = value.0.iter().map(KeyTable::from).collect();

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
    keys: Vec<Key>,
}

impl<'a> From<&'a ast::KeyTable> for KeyTable<'a> {
    fn from(table: &'a ast::KeyTable) -> Self {
        let ast::KeyTable {
            doc: _,
            with_modifiers,
            with_dual_functions,
            name,
            keys,
        } = table;

        let powerset = Modifier::powerset();

        let mut normalized_keys = Vec::with_capacity(
            keys.len()
                * (if with_modifiers.is_some() {
                    powerset.len()
                } else {
                    1
                } + if with_dual_functions.is_some() {
                    DualFunctionModifier::VALUES.len()
                } else {
                    1
                }),
        );

        let mut offset = 0u16;

        for key in keys {
            let ast::Key { doc: _, name, code } = key;

            offset = if let Some(code) = code {
                lit_int_to_u16(code)
            } else if let Some(offset) = offset.checked_add(1) {
                offset
            } else {
                continue;
            };

            let match_arm_literal = if let Some(code) = code {
                syn::LitInt::new(&offset.to_string(), code.span())
            } else {
                syn::LitInt::new(&offset.to_string(), name.span())
            };

            normalized_keys.push(Key {
                name: name.to_owned(),
                match_arm_literal: match_arm_literal.clone(),
            });

            if with_modifiers.is_some() {
                normalized_keys.extend(keys_from_modifiers(key, &powerset, offset));
            }

            if with_dual_functions.is_some() {
                let dual_keys =
                    DualFunctionModifier::VALUES
                        .iter()
                        .copied()
                        .filter_map(|modifier| {
                            let name = quote::format_ident!("Dual{modifier:?}{name}");

                            let code_u16 = lit_int_to_u16(&match_arm_literal)
                                .checked_add(modifier.as_modifier_value())?;

                            let match_arm_literal =
                                syn::LitInt::new(&code_u16.to_string(), match_arm_literal.span());

                            Some(Key {
                                name,
                                match_arm_literal,
                            })
                        });

                normalized_keys.extend(dual_keys);
            }
        }

        Self {
            name,
            keys: normalized_keys,
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

struct Key {
    name: syn::Ident,
    match_arm_literal: syn::LitInt,
}

fn keys_from_modifiers(key: &ast::Key, powerset: &[Vec<Modifier>], offset: u16) -> Vec<Key> {
    let ast::Key { doc: _, name, code } = key;

    powerset
        .iter()
        .filter(|set| !set.is_empty())
        .filter_map(|modifiers| {
            let modifier_prefix = modifiers
                .iter()
                .map(ToString::to_string)
                .collect::<String>();

            let name = quote::format_ident!("{modifier_prefix}{name}");

            let modifier_value = modifiers
                .iter()
                .copied()
                .map(|modifier| modifier.as_modifier_value())
                .fold(0, std::ops::Add::add);

            let match_arm_u16 = offset.checked_add(modifier_value)?;

            let match_arm_literal = syn::LitInt::new(
                &match_arm_u16.to_string(),
                code.as_ref()
                    .map(|code| code.span())
                    .unwrap_or_else(|| name.span()),
            );

            Some(Key {
                name,
                match_arm_literal,
            })
        })
        .collect()
}
