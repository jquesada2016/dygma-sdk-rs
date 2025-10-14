use crate::{
    ast,
    codegen::{DualFunctionModifier, KeyCodeOverflowsU16Error, Modifier, lit_int_to_u16},
};
use itertools::Itertools;
use quote::{ToTokens, quote};

pub struct KeyTableEnum<'a> {
    doc: &'a syn::Attribute,
    name: &'a syn::Ident,
    variants: Vec<Variant>,
}

impl<'a> From<&'a ast::KeyTable> for KeyTableEnum<'a> {
    fn from(table: &'a ast::KeyTable) -> Self {
        let ast::KeyTable {
            doc,
            with_modifiers,
            with_dual_functions,
            name,
            keys,
        } = table;

        let mut offset = 0u16;

        let update_offset = |offset: u16, key: &ast::Key| {
            key.code
                .as_ref()
                .map(lit_int_to_u16)
                .map(Ok)
                .unwrap_or_else(|| offset.checked_add(1).ok_or(KeyCodeOverflowsU16Error))
        };

        let mut variants = if with_modifiers.is_some() {
            let powerset = Modifier::powerset();

            powerset
                .into_iter()
                .flat_map(|modifier_set| {
                    keys.iter().filter_map(move |key| {
                        offset = update_offset(offset, key).ok()?;

                        Variant::new_with_modifiers(key, modifier_set.clone(), offset).ok()
                    })
                })
                .collect::<Vec<_>>()
        } else {
            keys.iter()
                .filter_map(move |key| {
                    offset = update_offset(offset, key).ok()?;

                    Some(Variant::new(key, offset))
                })
                .collect()
        };

        let mut offset = 0u16;

        if with_dual_functions.is_some() {
            let dual_variants = DualFunctionModifier::VALUES
                .iter()
                .copied()
                .flat_map(|modifier| {
                    keys.iter().filter_map(move |key| {
                        offset = update_offset(offset, key).ok()?;

                        Variant::new_with_dual_functions(key, modifier, offset).ok()
                    })
                });

            variants.extend(dual_variants)
        };

        Self {
            doc,
            name,
            variants,
        }
    }
}

impl<'a> ToTokens for KeyTableEnum<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            doc,
            name,
            variants,
        } = self;

        let token_stream = quote! {
          #doc
          #[derive(Clone, Copy, Debug, Display, Hash, PartialEq, Eq, PartialOrd, Ord)]
          #[repr(u16)]
          pub enum [<#name:camel>] {
            #( #variants ),*
          }
        };

        token_stream.to_tokens(tokens);
    }
}

struct Variant {
    meta: VariantMeta,
    name: syn::Ident,
    code: syn::LitInt,
}

impl ToTokens for Variant {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { meta, name, code } = self;

        let token_stream = quote! {
          #meta
          #name = #code
        };

        token_stream.to_tokens(tokens)
    }
}

impl Variant {
    fn new(key: &ast::Key, offset: u16) -> Self {
        let ast::Key { doc: _, name, code } = key;

        let meta = VariantMeta::from(key);

        let name = name.to_owned();

        let code = code
            .clone()
            .unwrap_or_else(|| syn::LitInt::new(&offset.to_string(), name.span()));

        Self { meta, name, code }
    }

    fn new_with_modifiers(
        key: &ast::Key,
        modifiers: Vec<Modifier>,
        offset: u16,
    ) -> Result<Self, KeyCodeOverflowsU16Error> {
        let ast::Key { doc: _, name, code } = key;

        if modifiers.is_empty() {
            return Ok(Self::new(key, offset));
        }

        let meta = VariantMeta::new_with_modifiers(key, &modifiers);

        let modifier_prefix = modifiers
            .iter()
            .map(|modifier| modifier.to_string())
            .collect::<String>();

        let name = quote::format_ident!("{modifier_prefix}{name}");

        let modifier_code_value = modifiers
            .iter()
            .copied()
            .map(Modifier::as_modifier_value)
            .fold(0, std::ops::Add::add);

        let code = code
            .clone()
            .unwrap_or_else(|| syn::LitInt::new(&offset.to_string(), name.span()));

        let code_u16 = lit_int_to_u16(&code);

        let code_u16 = code_u16
            .checked_add(modifier_code_value)
            .ok_or(KeyCodeOverflowsU16Error)?;

        let code = syn::LitInt::new(&code_u16.to_string(), code.span());

        Ok(Self { meta, name, code })
    }

