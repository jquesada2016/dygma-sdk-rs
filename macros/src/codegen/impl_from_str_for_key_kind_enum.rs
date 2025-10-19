use crate::{Ir, ir};
use quote::{ToTokens, quote};

pub struct ImplFromStrForKeyKindEnum<'a> {
    variants: Vec<Variant<'a>>,
}

impl<'a> From<&'a Ir> for ImplFromStrForKeyKindEnum<'a> {
    fn from(ir: &'a Ir) -> Self {
        let variants = ir.0.iter().map(Variant::from).collect();

        Self { variants }
    }
}

impl<'a> ToTokens for ImplFromStrForKeyKindEnum<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { variants } = self;

        let token_stream = quote! {
          impl FromStr for KeyKind {
            type Err = FromStrError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
              Err(FromStrError)
                #( #variants )*
            }
          }
        };

        token_stream.to_tokens(tokens)
    }
}

struct Variant<'a> {
    name: &'a syn::Ident,
}

impl<'a> From<&'a ir::KeyTable> for Variant<'a> {
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

impl<'a> ToTokens for Variant<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { name } = self;

        let token_stream = quote! {
          .or_else(|_| s.parse::<[<#name:camel>]>().map(Self::[<#name:camel>]))
        };

        token_stream.to_tokens(tokens);
    }
}
