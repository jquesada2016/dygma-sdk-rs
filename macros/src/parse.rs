use syn::parse::Parse;

use crate::{
    Ast,
    ast::{Key, KeyTable},
};

impl Parse for Ast {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let tables =
            syn::punctuated::Punctuated::<KeyTable, syn::Token![,]>::parse_terminated(input)?
                .into_iter()
                .collect();

        Ok(Self(tables))
    }
}

impl Parse for KeyTable {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = syn::Attribute::parse_outer(input)?;

        let Some(doc) = attrs
            .extract_if(.., |attr| {
                matches!(
                    &attr.meta,
                    syn::Meta::NameValue(syn::MetaNameValue { path, .. })
                      if *path == syn::parse_quote!(doc)
                )
            })
            .next()
        else {
            abort!(input.span(), "there must be a single doc comment");
        };

        let with_modifiers = attrs
            .extract_if(.., |attr| {
                matches!(
                  &attr.meta,
                  syn::Meta::Path(path)
                    if *path == syn::parse_quote!(with_modifiers)
                )
            })
            .next();

        let with_dual_functions = attrs
            .extract_if(.., |attr| {
                matches!(
                  &attr.meta,
                  syn::Meta::Path(path)
                    if *path == syn::parse_quote!(with_dual_functions)
                )
            })
            .next();

        for attr in attrs {
            emit_error!(attr, "unknown attribute");
        }

        let name = syn::Ident::parse(input)?;

        <syn::Token![:]>::parse(input)?;

        let keys;
        syn::braced!(keys in input);

        let keys = syn::punctuated::Punctuated::<Key, syn::Token![,]>::parse_terminated(&keys)?
            .into_iter()
            .collect();

        Ok(Self {
            doc,
            with_dual_functions,
            with_modifiers,
            name,
            keys,
        })
    }
}

impl Parse for Key {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = syn::Attribute::parse_outer(input)?;

        let doc = attrs
            .iter()
            .find(|attr| {
                matches!(
                    &attr.meta,
                    syn::Meta::NameValue(syn::MetaNameValue { path, .. })
                      if *path == syn::parse_quote!(doc)
                )
            })
            .cloned();

        let name = syn::Ident::parse(input)?;

        let code = Option::<syn::Token![=]>::parse(input)?
            .map(|_| syn::LitInt::parse(input))
            .transpose()?;

        Ok(Self { doc, name, code })
    }
}
