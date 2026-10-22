//! The serde implementation behind the products' colormap layers,
//! stamped out per product by [`colormap_layer_serde!`].
//!
//! Every product's colormap layer serializes as a JSON object carrying
//! the write-only `layer_number` UX aid plus one palette-index array
//! per LED span, and deserializes with the derived-struct rules the
//! config files rely on: a present `layer_number` accepted and
//! discarded, any other unknown field ignored, a repeated LED field a
//! `duplicate field` error (not a silent overwrite), a missing LED
//! field a `missing field` error, and each array length-checked into
//! its fixed size with serde's `invalid length` shape.
//!
//! Those rules used to exist as four hand-written visitor pairs (one
//! per product) that differed only in the span fields' names, counts,
//! and labels; they live here now, once. The JSON error text is
//! user-facing — the diagnostics bridge renders serde_json's message
//! verbatim — so the sentence builders below are pinned by tests to
//! the exact strings the hand-written visitors produced.

/// Stamps out the [`serde::Serialize`] and [`serde::Deserialize`]
/// impls for one product's colormap layer.
///
/// The invocation names the layer type and then, in wire order, one
/// entry per LED-span field: the struct field, its internal `Field`
/// variant, the bracketed per-layer index count, and the label the
/// layer's "expecting" sentence uses for the span.
///
/// ```text
/// colormap_layer_serde! {
///     DefyColormapLayer,
///     keyboard => Keyboard [KEYBOARD_LEDS] "keyboard",
///     underglow => Underglow [UNDERGLOW_LEDS] "underglow",
/// }
/// ```
///
/// The generated deserializer carries the shared rule set — the
/// duplicate-field / ignored-field / length-check behavior documented
/// in the [module docs](self) — with every error message byte-identical
/// to the hand-written visitors this replaced.
macro_rules! colormap_layer_serde {
    (
        $layer:ident,
        $( $field:ident => $variant:ident [ $count:expr ] $label:literal ),+ $(,)?
    ) => {
        impl serde::Serialize for $layer {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                use serde::ser::SerializeStruct;

                let mut state = serializer.serialize_struct(
                    stringify!($layer),
                    1 + [$(stringify!($field)),+].len(),
                )?;

                state.serialize_field("layer_number", &self.layer_number)?;
                $(
                    state.serialize_field(stringify!($field), &&self.$field[..])?;
                )+

                state.end()
            }
        }

        impl<'de> serde::Deserialize<'de> for $layer {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                enum Field {
                    $(
                        $variant,
                    )+
                    /// A present `layer_number` — accepted and discarded, the
                    /// way the derived skip-deserializing keymap layers treat
                    /// theirs (it is a write-only UX aid).
                    LayerNumber,
                    /// Any other field — ignored, the way derived structs
                    /// tolerate unknown fields (`deny_unknown_fields` is not
                    /// the derived default).
                    Other,
                }

                impl<'de> serde::Deserialize<'de> for Field {
                    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                    where
                        D: serde::Deserializer<'de>,
                    {
                        struct FieldVisitor;

                        impl serde::de::Visitor<'_> for FieldVisitor {
                            type Value = Field;

                            fn expecting(
                                &self,
                                f: &mut std::fmt::Formatter<'_>,
                            ) -> std::fmt::Result {
                                f.write_str(
                                    &$crate::devices::colormap_serde::field_name_list(&[
                                        $(stringify!($field)),
                                        +
                                    ]),
                                )
                            }

                            fn visit_str<E: serde::de::Error>(
                                self,
                                value: &str,
                            ) -> Result<Field, E> {
                                match value {
                                    $(
                                        stringify!($field) => Ok(Field::$variant),
                                    )+
                                    "layer_number" => Ok(Field::LayerNumber),
                                    _ => Ok(Field::Other),
                                }
                            }
                        }

                        deserializer.deserialize_identifier(FieldVisitor)
                    }
                }

                struct LayerVisitor;

                impl<'de> serde::de::Visitor<'de> for LayerVisitor {
                    type Value = $layer;

                    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        f.write_str(
                            &$crate::devices::colormap_serde::layer_expectation(&[
                                $(($count, $label)),+
                            ]),
                        )
                    }

                    fn visit_map<A: serde::de::MapAccess<'de>>(
                        self,
                        mut map: A,
                    ) -> Result<Self::Value, A::Error> {
                        $(
                            let mut $field = None::<
                                Vec<$crate::focus_api::parsing::colormap::PaletteIndex>,
                            >;
                        )+

                        while let Some(field) = map.next_key::<Field>()? {
                            match field {
                                $(
                                    Field::$variant => {
                                        if $field.is_some() {
                                            return Err(serde::de::Error::duplicate_field(
                                                stringify!($field),
                                            ));
                                        }

                                        $field = Some(map.next_value()?);
                                    }
                                )+
                                Field::LayerNumber | Field::Other => {
                                    map.next_value::<serde::de::IgnoredAny>()?;
                                }
                            }
                        }

                        Ok(Self::Value {
                            layer_number: 0,
                            $(
                                $field: $crate::devices::colormap_serde::checked_fixed(
                                    $field,
                                    stringify!($field),
                                )?,
                            )+
                        })
                    }
                }

                deserializer.deserialize_struct(
                    stringify!($layer),
                    &["layer_number", $(stringify!($field)),+],
                    LayerVisitor,
                )
            }
        }
    };
}

