use crate::{KeyCodeOverflowsU16Error, ast, lit_int_to_u16};

pub struct Ir(pub Vec<KeyTable>);

impl From<ast::Ast> for Ir {
    fn from(ast: ast::Ast) -> Self {
        let tables = ast.0.into_iter().map(KeyTable::from).collect();

        Self(tables)
    }
}

pub struct KeyTable {
    pub doc: syn::Attribute,
    pub with_modifiers: Option<syn::Attribute>,
    pub with_dual_functions: Option<syn::Attribute>,
    pub name: syn::Ident,
    pub keys: Vec<Key>,
}

impl From<ast::KeyTable> for KeyTable {
    fn from(table: ast::KeyTable) -> Self {
        let ast::KeyTable {
            doc,
            with_modifiers,
            with_dual_functions,
            name,
            keys,
        } = table;

        let mut offset = 0;

        let keys = keys
            .into_iter()
            .filter_map(|key| Key::new(key, &mut offset).ok())
            .collect();

        Self {
            doc,
            with_modifiers,
            with_dual_functions,
            name,
            keys,
        }
    }
}

pub struct Key {
    pub doc: Option<syn::Attribute>,
    pub name: syn::Ident,
    pub code: syn::LitInt,
}

impl Key {
    fn new(key: ast::Key, offset: &mut u16) -> Result<Self, KeyCodeOverflowsU16Error> {
        let ast::Key { doc, name, code } = key;

        *offset = if let Some(code_u16) = code.as_ref().map(lit_int_to_u16) {
            code_u16
        } else if let Some(code_u16) = offset.checked_add(1) {
            code_u16
        } else {
            return Err(KeyCodeOverflowsU16Error);
        };

        let code = code.unwrap_or_else(|| syn::LitInt::new(&offset.to_string(), name.span()));

        Ok(Self { doc, name, code })
    }
}
