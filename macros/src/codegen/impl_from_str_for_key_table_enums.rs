use crate::ir;
use quote::{ToTokens, quote};

pub struct ImplFromStrForKeyTableEnum<'a> {
    name: &'a syn::Ident,
    variants: Vec<Variant<'a>>,
}

impl<'a> From<&'a ir::KeyTable> for ImplFromStrForKeyTableEnum<'a> {
    fn from(table: &'a ir::KeyTable) -> Self {
        let ir::KeyTable {
            doc: _,
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

        Self { name, variants }
    }
}

impl<'a> ToTokens for ImplFromStrForKeyTableEnum<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { name, variants } = self;

        let token_stream = quote! {
            impl FromStr for [<#name:camel>] {
                type Err = FromStrError;

                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    let s = s
                        .trim()
                        .chars()
                        .filter(|c| *c != ' ')
                        .map(|c| c.to_ascii_lowercase())
                        .collect::<String>();

                    match s.as_str() {
                        #( #variants ),*,
                        _ => Err(FromStrError)
                    }
                }
            }
        };

        token_stream.to_tokens(tokens);
    }
}

struct Variant<'a> {
    name: &'a syn::Ident,
    matchers: Vec<syn::LitStr>,
}

impl<'a> From<&'a ir::Key> for Variant<'a> {
    fn from(key: &'a ir::Key) -> Self {
        let ir::Key {
            meta: ir::KeyMeta { display_name },
            name,
            code: _,
        } = key;

        let display_name_matcher = syn::LitStr::new(
            &display_name
                .value()
                .chars()
                .filter(|c| *c != ' ')
                .map(|c| c.to_ascii_lowercase())
                .collect::<String>(),
            display_name.span(),
        );

        let debug_name_matcher = syn::LitStr::new(
            &name
                .to_string()
                .chars()
                .map(|c| c.to_ascii_lowercase())
                .collect::<String>(),
            name.span(),
        );

        let matchers = if display_name_matcher.value() == debug_name_matcher.value() {
            vec![debug_name_matcher]
        } else {
            vec![display_name_matcher, debug_name_matcher]
        };

        Self { name, matchers }
    }
}

impl<'a> ToTokens for Variant<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { name, matchers } = self;

        let match_arms = matchers.iter().map(|matcher| {
            quote! { #matcher => Ok(Self::#name) }
        });

        let token_stream = quote! {
            #( #match_arms ),*
        };

        token_stream.to_tokens(tokens);
    }
}