    fn new_with_dual_functions(
        key: &ast::Key,
        modifier: DualFunctionModifier,
        offset: u16,
    ) -> Result<Self, KeyCodeOverflowsU16Error> {
        let ast::Key { doc: _, name, code } = key;

        let meta = VariantMeta::new_with_dual_functions(key, modifier);

        let name = quote::format_ident!("Dual{modifier:?}{name}");

        let code = code
            .clone()
            .unwrap_or_else(|| syn::LitInt::new(&offset.to_string(), name.span()));

        let code_u16 = lit_int_to_u16(&code);
        let modifier_value = modifier.as_modifier_value();

        let code_u16 = code_u16
            .checked_add(modifier_value)
            .ok_or(KeyCodeOverflowsU16Error)?;

        let code = syn::LitInt::new(&code_u16.to_string(), code.span());

        Ok(Self { meta, name, code })
    }
}

struct VariantMeta {
    doc_str: syn::LitStr,
    display_name: syn::LitStr,
}

impl From<&ast::Key> for VariantMeta {
    fn from(key: &ast::Key) -> Self {
        let ast::Key { doc, name, code: _ } = key;

        let display_name = if let Some(lit_str) = doc.as_ref().and_then(attribute_to_lit_str) {
            syn::LitStr::new(lit_str.value().trim(), lit_str.span())
        } else {
            syn::LitStr::new(&name.to_string(), name.span())
        };

        let doc_str = doc
            .as_ref()
            .and_then(attribute_to_lit_str)
            .cloned()
            .unwrap_or_else(|| syn::LitStr::new(&format!(" {name}"), name.span()));

        Self {
            doc_str,
            display_name,
        }
    }
}

impl ToTokens for VariantMeta {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            doc_str,
            display_name,
        } = self;

        let token_stream = quote! {
          #[doc = #doc_str]
          #[display(#display_name)]
        };

        token_stream.to_tokens(tokens);
    }
}

impl VariantMeta {
    fn new_with_modifiers(key: &ast::Key, modifiers: &[Modifier]) -> Self {
        let ast::Key { doc, name, code: _ } = key;

        let modifier_prefix = modifiers
            .iter()
            .map(|modifier| modifier.to_string())
            .join(" + ");

        let display_name = if let Some(lit_str) = doc.as_ref().and_then(attribute_to_lit_str) {
            syn::LitStr::new(
                &format!("{modifier_prefix} + {}", lit_str.value().trim()),
                lit_str.span(),
            )
        } else {
            syn::LitStr::new(&format!("{modifier_prefix} + {name}"), name.span())
        };

        let doc_str = syn::LitStr::new(&format!(" {}", display_name.value()), display_name.span());

        Self {
            doc_str,
            display_name,
        }
    }

    fn new_with_dual_functions(key: &ast::Key, modifier: DualFunctionModifier) -> Self {
        let ast::Key { doc, name, code: _ } = key;

        let display_name = if let Some(lit_str) = doc.as_ref().and_then(attribute_to_lit_str) {
            syn::LitStr::new(
                &format!("{} / {modifier}", lit_str.value().trim()),
                lit_str.span(),
            )
        } else {
            syn::LitStr::new(&format!("{name} / {modifier}"), name.span())
        };

        let doc_str = syn::LitStr::new(&format!(" {}", display_name.value()), display_name.span());

        Self {
            doc_str,
            display_name,
        }
    }
}

fn attribute_to_lit_str(attr: &syn::Attribute) -> Option<&syn::LitStr> {
    if let syn::Attribute {
        meta:
            syn::Meta::NameValue(syn::MetaNameValue {
                value:
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(lit_str),
                        ..
                    }),
                ..
            }),
        ..
    } = attr
    {
        Some(lit_str)
    } else {
        None
    }
}
