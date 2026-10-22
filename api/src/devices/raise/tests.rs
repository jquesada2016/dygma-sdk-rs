//! Tests for the Raise format-level model: wire shapes through the
//! typed maps, the physical layout, and the offline restore-plan
//! validation against the real shipped artifacts.

use super::*;
use crate::devices::conversions::{self, ConversionTarget};
use crate::focus_api::parsing;

/// The shipped Raise ANSI backup fixture (a byte-identical copy of
/// `bazecor/src/defaultBackups/RaiseANSI.json`): a real device
/// capture — 800-token keymap, 1320-token colormap, 48-decimal
/// palette, the padded superkeys form (512 tokens: one superkey, then
/// stale bytes and padding), macros with stale-byte tails.
const RAISE_ANSI_BACKUP_JSON: &str =
    include_str!("../../../../tools/bazecor-differential/fixtures/RaiseANSI.json");

/// The shipped Raise ISO backup fixture: the same shapes, with the
/// superkeys and macros entries in the live-bytes form (7 and 36
/// tokens).
const RAISE_ISO_BACKUP_JSON: &str =
    include_str!("../../../../tools/bazecor-differential/fixtures/RaiseISO.json");

fn raise_backup(json: &str) -> parsing::backup::DefyBackup {
    serde_json::from_str(json).expect("the shipped Raise backup parses")
}

#[test]
fn the_layout_models_seventy_three_keys_and_leaves_seven_slots() {
    // The descriptor rows total 73; the unmodeled slots are exactly
    // the seven wire positions the shipped backups never carry a real
    // key at (pinned against the real layer data below).
    let modeled: std::collections::BTreeSet<u8> = LAYOUT
        .left
        .row_1
        .into_iter()
        .chain(LAYOUT.left.row_2)
        .chain(LAYOUT.left.row_3)
        .chain(LAYOUT.left.row_4)
        .chain(LAYOUT.left.thumb_row)
        .chain(LAYOUT.right.row_1)
        .chain(LAYOUT.right.row_2)
        .chain(LAYOUT.right.row_3)
        .chain(LAYOUT.right.row_4)
        .chain(LAYOUT.right.thumb_row)
        .collect();

    assert_eq!(modeled.len(), 73);
    assert_eq!(
        modeled,
        (0..80)
            .filter(|position| ![7, 8, 23, 39, 40, 55, 56].contains(position))
            .collect()
    );
}

#[test]
fn the_real_layers_name_exactly_the_descriptor_gaps_plus_four_factory_empty_keys() {
    // The LAYOUT derivation's second source, pinned exactly: across
    // all twenty shipped layers, the positions that never carry a
    // real key are the seven descriptor gaps (7–8, 23, 39–40, 55–56
    // — each holding `0` on layers 1–2 and `65535` transparent on
    // layers 3–10) PLUS four wire positions the descriptor's rows
    // NAME but the factory default maps on no layer: 22 (left row 2's
    // last key), 38 (left row 3's last), 57 (right row 4's first),
    // and 69 (a left thumb key). Modeled-ness is the DESCRIPTOR's
    // fact — the LAYOUT models all four; factory-emptiness is the
    // default's. Every other position carries at least one real key
    // code somewhere across the layers.
    for json in [RAISE_ANSI_BACKUP_JSON, RAISE_ISO_BACKUP_JSON] {
        let backup = raise_backup(json);

        let keymap = backup
            .backup
            .iter()
            .find(|entry| entry.command == "keymap.custom")
            .expect("the backup carries a keymap.custom");

        let tokens: Vec<u16> = keymap
            .data
            .split_whitespace()
            .map(|token| token.parse().expect("a key code"))
            .collect();

        assert_eq!(tokens.len(), 800);

        let mut never_a_key = Vec::new();

        for position in 0..80u16 {
            let values: std::collections::BTreeSet<u16> = (0..10)
                .map(|layer| tokens[layer * 80 + position as usize])
                .collect();

            let empty = match values.len() {
                1 => values.contains(&0) || values.contains(&65535),
                2 => values.contains(&0) && values.contains(&65535),
                _ => false,
            };

            if empty {
                never_a_key.push(position);
            }
        }

        assert_eq!(
            never_a_key,
            vec![7, 8, 22, 23, 38, 39, 40, 55, 56, 57, 69],
            "the ever-empty wire positions"
        );
    }
}

#[test]
fn the_real_backups_validate_at_full_magnitude() {
    // Both shipped Raise backups — real captures — validate whole
    // through the Raise validator: every shaped entry parses through
    // the Raise types (the padded AND live superkeys/macros forms
    // included), and the plan is the complete write sequence.
    for json in [RAISE_ANSI_BACKUP_JSON, RAISE_ISO_BACKUP_JSON] {
        let backup = raise_backup(json);

        let plan = backup_restore_plan(&backup).expect("the Raise backup validates");

        assert_eq!(plan.len(), backup.backup.len() + 1); // + the `led.mode 0` tail
        assert_eq!(plan.last().unwrap().command, "led.mode");
        assert_eq!(plan.last().unwrap().data, "0");
    }
}

