pub struct Ast(pub Vec<KeyTable>);

pub struct KeyTable {
    pub doc: syn::Attribute,
    pub with_modifiers: Option<syn::Attribute>,
    pub with_dual_functions: Option<syn::Attribute>,
    pub name: syn::Ident,
    pub keys: Vec<Key>,
}

pub struct Key {
    pub doc: Option<syn::Attribute>,
    pub name: syn::Ident,
    pub code: Option<syn::LitInt>,
}
