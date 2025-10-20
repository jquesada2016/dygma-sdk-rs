use crate::{ast, lit_int_to_u16};
use itertools::Itertools;

struct KeyCodeOverflowsU16Error;

pub struct Ir(pub Vec<KeyTable>);

impl From<ast::Ast> for Ir {
    fn from(ast: ast::Ast) -> Self {
        let tables = ast.0.into_iter().map(KeyTable::from).collect();

        Self(tables)
    }
}

pub struct KeyTable {
    pub doc: syn::Attribute,
    pub name: syn::Ident,
    pub keys: Vec<Key>,
    pub keys_with_modifiers: Vec<Key>,
    pub keys_with_dual_functions: Vec<Key>,
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
            .collect::<Vec<_>>();

        let keys_with_modifiers = if with_modifiers.is_some() {
            create_keys_with_modifiers(&keys)
        } else {
            Vec::default()
        };

        let keys_with_dual_functions = if with_dual_functions.is_some() {
            create_keys_with_dual_functions(&keys)
        } else {
            Vec::default()
        };

        Self {
            doc,
            name,
            keys,
            keys_with_modifiers,
            keys_with_dual_functions,
        }
    }
}

pub struct Key {
    pub meta: KeyMeta,
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

        let meta = KeyMeta::from_doc(doc, &name);

        let code = code.unwrap_or_else(|| syn::LitInt::new(&offset.to_string(), name.span()));

        Ok(Self { meta, name, code })
    }

    fn with_modifiers(&self, modifiers: &[Modifier]) -> Result<Self, KeyCodeOverflowsU16Error> {
        let Self { meta, name, code } = self;

        let meta = meta.with_modifiers(modifiers);

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

        let code_u16 = lit_int_to_u16(code)
            .checked_add(modifier_value)
            .ok_or(KeyCodeOverflowsU16Error)?;

        let code = syn::LitInt::new(&code_u16.to_string(), code.span());

        Ok(Self { meta, name, code })
    }

    fn with_dual_functions(
        &self,
        modifier: DualFunctionModifier,
    ) -> Result<Self, KeyCodeOverflowsU16Error> {
        let Self { meta, name, code } = self;

        let meta = meta.with_dual_functions(modifier);

        let name = quote::format_ident!("Dual{name}And{modifier:?}");

        let code_u16 = lit_int_to_u16(code)
            .checked_add(modifier.as_modifier_value())
            .ok_or(KeyCodeOverflowsU16Error)?;

        let code = syn::LitInt::new(&code_u16.to_string(), code.span());

        Ok(Self { meta, name, code })
    }
}

pub struct KeyMeta {
    pub display_name: syn::LitStr,
}

impl KeyMeta {
    fn from_doc(doc: Option<syn::Attribute>, name: &syn::Ident) -> Self {
        let display_name = doc
            .map(|doc| {
                let syn::Attribute {
                    meta: syn::Meta::NameValue(syn::MetaNameValue { value, .. }),
                    ..
                } = doc
                else {
                    abort!(doc, "doc comment did not have expected shape");
                };

                let display_name: syn::LitStr = syn::parse_quote! { #value };

                syn::LitStr::new(display_name.value().trim(), display_name.span())
            })
            .unwrap_or_else(|| syn::LitStr::new(&name.to_string(), name.span()));

        Self { display_name }
    }

    fn with_modifiers(&self, modifiers: &[Modifier]) -> Self {
        let Self { display_name } = self;

        let modifier_prefix = modifiers.iter().map(ToString::to_string).join(" + ");

        let display_name = syn::LitStr::new(
            &format!("{modifier_prefix} + {}", display_name.value()),
            display_name.span(),
        );

        Self { display_name }
    }

    fn with_dual_functions(&self, modifier: DualFunctionModifier) -> Self {
        let Self { display_name } = self;

        let display_name = syn::LitStr::new(
            &format!("{} / {modifier}", display_name.value()),
            display_name.span(),
        );

        Self { display_name }
    }
}

#[derive(Clone, Copy, Display)]
enum Modifier {
    Ctrl,
    Alt,
    AltGr,
    Shift,
    Os,
}

impl Modifier {
    const fn as_modifier_value(self) -> u16 {
        match self {
            Self::Ctrl => 0x0100,
            Self::Alt => 0x0200,
            Self::AltGr => 0x0400,
            Self::Shift => 0x0800,
            Self::Os => 0x1000,
        }
    }

    fn powerset() -> Vec<Vec<Self>> {
        [Self::Ctrl, Self::Alt, Self::AltGr, Self::Os, Self::Shift]
            .into_iter()
            .powerset()
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Display)]
enum DualFunctionModifier {
    Ctrl,
    Alt,
    AltGr,
    Os,
    Shift,
    #[display("Layer 1")]
    Layer1,
    #[display("Layer 2")]
    Layer2,
    #[display("Layer 3")]
    Layer3,
    #[display("Layer 4")]
    Layer4,
    #[display("Layer 5")]
    Layer5,
    #[display("Layer 6")]
    Layer6,
    #[display("Layer 7")]
    Layer7,
    #[display("Layer 8")]
    Layer8,
}

impl DualFunctionModifier {
    const VALUES: &[Self] = &[
        Self::Ctrl,
        Self::Alt,
        Self::AltGr,
        Self::Os,
        Self::Shift,
        Self::Layer1,
        Self::Layer2,
        Self::Layer3,
        Self::Layer4,
        Self::Layer5,
        Self::Layer6,
        Self::Layer7,
        Self::Layer8,
    ];

    const fn as_modifier_value(self) -> u16 {
        match self {
            Self::Ctrl => 49169,
            Self::Alt => 49681,
            Self::AltGr => 50705,
            Self::Os => 49937,
            Self::Shift => 49425,
            Self::Layer1 => 51218,
            Self::Layer2 => 51474,
            Self::Layer3 => 51730,
            Self::Layer4 => 51986,
            Self::Layer5 => 52242,
            Self::Layer6 => 52498,
            Self::Layer7 => 52754,
            Self::Layer8 => 53010,
        }
    }
}

fn create_keys_with_modifiers(keys: &[Key]) -> Vec<Key> {
    Modifier::powerset()
        .into_iter()
        .filter(|modifiers| !modifiers.is_empty())
        .flat_map(|modifiers| {
            keys.iter()
                .filter_map(move |key| key.with_modifiers(&modifiers).ok())
        })
        .collect()
}

fn create_keys_with_dual_functions(keys: &[Key]) -> Vec<Key> {
    DualFunctionModifier::VALUES
        .iter()
        .copied()
        .flat_map(|modifier| {
            keys.iter()
                .filter_map(move |key| key.with_dual_functions(modifier).ok())
        })
        .collect()
}