#[test]
fn wrong_shapes_are_rejected_by_name() {
    // The Raise validator's shaped entries reject the OTHER products'
    // shapes with typed errors: a Defy colormap (178-LED layers), a
    // Defy palette (RGBW), a Sonsei colormap (56-LED layers), a
    // Raise 2 colormap (176-LED layers).
    let backup = raise_backup(RAISE_ANSI_BACKUP_JSON);

    let entry_data = |command: &str| {
        backup
            .backup
            .iter()
            .find(|entry| entry.command == command)
            .expect("the backup carries the command")
            .data
            .clone()
    };

    // A ten-layer stream at the Defy's 178-LED width is ragged at
    // the Raise's 132-LED width (the typed rejection the shape
    // mismatch produces); a correct-width stream with the wrong
    // layer COUNT hits the count rejection.
    let defy_colormap: String = std::iter::repeat_n("4", 1780).collect::<Vec<_>>().join(" ");

    assert!(matches!(
        RaiseColormap::from_str(&defy_colormap),
        Err(ParseColormapError::MalformedData(_))
    ));

    let nine_raise_layers: String = std::iter::repeat_n("4", 9 * 132)
        .collect::<Vec<_>>()
        .join(" ");

    assert!(matches!(
        RaiseColormap::from_str(&nine_raise_layers),
        Err(ParseColormapError::IncorrectLayerCount(_))
    ));

    // 1320 tokens parsed at the wrong width: the Sonsei's 56-LED and
    // the Raise 2's 176-LED layers cannot hold the Raise's layers,
    // and a Raise 2-width stream (10 × 176) is wrong-shaped for the
    // Raise's 132-LED ones.
    let raise_colormap = entry_data("colormap.map");

    assert!(
        raise_colormap
            .parse::<crate::devices::sonsei::SonseiColormap>()
            .is_err()
    );
    assert!(
        raise_colormap
            .parse::<crate::devices::raise2::Raise2Colormap>()
            .is_err()
    );

    let raise2_width: String = std::iter::repeat_n("4", 1760).collect::<Vec<_>>().join(" ");

    assert!(raise2_width.parse::<RaiseColormap>().is_err());

    // The Defy palette shape (64 decimals — RGBW) is not an RGB
    // palette (48 decimals).
    let defy_palette: String = std::iter::repeat_n("0 0 0 0", 16)
        .collect::<Vec<_>>()
        .join(" ");

    assert!(parsing::palette::RgbPalette::from_str(&defy_palette).is_err());
}

#[test]
fn the_keymap_types_round_trip_json_and_wire() {
    // A real layer's data, through the typed round trip any config
    // file takes: wire parse → RaiseKeymapLayer → JSON → read back →
    // the same wire form.
    let backup = raise_backup(RAISE_ANSI_BACKUP_JSON);

    let keymap_data = backup
        .backup
        .iter()
        .find(|entry| entry.command == "keymap.custom")
        .expect("the backup carries a keymap.custom")
        .data
        .clone();

    let keymap: RaiseKeymap = keymap_data.parse().expect("the real keymap parses");

    let json = serde_json::to_vec_pretty(&keymap).expect("the keymap serializes");
    let read_back: RaiseKeymap =
        serde_json::from_slice(&json).expect("the written keymap JSON reads back");

    // The typed round trip normalizes the seven unmodeled slots to
    // `0` — the serializer's padding — so the comparison is against
    // the re-serialized form, which the real backup's layer 1 (whose
    // padding slots carry `0`) matches byte for byte and the upper
    // layers (whose padding slots carry `65535`) match everywhere
    // except those seven slots.
    let serialized = read_back.to_keymap_custom_data();
    let serialized_tokens: Vec<&str> = serialized.split_whitespace().collect();
    let original_tokens: Vec<&str> = keymap_data.split_whitespace().collect();

    assert_eq!(serialized_tokens.len(), original_tokens.len());

    for (position, (serialized_token, original_token)) in
        serialized_tokens.iter().zip(&original_tokens).enumerate()
    {
        if [7, 8, 23, 39, 40, 55, 56].contains(&(position % 80)) {
            assert_eq!(
                *serialized_token, "0",
                "the unmodeled slot serializes as padding"
            );
        } else {
            assert_eq!(serialized_token, original_token);
        }
    }

    // A wrong layer count is unrepresentable, at the wire boundary
    // and the JSON one.
    let nine_layers: Vec<_> = original_tokens[..9 * 80].to_vec();
    assert!(nine_layers.join(" ").parse::<RaiseKeymap>().is_err());

    let ten_layers_json = serde_json::to_vec(&keymap).unwrap();
    let mut layers: Vec<serde_json::Value> = serde_json::from_slice(&ten_layers_json).unwrap();
    layers.pop();
    assert!(serde_json::from_value::<RaiseKeymap>(serde_json::Value::Array(layers)).is_err());
}

#[test]
fn the_converted_backups_revalidate_through_the_raise_generation_validators() {
    // The cross-unit property: a backup converted by the conversions
    // unit's Raise↔Raise2 recipes validates whole through the TARGET
    // product's validator — the converted file is a real Raise 2 (or
    // Raise) backup as far as the restore validation is concerned.
    //
    // (The Raise→Raise2 direction validates here; the Raise2→Raise
    // direction's sources are the synthesized virtual backups, run
    // through the same proof in the raise2 module's tests.)
    let backup = raise_backup(RAISE_ANSI_BACKUP_JSON);

    for keyboard_type in [
        conversions::KeyboardType::Ansi,
        conversions::KeyboardType::Iso,
    ] {
        let (converted, report) =
            conversions::convert_backup(&backup, ConversionTarget::Raise2(keyboard_type))
                .expect("the Raise backup converts");

        assert_eq!(report.from, conversions::Product::Raise);

        let plan = crate::devices::raise2::backup_restore_plan(&converted)
            .expect("the converted Raise 2 backup validates as a Raise 2");

        assert_eq!(plan.len(), converted.backup.len() + 1);
    }
}
