use syn::parse::{Parse, Parser};
use syn::spanned::Spanned as _;

pub struct Ast(pub Vec<KeyTable>);

impl Parse for Ast {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let tables =
            syn::punctuated::Punctuated::<KeyTable, syn::Token![,]>::parse_terminated(input)?
                .into_iter()
                .collect();

        Ok(Self(tables))
    }
}

pub struct KeyTable {
    pub doc: syn::Attribute,
    pub with_modifiers: Option<syn::Attribute>,
    pub with_dual_functions: Option<syn::Attribute>,
    pub name: syn::Ident,
    pub keys: Vec<Key>,
}

impl Parse for KeyTable {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = syn::Attribute::parse_outer(input)?;

        // A table's doc comment is public rustdoc on the generated
        // enums, so multiple `///` lines are joined into one attribute
        // rather than silently dropping the extras.
        let doc_attrs = attrs
            .extract_if(.., |attr| is_doc_attr(attr))
            .map(|attr| {
                let value = match &attr.meta {
                    syn::Meta::NameValue(syn::MetaNameValue {
                        value:
                            syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Str(doc),
                                ..
                            }),
                        ..
                    }) => doc.value(),
                    _ => abort!(attr, "doc comment did not have expected shape"),
                };

                (attr, value)
            })
            .collect::<Vec<_>>();

        let Some((first_doc, _)) = doc_attrs.first() else {
            abort!(input.span(), "there must be a doc comment");
        };

        let doc = if doc_attrs.len() == 1 {
            first_doc.clone()
        } else {
            let joined = doc_attrs
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>()
                .join("\n");

            let doc: syn::Attribute = syn::parse_quote! { #[doc = #joined] };

            doc
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

/// Whether `attr` is a doc comment.
fn is_doc_attr(attr: &syn::Attribute) -> bool {
    matches!(
        &attr.meta,
        syn::Meta::NameValue(syn::MetaNameValue { path, .. })
          if *path == syn::parse_quote!(doc)
    )
}

pub struct Key {
    pub doc: Option<syn::Attribute>,
    /// Modifier combos whose generated expansions must not exist for
    /// this key, one attribute per combo (`#[drop_modifier_combos(Ctrl +
    /// Shift)]`); the modifier names are resolved to
    /// [`crate::ir::Modifier`] values in `ir`.
    pub modifier_combo_drops: Vec<Vec<syn::Ident>>,
    pub name: syn::Ident,
    pub code: Option<syn::LitInt>,
}

impl Parse for Key {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = syn::Attribute::parse_outer(input)?;

        // A key's doc comment IS its display name, so at most one is
        // allowed — a second is an error, not a silent overwrite.
        let mut docs = attrs
            .extract_if(.., |attr| is_doc_attr(attr))
            .collect::<Vec<_>>();

        let doc = match docs.len() {
            0 => None,
            1 => Some(docs.remove(0)),
            _ => {
                return Err(syn::Error::new_spanned(
                    docs.remove(1),
                    "multiple doc comments on one key; a key has exactly one, used as its display name",
                ));
            }
        };

        let modifier_combo_drops = attrs
            .extract_if(.., |attr| {
                matches!(
                    &attr.meta,
                    syn::Meta::List(list)
                      if list.path.is_ident("drop_modifier_combos")
                )
            })
            .map(|attr| match attr.meta {
                syn::Meta::List(list) => {
                    // Capture the span before `parse2` consumes the
                    // attribute's tokens.
                    let list_span = list.span();

                    let punct =
                        syn::punctuated::Punctuated::<syn::Ident, syn::Token![+]>::parse_terminated
                            .parse2(list.tokens)?;

                    // `parse_terminated` accepts a trailing `+`, which
                    // would otherwise silently mean nothing.
                    if punct
                        .pairs()
                        .next_back()
                        .is_some_and(|pair| pair.punct().is_some())
                    {
                        return Err(syn::Error::new(
                            list_span,
                            "trailing `+` in `drop_modifier_combos`",
                        ));
                    }

                    // Rejected here (not only in `ir`) so the diagnostic
                    // points at the offending attribute.
                    if punct.is_empty() {
                        return Err(syn::Error::new(
                            list_span,
                            "empty `drop_modifier_combos` attribute; list at least one modifier: Ctrl, Alt, AltGr, Os, Shift",
                        ));
                    }

                    Ok(punct.into_iter().collect())
                }
                _ => unreachable!("extract_if only kept `drop_modifier_combos` lists"),
            })
            .collect::<syn::Result<Vec<_>>>()?;

        // Anything left over is an error rather than a silently ignored
        // typo.
        if let Some(attr) = attrs.into_iter().next() {
            return Err(syn::Error::new_spanned(
                attr,
                "unknown key attribute; expected a doc comment or `#[drop_modifier_combos(...)]`",
            ));
        }

        let name = syn::Ident::parse(input)?;

        let code = Option::<syn::Token![=]>::parse(input)?
            .map(|_| syn::LitInt::parse(input))
            .transpose()?;

        Ok(Self {
            doc,
            modifier_combo_drops,
            name,
            code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Key;
    use syn::parse::{Parse, Parser};

    fn parse_key(s: &str) -> syn::Result<Key> {
        Key::parse.parse_str(s)
    }

    fn parse_key_error(s: &str) -> syn::Error {
        parse_key(s)
            .err()
            .unwrap_or_else(|| panic!("`{s}` should not parse"))
    }

    #[test]
    fn parses_drop_modifier_combos() {
        let key = parse_key("#[drop_modifier_combos(Ctrl + Shift)] LeftAlt = 226").unwrap();

        assert_eq!(key.name, "LeftAlt");
        assert_eq!(
            key.modifier_combo_drops
                .iter()
                .map(|combo| combo.iter().map(ToString::to_string).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            vec![vec!["Ctrl".to_owned(), "Shift".to_owned()]]
        );
    }

    #[test]
    fn rejects_trailing_plus_in_drop_modifier_combos() {
        let error = parse_key_error("#[drop_modifier_combos(Ctrl +)] Foo = 1");

        assert!(error.to_string().contains("trailing `+`"), "{error}");
    }

    #[test]
    fn rejects_an_empty_drop_modifier_combos() {
        let error = parse_key_error("#[drop_modifier_combos()] Foo = 1");

        assert!(error.to_string().contains("empty"), "{error}");
    }

    #[test]
    fn rejects_unknown_key_attribute() {
        let error = parse_key_error("#[drop_modifiar_combos(Shift)] Foo = 1");

        assert!(
            error.to_string().contains("unknown key attribute"),
            "{error}"
        );
    }

    #[test]
    fn rejects_a_second_doc_comment() {
        let error = parse_key_error("/// -\n/// Dash\nFoo = 1");

        assert!(
            error.to_string().contains("multiple doc comments"),
            "{error}"
        );
    }
}