pub(super) use colormap_layer_serde;

/// Builds the field identifier's "expecting" sentence from the LED
/// field names: `` `keyboard`, `underglow_left`, or `underglow_right`
/// ``, with no "or" for one or two names.
pub(super) fn field_name_list(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [only] => format!("`{only}`"),
        [first, second] => format!("`{first}` or `{second}`"),
        [lead @ .., last] => {
            let lead = lead
                .iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", ");

            format!("{lead}, or `{last}`")
        }
    }
}

/// Builds the layer visitor's "expecting" sentence from the spans'
/// index counts and labels: "a colormap layer of 70 keyboard and 108
/// underglow palette indices" for the two-span shape, with a
/// comma-and-"and" list for three spans and a bare span for one.
pub(super) fn layer_expectation(spans: &[(usize, &str)]) -> String {
    let parts = spans
        .iter()
        .map(|(count, label)| format!("{count} {label}"))
        .collect::<Vec<_>>();

    let list = match parts.as_slice() {
        [] => String::new(),
        [only] => only.clone(),
        [first, second] => format!("{first} and {second}"),
        [lead @ .., last] => format!("{}, and {}", lead.join(", "), last),
    };

    format!("a colormap layer of {list} palette indices")
}

/// The indefinite article a span field's name takes in the length
/// error: "a `keyboard` array", "an `underglow` array".
fn index_article(name: &str) -> &'static str {
    match name.as_bytes().first() {
        Some(b'a' | b'e' | b'i' | b'o' | b'u') => "an",
        _ => "a",
    }
}

/// Turns one LED span's collected value into its fixed-size array,
/// naming the field for the missing-field error and naming the FOUND
/// count for the length error (serde's `invalid_length` takes the
/// found count, not the required one — `TryFrom<Vec<T>> for [T; N]`
/// hands the vec back as its error).
pub(super) fn checked_fixed<const N: usize, E: serde::de::Error>(
    found: Option<Vec<crate::focus_api::parsing::colormap::PaletteIndex>>,
    name: &'static str,
) -> Result<[crate::focus_api::parsing::colormap::PaletteIndex; N], E> {
    let found = found.ok_or_else(|| serde::de::Error::missing_field(name))?;

    found.try_into().map_err(|found: Vec<_>| {
        serde::de::Error::invalid_length(
            found.len(),
            &format!(
                "{} `{name}` array of exactly {N} indices",
                index_article(name),
            )
            .as_str(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::focus_api::parsing::colormap::PaletteIndex;

    #[test]
    fn expectation_sentences_match_the_hand_written_visitors_output() {
        // The exact strings the four hand-written visitors produced,
        // pinned per product shape so the builders cannot drift: Defy
        // (two spans), Sonsei (one), Raise, and Raise 2 (three).
        assert_eq!(
            layer_expectation(&[(70, "keyboard"), (108, "underglow")]),
            "a colormap layer of 70 keyboard and 108 underglow palette indices"
        );
        assert_eq!(
            layer_expectation(&[(56, "keyboard")]),
            "a colormap layer of 56 keyboard palette indices"
        );
        assert_eq!(
            layer_expectation(&[
                (69, "keyboard"),
                (30, "left-underglow"),
                (33, "right-underglow")
            ]),
            "a colormap layer of 69 keyboard, 30 left-underglow, and 33 right-underglow palette indices"
        );
        assert_eq!(
            layer_expectation(&[
                (69, "keyboard"),
                (53, "left-underglow"),
                (54, "right-underglow")
            ]),
            "a colormap layer of 69 keyboard, 53 left-underglow, and 54 right-underglow palette indices"
        );

        assert_eq!(field_name_list(&["keyboard"]), "`keyboard`");
        assert_eq!(
            field_name_list(&["keyboard", "underglow"]),
            "`keyboard` or `underglow`"
        );
        assert_eq!(
            field_name_list(&["keyboard", "underglow_left", "underglow_right"]),
            "`keyboard`, `underglow_left`, or `underglow_right`"
        );

        assert_eq!(index_article("keyboard"), "a");
        assert_eq!(index_article("underglow"), "an");
        assert_eq!(index_article("underglow_left"), "an");
        assert_eq!(index_article("underglow_right"), "an");
    }

    #[test]
    fn checked_fixed_names_the_field_and_the_found_length() {
        let index = PaletteIndex::new(0).unwrap();

        // Missing.
        let err = checked_fixed::<70, serde_json::Error>(None, "keyboard").unwrap_err();

        assert_eq!(err.to_string(), "missing field `keyboard`");

        // Wrong length: the FOUND count leads, the required one trails.
        let err =
            checked_fixed::<70, serde_json::Error>(Some(vec![index]), "keyboard").unwrap_err();

        assert_eq!(
            err.to_string(),
            "invalid length 1, expected a `keyboard` array of exactly 70 indices"
        );

        let err =
            checked_fixed::<108, serde_json::Error>(Some(vec![index]), "underglow").unwrap_err();

        assert_eq!(
            err.to_string(),
            "invalid length 1, expected an `underglow` array of exactly 108 indices"
        );

        // The right length passes through.
        let ok = checked_fixed::<1, serde_json::Error>(Some(vec![index]), "keyboard").unwrap();

        assert_eq!(ok, [index]);
    }
}
