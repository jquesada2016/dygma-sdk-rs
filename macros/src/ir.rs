use crate::{ast, lit_int_to_u16};
use itertools::Itertools;
use proc_macro2::Span;
use std::collections::{HashMap, HashSet};

struct KeyCodeOverflowsU16Error;

pub struct Ir(pub Vec<KeyTable>);

impl From<ast::Ast> for Ir {
    fn from(ast: ast::Ast) -> Self {
        let tables: Vec<KeyTable> = ast.0.into_iter().map(KeyTable::from).collect();

        abort_on_duplicate_codes(&tables);
        abort_on_ownerless_drops(&tables);

        Self(tables)
    }
}

/// Every generated key ends up as a match arm on the same `u16`, so two
/// keys sharing a code would silently make one of them unreachable.
/// Aborts the compile, pointing at the second key, when a code repeats —
/// whether the repeat is within one table or across tables.
///
/// The guarantee boundary: this proves *code* uniqueness at compile
/// time. Parse-matcher uniqueness (two keys normalizing to the same
/// parse string) is a separate property — it is proven by the keycode
/// round-trip test in `keycode_tables` plus the zero-warning clippy gate
/// on the generated matches, not here.
fn abort_on_duplicate_codes(tables: &[KeyTable]) {
    let mut first_key_by_code: HashMap<u16, String> = HashMap::new();

    for table in tables {
        check_table_for_duplicates(table, &mut first_key_by_code);
    }
}

/// Checks `table`'s keys against `seen`, aborting the compile when a key
/// repeats a code — either from an earlier key of the same table, or
/// (when `seen` is carried across tables) from an earlier table.
fn check_table_for_duplicates(table: &KeyTable, seen: &mut HashMap<u16, String>) {
    let keys = table
        .keys
        .iter()
        .chain(&table.keys_with_modifiers)
        .chain(&table.keys_with_dual_functions);

    for key in keys {
        let code = lit_int_to_u16(&key.code);
        let name = format!("{}::{}", table.name, key.name);

        if let Some(first_name) = seen.insert(code, name.clone()) {
            abort!(
                key.code.span(),
                "duplicate key code {}: `{}` collides with `{}`",
                code,
                name,
                first_name;
                help = "every key must have a unique code"
            );
        }
    }
}

/// A `drop_modifier_combos` entry exists to hand its expansion's
/// would-be code (`key.code + mask`) to a named key. Verifies that, for
/// every drop, some key actually owns that code — otherwise the drop
/// silently removes it from the resolution space (expansion gone, no
/// owner, code becomes unknown), and a future DSL edit could delete a
/// keycode without a sound. Aborts the compile, pointing at the
/// offending key, mirroring `abort_on_duplicate_codes`.
fn abort_on_ownerless_drops(tables: &[KeyTable]) {
    let owned_codes: HashSet<u16> = tables
        .iter()
        .flat_map(|table| {
            table
                .keys
                .iter()
                .chain(&table.keys_with_modifiers)
                .chain(&table.keys_with_dual_functions)
        })
        .map(|key| lit_int_to_u16(&key.code))
        .collect();

    for table in tables {
        for key in &table.keys {
            let base = lit_int_to_u16(&key.code);

            for &mask in &key.modifier_combo_drops {
                if !is_unowned_drop(&owned_codes, base, mask) {
                    continue;
                }

                match base.checked_add(mask) {
                    Some(code) => abort!(
                        key.code.span(),
                        "dropping the `+0x{mask:04X}` modifier combo from `{}::{}` leaves code {code} unowned",
                        table.name,
                        key.name;
                        help = "a `drop_modifier_combos` entry must hand its code to a named key"
                    ),
                    None => abort!(
                        key.code.span(),
                        "the `+0x{mask:04X}` modifier combo of `{}::{}` overflows u16",
                        table.name,
                        key.name;
                        help = "every dropped combo's code must fit u16"
                    ),
                }
            }
        }
    }
}

