use crate::ir;
use quote::{ToTokens, quote};

pub struct KeyTableEnum<'a> {
    doc: &'a syn::Attribute,
    name: &'a syn::Ident,
    variants: Vec<Variant<'a>>,
}

impl<'a> From<&'a ir::KeyTable> for KeyTableEnum<'a> {
    fn from(table: &'a ir::KeyTable) -> Self {
        let ir::KeyTable {
            doc,
            name,
            keys,
            keys_with_modifiers,
            keys_with_dual_functions,
        } = table;

        let variants = keys
            .iter()
            .map(Variant::from)
            .chain(keys_with_modifiers.iter().map(Variant::from))
            .chain(keys_with_dual_functions.iter().map(Variant::from))
            .collect();

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

struct Variant<'a> {
    meta: VariantMeta<'a>,
    name: &'a syn::Ident,
    code: &'a syn::LitInt,
}

impl<'a> From<&'a ir::Key> for Variant<'a> {
    fn from(key: &'a ir::Key) -> Self {
        let ir::Key { meta, name, code } = key;

        let meta = VariantMeta::from(meta);

        Self { meta, name, code }
    }
}

impl<'a> ToTokens for Variant<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { meta, name, code } = self;

        let token_stream = quote! {
          #meta
          #name = #code
        };

        token_stream.to_tokens(tokens)
    }
}

struct VariantMeta<'a> {
    doc_str: syn::LitStr,
    display_name: &'a syn::LitStr,
}

impl<'a> From<&'a ir::KeyMeta> for VariantMeta<'a> {
    fn from(key: &'a ir::KeyMeta) -> Self {
        let ir::KeyMeta { display_name } = key;

        let doc_str = syn::LitStr::new(&format!(" {}", display_name.value()), display_name.span());

        Self {
            doc_str,
            display_name,
        }
    }
}

impl<'a> ToTokens for VariantMeta<'a> {
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
