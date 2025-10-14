use crate::{Ast, ast};
use quote::{ToTokens, quote};

pub struct KeyKindEnum<'a> {
    variants: Vec<Variant<'a>>,
}

impl<'a> From<&'a Ast> for KeyKindEnum<'a> {
    fn from(ast: &'a Ast) -> Self {
        let variants = ast.0.iter().map(Variant::from).collect();

        Self { variants }
    }
}

impl<'a> ToTokens for KeyKindEnum<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { variants } = self;

        let token_stream = quote! {
          /// Represents all possible keys in a keymap.
          #[derive(Clone, Copy, Debug, Display, From, Hash, PartialEq, Eq, PartialOrd, Ord)]
          #[display("{_0}")]
          pub enum KeyKind {
            #( #variants ),*,
            /// An unknown key.
            #[display("<unknown {_0}>")]
            #[from(ignore)]
            Unknown(u16),
          }
        };

        token_stream.to_tokens(tokens);
    }
}

struct Variant<'a> {
    doc: &'a syn::Attribute,
    name: &'a syn::Ident,
}

impl<'a> From<&'a ast::KeyTable> for Variant<'a> {
    fn from(table: &'a ast::KeyTable) -> Self {
        let ast::KeyTable {
            doc,
            with_modifiers: _,
            with_dual_functions: _,
            name,
            keys: _,
        } = table;

        Self { doc, name }
    }
}

impl<'a> ToTokens for Variant<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { doc, name } = self;

        let token_stream = quote! {
          #doc
          [<#name:camel>]([<#name:camel>])
        };

        token_stream.to_tokens(tokens);
    }
}