/// Whether dropping `mask` from base `base` would leave its would-be
/// code (`base + mask`) owned by no key — either because no key owns
/// that code or because the sum overflows u16.
fn is_unowned_drop(owned_codes: &HashSet<u16>, base: u16, mask: u16) -> bool {
    match base.checked_add(mask) {
        Some(code) => !owned_codes.contains(&code),
        None => true,
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
    /// Masks of the modifier combos whose generated expansions must not
    /// exist for this key (from `drop_modifier_combos` attributes). The
    /// modifier values are distinct bits, so each combo's mask uniquely
    /// identifies it.
    pub modifier_combo_drops: Vec<u16>,
}

impl Key {
    fn new(key: ast::Key, offset: &mut u16) -> Result<Self, KeyCodeOverflowsU16Error> {
        let ast::Key {
            doc,
            modifier_combo_drops,
            name,
            code,
        } = key;

        *offset = if let Some(code_u16) = code.as_ref().map(lit_int_to_u16) {
            code_u16
        } else if let Some(code_u16) = offset.checked_add(1) {
            code_u16
        } else {
            return Err(KeyCodeOverflowsU16Error);
        };

        let meta = KeyMeta::from_doc(doc, &name);

        let code = code.unwrap_or_else(|| syn::LitInt::new(&offset.to_string(), name.span()));

        let modifier_combo_drops = modifier_combo_drops
            .iter()
            .map(|combo| resolve_modifier_combo_mask(combo))
            .collect();

        Ok(Self {
            meta,
            name,
            code,
            modifier_combo_drops,
        })
    }

    /// Whether the modifier powerset expansion must skip the combo
    /// `modifiers` for this key because a `drop_modifier_combos`
    /// attribute lists it.
    fn is_modifier_combo_dropped(&self, modifiers: &[Modifier]) -> bool {
        let mask = modifiers
            .iter()
            .copied()
            .map(|modifier| modifier.as_modifier_value())
            .sum::<u16>();

        self.modifier_combo_drops.contains(&mask)
    }

    fn with_modifiers(&self, modifiers: &[Modifier]) -> Result<Self, KeyCodeOverflowsU16Error> {
        let Self {
            meta, name, code, ..
        } = self;

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
            .sum::<u16>();

        let code_u16 = lit_int_to_u16(code)
            .checked_add(modifier_value)
            .ok_or(KeyCodeOverflowsU16Error)?;

        let code = syn::LitInt::new(&code_u16.to_string(), code.span());

        // Combo keys are leaf keys; only plain table keys carry drops.
        Ok(Self {
            meta,
            name,
            code,
            modifier_combo_drops: Vec::new(),
        })
    }

    fn with_dual_functions(
        &self,
        modifier: DualFunctionModifier,
    ) -> Result<Self, KeyCodeOverflowsU16Error> {
        let Self {
            meta, name, code, ..
        } = self;

        let meta = meta.with_dual_functions(modifier);

        let name = quote::format_ident!("Dual{name}And{modifier:?}");

        let code_u16 = lit_int_to_u16(code)
            .checked_add(modifier.as_modifier_value())
            .ok_or(KeyCodeOverflowsU16Error)?;

        let code = syn::LitInt::new(&code_u16.to_string(), code.span());

        Ok(Self {
            meta,
            name,
            code,
            modifier_combo_drops: Vec::new(),
        })
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

#[derive(Clone, Copy, Debug, Display, PartialEq)]
enum Modifier {
    Ctrl,
    Alt,
    AltGr,
    Shift,
    Os,
}

impl Modifier {
    /// Resolves one modifier name from a `drop_modifier_combos`
    /// attribute, aborting the compile on a name that is not a modifier.
    fn from_ident(ident: &syn::Ident) -> Self {
        match ident.to_string().as_str() {
            "Ctrl" => Self::Ctrl,
            "Alt" => Self::Alt,
            "AltGr" => Self::AltGr,
            "Os" => Self::Os,
            "Shift" => Self::Shift,
            _ => abort!(
                ident,
                "unknown modifier `{}` in `drop_modifier_combos`",
                ident;
                help = "expected one of Ctrl, Alt, AltGr, Os, Shift"
            ),
        }
    }

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

/// Why a `drop_modifier_combos` combo cannot be reduced to a mask.
#[derive(Debug, PartialEq)]
enum InvalidModifierCombo {
    /// No modifiers at all.
    Empty,
    /// A modifier repeats; the mask must combine DISTINCT modifier bits
    /// to identify one combo.
    Duplicate(Modifier),
}

/// Reduces a combo to a single mask of its modifiers' values. The
/// modifier values are distinct bits, so a combo's mask identifies it —
/// which only holds when no modifier repeats (`Ctrl + Ctrl` would sum to
/// `Alt`'s bit and drop the wrong expansion).
fn modifier_combo_mask(combo: &[Modifier]) -> Result<u16, InvalidModifierCombo> {
    let mut mask = 0;

    for modifier in combo {
        let bit = modifier.as_modifier_value();

        if mask & bit != 0 {
            return Err(InvalidModifierCombo::Duplicate(*modifier));
        }

        mask |= bit;
    }

    if mask == 0 {
        return Err(InvalidModifierCombo::Empty);
    }

    Ok(mask)
}

/// Resolves one `drop_modifier_combos` attribute into a single mask of
/// its modifiers' values, aborting the compile on an invalid combo.
fn resolve_modifier_combo_mask(combo: &[syn::Ident]) -> u16 {
    let modifiers: Vec<Modifier> = combo.iter().map(Modifier::from_ident).collect();

    match modifier_combo_mask(&modifiers) {
        Ok(mask) => mask,
        Err(InvalidModifierCombo::Empty) => abort!(
            Span::call_site(),
            "empty `drop_modifier_combos` attribute";
            help = "list at least one modifier: Ctrl, Alt, AltGr, Os, Shift"
        ),
        Err(InvalidModifierCombo::Duplicate(modifier)) => {
            let repeated = combo
                .iter()
                .filter(|ident| Modifier::from_ident(ident) == modifier)
                .nth(1)
                .expect("duplicate reported, so a second occurrence exists");

            abort!(
                repeated.span(),
                "duplicate modifier `{}` in `drop_modifier_combos`",
                modifier;
                help = "each modifier may appear at most once per combo"
            )
        }
    }
}

fn create_keys_with_modifiers(keys: &[Key]) -> Vec<Key> {
    Modifier::powerset()
        .into_iter()
        .filter(|modifiers| !modifiers.is_empty())
        .flat_map(|modifiers| {
            keys.iter().filter_map(move |key| {
                if key.is_modifier_combo_dropped(&modifiers) {
                    return None;
                }

                key.with_modifiers(&modifiers).ok()
            })
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{InvalidModifierCombo, Modifier, is_unowned_drop, modifier_combo_mask};

    #[test]
    fn distinct_modifiers_combine_to_their_mask() {
        assert_eq!(
            modifier_combo_mask(&[Modifier::Ctrl, Modifier::Shift]),
            Ok(0x0900)
        );
        assert_eq!(
            modifier_combo_mask(&[Modifier::Ctrl, Modifier::Alt, Modifier::Shift]),
            Ok(0x0B00)
        );
        assert_eq!(modifier_combo_mask(&[Modifier::Os]), Ok(0x1000));
    }

    #[test]
    fn a_repeated_modifier_is_rejected() {
        // `Ctrl + Ctrl` would otherwise sum to `Alt`'s bit.
        assert_eq!(
            modifier_combo_mask(&[Modifier::Ctrl, Modifier::Ctrl]),
            Err(InvalidModifierCombo::Duplicate(Modifier::Ctrl))
        );
    }

    #[test]
    fn an_empty_combo_is_rejected() {
        assert_eq!(modifier_combo_mask(&[]), Err(InvalidModifierCombo::Empty));
    }

    #[test]
    fn a_drop_without_an_owner_is_flagged() {
        let owned: HashSet<u16> = [2093].into();

        // `symbols::Dash` + `Shift` = 2093, owned by `shift_symbols`.
        assert!(!is_unowned_drop(&owned, 45, 2048));

        // The same drop with no `shift_symbols` key would orphan 2093.
        assert!(is_unowned_drop(&HashSet::new(), 45, 2048));

        // A base + mask that cannot fit u16 can never be owned.
        assert!(is_unowned_drop(&owned, u16::MAX, 2048));
    }
}
