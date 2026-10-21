//! Tests for the cross-product conversion layer: mapping-table
//! consistency, the layered-wire shape checks, and the color math.
//!
//! The differential tests — this module's output compared byte for
//! byte against expectations recorded from Bazecor's own conversion
//! source — live in `differential` below.

use super::*;

#[test]
fn keymap_table_maps_every_sonsei_position_in_the_bazecor_pattern() {
    // Rows 0–3: columns 0–5 ← Defy columns 0–5; columns 6–11 ← Defy
    // columns 10–15. Thumb row: 0–3 ← 64–67, 4–7 none, 8–11 ← 76–79.
    let table = &DEFY_TO_SONSEI_KEYMAP;

    assert_eq!(table[0], 0);
    assert_eq!(table[5], 5);
    assert_eq!(table[6], 10);
    assert_eq!(table[11], 15);
    assert_eq!(table[12], 16);
    assert_eq!(table[47], 63);
    assert_eq!(table[48], 64);
    assert_eq!(table[51], 67);
    assert_eq!(table[52], NO_SOURCE_POSITION);
    assert_eq!(table[55], NO_SOURCE_POSITION);
    assert_eq!(table[56], 76);
    assert_eq!(table[59], 79);

    // The dropped Defy positions per layer: the inner columns (6, 9),
    // the wire gaps (7, 8) of rows 0–2, the row-3 gap (6–9 → only the
    // real padding 54–57), and the bottom thumb rows 68–75.
    let dropped = (0..80u16)
        .filter(|source| !table.contains(&(*source as i16)))
        .collect::<Vec<_>>();

    assert_eq!(
        dropped,
        [
            6, 7, 8, 9, 22, 23, 24, 25, 38, 39, 40, 41, 54, 55, 56, 57, 68, 69, 70, 71, 72, 73, 74,
            75
        ]
    );
}

#[test]
fn colormap_table_maps_every_sonsei_led_in_the_bazecor_pattern() {
    let table = &DEFY_TO_SONSEI_COLORMAP;

    // Row 0 left-to-right then the reversed right half; thumb row
    // takes the cluster tops.
    assert_eq!(table[0], 0);
    assert_eq!(table[5], 5);
    assert_eq!(table[6], 40);
    assert_eq!(table[11], 35);
    assert_eq!(table[12], 7);
    assert_eq!(table[47], 56);
    assert_eq!(table[48], 27);
    assert_eq!(table[51], 30);
    assert_eq!(table[52], 65);
    assert_eq!(table[55], 62);

    // Every Sonsei LED has a Defy source — nothing is defaulted in the
    // forward direction.
    assert!(!table.contains(&NO_SOURCE_POSITION));

    // The forward map covers 56 distinct Defy LEDs; the other 122 (the
    // underglow plus the LEDs of keys the Sonsei has no equivalent
    // for) are dropped.
    let distinct = table.iter().collect::<std::collections::HashSet<_>>();

    assert_eq!(distinct.len(), 56);
}

#[test]
fn inverse_tables_are_exact_inverses_of_the_forward_tables() {
    for (source, target) in SONSEI_TO_DEFY_KEYMAP.iter().enumerate() {
        if *target != NO_SOURCE_POSITION {
            assert_eq!(
                DEFY_TO_SONSEI_KEYMAP[*target as usize], source as i16,
                "Sonsei → Defy entry {source} disagrees with the forward table"
            );
        }
    }

    for (source, target) in SONSEI_TO_DEFY_COLORMAP.iter().enumerate() {
        if *target != NO_SOURCE_POSITION {
            assert_eq!(
                DEFY_TO_SONSEI_COLORMAP[*target as usize], source as i16,
                "Sonsei → Defy colormap entry {source} disagrees with the forward table"
            );
        }
    }

    // And the reverse direction too: every forward mapping round
    // trips.
    for (target, source) in DEFY_TO_SONSEI_KEYMAP.iter().enumerate() {
        if *source != NO_SOURCE_POSITION {
            assert_eq!(SONSEI_TO_DEFY_KEYMAP[*source as usize], target as i16);
        }
    }

    for (target, source) in DEFY_TO_SONSEI_COLORMAP.iter().enumerate() {
        if *source != NO_SOURCE_POSITION {
            assert_eq!(SONSEI_TO_DEFY_COLORMAP[*source as usize], target as i16);
        }
    }
}

#[test]
fn sonsei_to_defy_keymap_defaults_are_the_expected_positions() {
    let defaulted = SONSEI_TO_DEFY_KEYMAP
        .iter()
        .enumerate()
        .filter(|(_, source)| **source == NO_SOURCE_POSITION)
        .map(|(position, _)| position as u16)
        .collect::<Vec<_>>();

    // The Defy-only positions: the inner columns and wire gaps of rows
    // 0–2, the row-3 gap, and the bottom thumb rows.
    assert_eq!(
        defaulted,
        [
            6, 7, 8, 9, 22, 23, 24, 25, 38, 39, 40, 41, 54, 55, 56, 57, 68, 69, 70, 71, 72, 73, 74,
            75
        ]
    );
}

#[test]
fn layered_value_parsing_checks_shape_and_range() {
    let ok = parse_layered_values("1 2 3 4 5 6", 3, 2, 255).unwrap();

    assert_eq!(ok, vec![vec![1, 2, 3], vec![4, 5, 6]]);

    // Extra separators are separators, nothing more.
    let spaced = parse_layered_values(" 1  2 3 4 5 6 ", 3, 2, 255).unwrap();

    assert_eq!(spaced, vec![vec![1, 2, 3], vec![4, 5, 6]]);

    // Not a number, with the token's real byte offset.
    let err = parse_layered_values("1 2 3 O 5 6", 3, 2, 255).unwrap_err();

    assert_eq!(err.layer_size, 3);
    assert_eq!(err.layer_count, 2);
    assert_eq!(
        err.kind,
        LayeredWireErrorKind::TokenNotANumber {
            offset: 6,
            token: "O".to_owned()
        }
    );

    // Out of range.
    let err = parse_layered_values("1 2 3 4 5 256", 3, 2, 255).unwrap_err();

    assert_eq!(
        err.kind,
        LayeredWireErrorKind::TokenOutOfRange {
            offset: 10,
            token: "256".to_owned(),
            max: 255
        }
    );

    // Wrong count: too few and too many.
    let err = parse_layered_values("1 2 3 4 5", 3, 2, 255).unwrap_err();

    assert_eq!(
        err.kind,
        LayeredWireErrorKind::WrongValueCount {
            found: 5,
            expected: 6
        }
    );

    let err = parse_layered_values("1 2 3 4 5 6 7", 3, 2, 255).unwrap_err();

    assert_eq!(
        err.kind,
        LayeredWireErrorKind::WrongValueCount {
            found: 7,
            expected: 6
        }
    );
}

#[test]
fn color_math_matches_bazecors_rgbw2b_and_rgb2w() {
    // rgbw2b folds the white channel in, clamping at the byte range.
    assert_eq!(
        convert_color(&[100, 50, 25, 30], ColorChannelConversion::RgbwToRgb),
        vec![130, 80, 55]
    );
    // Clamping: 240 + 30 = 270 → 255.
    assert_eq!(
        convert_color(&[240, 0, 0, 30], ColorChannelConversion::RgbwToRgb),
        vec![255, 30, 30]
    );
    // rgb2w splits the shared component out.
    assert_eq!(
        convert_color(&[130, 80, 55], ColorChannelConversion::RgbToRgbw),
        vec![75, 25, 0, 55]
    );
    // Round trip through rgb2w then rgbw2b restores the color.
    let original = [17u16, 200, 129];
    let back = convert_color(
        &convert_color(&original, ColorChannelConversion::RgbToRgbw),
        ColorChannelConversion::RgbwToRgb,
    );

    assert_eq!(back, original.to_vec());
}

#[test]
fn product_reads_descriptors_like_bazecors_restore() {
    let descriptor = serde_json::json!({
        "info": { "product": "Sonsei" }
    });

    assert_eq!(
        Product::from_descriptor(Some(&descriptor)).unwrap(),
        Product::Sonsei
    );

    assert!(matches!(
        Product::from_descriptor(None),
        Err(ConversionError::MissingDescriptor)
    ));

    let nameless = serde_json::json!({ "info": {} });

    assert!(matches!(
        Product::from_descriptor(Some(&nameless)),
        Err(ConversionError::NoProductName)
    ));

    let alien = serde_json::json!({ "info": { "product": "TypeMatrix" } });

    assert_eq!(
        Product::from_descriptor(Some(&alien)),
        Err(ConversionError::UnknownProduct {
            product: "TypeMatrix".to_owned()
        })
    );
}

#[test]
fn embedded_sonsei_descriptor_matches_the_shipped_backup() {
    // The embedded descriptor must stay byte-equal to the shipped
    // Sonsei backup's `neuron.device` — apart from `chipId`, which the
    // offline conversion empties (the target Neuron is unknown).
    let shipped = serde_json::from_str::<serde_json::Value>(include_str!(
        "../../../../tools/bazecor-differential/fixtures/Sonsei.json"
    ))
    .unwrap();
    let shipped = &shipped["neuron"]["device"];

    let mut ours = sonsei_device_descriptor();

    assert_eq!(ours["info"], shipped["info"]);
    assert_eq!(ours["usb"], shipped["usb"]);
    assert_eq!(ours["keyboard"], shipped["keyboard"]);
    assert_eq!(ours["keyboardUnderglow"], shipped["keyboardUnderglow"]);
    assert_eq!(ours["RGBWMode"], shipped["RGBWMode"]);
    assert_eq!(ours["hasRF"], shipped["hasRF"]);
    assert_eq!(ours["sides"], shipped["sides"]);
    assert_eq!(ours["components"], shipped["components"]);
    assert_eq!(ours["instructions"], shipped["instructions"]);

    ours["chipId"] = shipped["chipId"].clone();

    // With the chip id filled back in, the whole object is equal.
    assert_eq!(ours, *shipped);
}

#[test]
fn raise_keymap_tables_follow_the_bazecor_rotation_and_swaps() {
    // Raise 2 → Raise: new[69] = old[71], new[70] = old[69],
    // new[71] = old[70] (the thumb-cluster rotation), plus — ANSI only
    // — the Enter swap 31 ↔ 47 and the Shift swap 48 ↔ 49.
    let r2r_ansi = &RAISE2_TO_RAISE_KEYMAP_ANSI;

    assert_eq!(r2r_ansi[69], 71);
    assert_eq!(r2r_ansi[70], 69);
    assert_eq!(r2r_ansi[71], 70);
    assert_eq!(r2r_ansi[31], 47);
    assert_eq!(r2r_ansi[47], 31);
    assert_eq!(r2r_ansi[48], 49);
    assert_eq!(r2r_ansi[49], 48);
    assert_eq!(r2r_ansi[0], 0);
    assert_eq!(r2r_ansi[79], 79);

    let r2r_iso = &RAISE2_TO_RAISE_KEYMAP_ISO;

    assert_eq!(r2r_iso[69], 71);
    assert_eq!(r2r_iso[31], 31, "the ISO target takes no Enter swap");
    assert_eq!(r2r_iso[48], 48, "the ISO target takes no Shift swap");

    // Raise → Raise 2: the exact inverse rotation, same ANSI swaps.
    let r_r2_ansi = &RAISE_TO_RAISE2_KEYMAP_ANSI;

    assert_eq!(r_r2_ansi[69], 70);
    assert_eq!(r_r2_ansi[70], 71);
    assert_eq!(r_r2_ansi[71], 69);
    assert_eq!(r_r2_ansi[31], 47);
    assert_eq!(r_r2_ansi[48], 49);

    let r_r2_iso = &RAISE_TO_RAISE2_KEYMAP_ISO;

    assert_eq!(r_r2_iso[69], 70);
    assert_eq!(r_r2_iso[31], 31);

    // No Raise keymap position is ever without a source: rotation and
    // swaps are permutations of the 80 positions.
    for table in [
        &RAISE2_TO_RAISE_KEYMAP_ANSI,
        &RAISE2_TO_RAISE_KEYMAP_ISO,
        &RAISE_TO_RAISE2_KEYMAP_ANSI,
        &RAISE_TO_RAISE2_KEYMAP_ISO,
    ] {
        assert!(!table.contains(&NO_SOURCE_POSITION));
    }

    // And the rotations compose to identity: converting Raise 2 →
    // Raise → Raise 2 (or the other way) carries every position back —
    // g1[g2[t]] == t for the two directions' tables.
    for (down, up) in [
        (&RAISE2_TO_RAISE_KEYMAP_ANSI, &RAISE_TO_RAISE2_KEYMAP_ANSI),
        (&RAISE2_TO_RAISE_KEYMAP_ISO, &RAISE_TO_RAISE2_KEYMAP_ISO),
    ] {
        for (target, through) in up.iter().enumerate() {
            assert_eq!(
                down[*through as usize], target as i16,
                "position {target} does not round trip through the Raise 2 layer"
            );
        }
    }
}

#[test]
fn raise_colormap_tables_follow_the_bazecor_patterns() {
    // Raise 2 → Raise: positions 0–131 pass through identity, with the
    // Enter swap 40 ↔ 48 on an ANSI target and the Shift swap 19 ↔ 20
    // additionally when the backup's layout is ISO.
    let ansi_from_ansi = &RAISE2_TO_RAISE_COLORMAP_ANSI_FROM_ANSI;

    assert_eq!(ansi_from_ansi[40], 48);
    assert_eq!(ansi_from_ansi[48], 40);
    assert_eq!(ansi_from_ansi[19], 19, "an ANSI backup takes no Shift swap");
    assert_eq!(ansi_from_ansi[0], 0);
    assert_eq!(ansi_from_ansi[131], 131);

    let ansi_from_iso = &RAISE2_TO_RAISE_COLORMAP_ANSI_FROM_ISO;

    assert_eq!(ansi_from_iso[40], 48);
    assert_eq!(ansi_from_iso[19], 20);
    assert_eq!(ansi_from_iso[20], 19);

    let iso = &RAISE2_TO_RAISE_COLORMAP_ISO;

    for (position, source) in iso.iter().enumerate() {
        assert_eq!(*source, position as i16, "the ISO target truncates only");
    }

    // No Raise colormap target is ever without a source either: the
    // truncation keeps every position it emits and the swaps are
    // permutations.
    for table in [
        &RAISE2_TO_RAISE_COLORMAP_ANSI_FROM_ANSI,
        &RAISE2_TO_RAISE_COLORMAP_ANSI_FROM_ISO,
        &RAISE2_TO_RAISE_COLORMAP_ISO,
    ] {
        assert!(!table.contains(&NO_SOURCE_POSITION));
    }

    // Raise → Raise 2: the underglow interpolation. Keyboard LEDs
    // 0–68 pass through; left target 69 + i takes Raise source
    // 69 + floor(i × 30 / 53); right target 122 + i takes source
    // 99 + floor(i × 33 / 54).
    let r_r2 = &RAISE_TO_RAISE2_COLORMAP;

    assert_eq!(r_r2[0], 0);
    assert_eq!(r_r2[68], 68);
    assert_eq!(r_r2[69], 69);
    assert_eq!(r_r2[121], 69 + (52 * 30) / 53);
    assert_eq!(r_r2[122], 99);
    assert_eq!(r_r2[175], 99 + (53 * 33) / 54);

    // No target is without a source, and every source LED 0–131 is
    // read: the stretch intervals are wider than one target step.
    assert!(!r_r2.contains(&NO_SOURCE_POSITION));

    let referenced: std::collections::HashSet<i16> = r_r2.iter().copied().collect();

    assert_eq!(referenced.len(), RAISE_COLOR_LAYER_LEDS);

    for source in 0..RAISE_COLOR_LAYER_LEDS {
        assert!(
            referenced.contains(&(source as i16)),
            "Raise source LED {source} is never read"
        );
    }
}

#[test]
fn keyboard_type_reads_the_descriptor_spelling() {
    let ansi = serde_json::json!({ "info": { "keyboardType": "ANSI" } });
    let iso = serde_json::json!({ "info": { "keyboardType": "ISO" } });

    assert_eq!(
        KeyboardType::from_descriptor(Some(&ansi)).unwrap(),
        KeyboardType::Ansi
    );
    assert_eq!(
        KeyboardType::from_descriptor(Some(&iso)).unwrap(),
        KeyboardType::Iso
    );

    // Exact case, as shipped: the lowercase spellings are unknown.
    assert_eq!(
        KeyboardType::from_descriptor(Some(&serde_json::json!({
            "info": { "keyboardType": "ansi" }
        }))),
        Err(ConversionError::UnknownKeyboardType {
            keyboard_type: "ansi".to_owned()
        })
    );

    // Missing field and non-string values are the no-layout error.
    assert!(matches!(
        KeyboardType::from_descriptor(Some(&serde_json::json!({ "info": {} }))),
        Err(ConversionError::NoKeyboardType)
    ));
    assert!(matches!(
        KeyboardType::from_descriptor(Some(&serde_json::json!({
            "info": { "keyboardType": 3 }
        }))),
        Err(ConversionError::NoKeyboardType)
    ));
    assert!(matches!(
        KeyboardType::from_descriptor(None),
        Err(ConversionError::NoKeyboardType)
    ));

    // The Defy-family layout spellings are not Raise layouts.
    assert_eq!(
        KeyboardType::from_descriptor(Some(&serde_json::json!({
            "info": { "keyboardType": "wireless" }
        }))),
        Err(ConversionError::UnknownKeyboardType {
            keyboard_type: "wireless".to_owned()
        })
    );
}

#[test]
fn embedded_raise_descriptors_match_the_shipped_files() {
    // The embedded descriptors must stay byte-equal to the shipped
    // artifacts they transcribe — the Raise backups' `neuron.device`
    // (apart from `chipId`, emptied like the Sonsei's) and the Raise 2
    // virtuals' `device` objects verbatim (they carry no chip id).
    let raise_ansi = serde_json::from_str::<serde_json::Value>(include_str!(
        "../../../../tools/bazecor-differential/fixtures/RaiseANSI.json"
    ))
    .unwrap();
    let raise_iso = serde_json::from_str::<serde_json::Value>(include_str!(
        "../../../../tools/bazecor-differential/fixtures/RaiseISO.json"
    ))
    .unwrap();
    let raise2_ansi = serde_json::from_str::<serde_json::Value>(include_str!(
        "../../../../tools/bazecor-differential/fixtures/Raise2ANSI.json"
    ))
    .unwrap();
    let raise2_iso = serde_json::from_str::<serde_json::Value>(include_str!(
        "../../../../tools/bazecor-differential/fixtures/Raise2ISO.json"
    ))
    .unwrap();

    let mut ours = raise_device_descriptor(KeyboardType::Ansi);

    ours["chipId"] = raise_ansi["neuron"]["device"]["chipId"].clone();
    assert_eq!(ours, raise_ansi["neuron"]["device"]);

    let mut ours = raise_device_descriptor(KeyboardType::Iso);

    ours["chipId"] = raise_iso["neuron"]["device"]["chipId"].clone();
    assert_eq!(ours, raise_iso["neuron"]["device"]);

    // The Raise 2 virtuals carry no chip id — the descriptor is the
    // whole `device` object, verbatim.
    assert_eq!(
        raise2_device_descriptor(KeyboardType::Ansi),
        raise2_ansi["device"]
    );
    assert_eq!(
        raise2_device_descriptor(KeyboardType::Iso),
        raise2_iso["device"]
    );
}

#[test]
fn conversion_target_names_its_product_and_layout() {
    assert_eq!(ConversionTarget::Defy.product(), Product::Defy);
    assert_eq!(ConversionTarget::Sonsei.product(), Product::Sonsei);
    assert_eq!(
        ConversionTarget::Raise(KeyboardType::Iso).product(),
        Product::Raise
    );
    assert_eq!(
        ConversionTarget::Raise2(KeyboardType::Ansi).product(),
        Product::Raise2
    );

    assert_eq!(ConversionTarget::Defy.to_string(), "Defy");
    assert_eq!(
        ConversionTarget::Raise2(KeyboardType::Ansi).to_string(),
        "Raise 2 (ANSI)"
    );
}

#[test]
fn descriptor_underglow_width_reads_the_geometry_bazecor_parses_at() {
    let raise = serde_json::json!({
        "keyboardUnderglow": { "rows": 6, "columns": 22 }
    });

    assert_eq!(
        descriptor_underglow_width(Some(&raise)).unwrap(),
        RAISE_COLOR_LAYER_LEDS
    );

    // The Raise2 ISO virtual's geometry: 5 × 29.
    let raise2_iso = serde_json::json!({
        "keyboardUnderglow": { "rows": 5, "columns": 29 }
    });

    assert_eq!(descriptor_underglow_width(Some(&raise2_iso)).unwrap(), 145);

    // Missing, non-numeric, zero, overflowing, and absurd geometry are
    // all "no usable geometry" — the overflowing and absurd claims are
    // external input and must error, never panic or allocate from the
    // claimed width.
    assert!(matches!(
        descriptor_underglow_width(Some(&serde_json::json!({}))),
        Err(ConversionError::MissingUnderglowGeometry)
    ));
    assert!(matches!(
        descriptor_underglow_width(Some(&serde_json::json!({
            "keyboardUnderglow": { "rows": "six", "columns": 22 }
        }))),
        Err(ConversionError::MissingUnderglowGeometry)
    ));
    assert!(matches!(
        descriptor_underglow_width(Some(&serde_json::json!({
            "keyboardUnderglow": { "rows": 0, "columns": 0 }
        }))),
        Err(ConversionError::MissingUnderglowGeometry)
    ));
    assert!(matches!(
        descriptor_underglow_width(Some(&serde_json::json!({
            "keyboardUnderglow": {
                "rows": 9_223_372_036_854_775_808u64,
                "columns": 9_223_372_036_854_775_808u64
            }
        }))),
        Err(ConversionError::MissingUnderglowGeometry)
    ));
    assert!(matches!(
        descriptor_underglow_width(Some(&serde_json::json!({
            "keyboardUnderglow": { "rows": 1_000_000_000u64, "columns": 1_000_000_000u64 }
        }))),
        Err(ConversionError::MissingUnderglowGeometry)
    ));
    assert!(matches!(
        descriptor_underglow_width(None),
        Err(ConversionError::MissingUnderglowGeometry)
    ));
}

/// The differential tests: this module's conversion output compared
/// byte for byte against output recorded from Bazecor's own
/// conversion source on the shipped default backups.
mod differential {
    use super::*;
    use crate::devices::defy::DefyKeyboard;
    use crate::focus_api::parsing::backup::BackupCommand;

    /// A recorded expectation: what Bazecor's own conversion code
    /// produced for one shipped backup, plus the intermediate strings
    /// the conversion is made of.
    #[derive(serde::Deserialize)]
    struct DefyToSonseiExpectation {
        /// The Bazecor commit the expectation was recorded from.
        bazecor_commit: String,
        /// The shipped backup file the input was taken from.
        source_fixture: String,
        /// The converted `keymap.custom` data (10 × 60 codes).
        keymap_custom: String,
        /// The converted `colormap.map` data (10 × 56 LEDs).
        colormap_map: String,
        /// The converted `palette` data (16 × 3 channels).
        palette: String,
    }

    /// The real shipped Defy wireless backup, verbatim from
    /// Bazecor's `src/defaultBackups/`.
    fn defy_wireless_backup() -> DefyBackup {
        serde_json::from_str::<DefyBackup>(include_str!(
            "../../../../tools/bazecor-differential/fixtures/Defy-wireless-backup.json"
        ))
        .unwrap()
    }

    /// The real shipped Defy wired backup, verbatim.
    fn defy_wired_backup() -> DefyBackup {
        serde_json::from_str::<DefyBackup>(include_str!(
            "../../../../tools/bazecor-differential/fixtures/Defy-wired-backup.json"
        ))
        .unwrap()
    }

    /// The real shipped Sonsei backup, verbatim.
    fn sonsei_backup() -> DefyBackup {
        serde_json::from_str::<DefyBackup>(include_str!(
            "../../../../tools/bazecor-differential/fixtures/Sonsei.json"
        ))
        .unwrap()
    }

    /// Compares our conversion of one shipped Defy backup against
    /// Bazecor's recorded output, byte for byte.
    fn assert_matches_bazecor(backup: &DefyBackup, expectation: &DefyToSonseiExpectation) {
        // The provenance is part of the expectation: it must name the
        // Bazecor commit the recorded output came from.
        assert!(
            !expectation.bazecor_commit.is_empty()
                && expectation
                    .bazecor_commit
                    .chars()
                    .all(|c| c.is_ascii_hexdigit()),
            "the recorded commit should be a git sha, got {}",
            expectation.bazecor_commit
        );

        let (converted, report) = convert_defy_backup_to_sonsei(backup).unwrap();

        assert_eq!(report.from, Product::Defy);
        assert_eq!(report.to, Product::Sonsei);

        let entry = |command: &str| {
            converted
                .backup
                .iter()
                .find(|entry| entry.command == command)
                .unwrap()
        };

        assert_eq!(entry("keymap.custom").data, expectation.keymap_custom);
        assert_eq!(entry("colormap.map").data, expectation.colormap_map);
        assert_eq!(entry("palette").data, expectation.palette);

        // The capture order is kept: the entries stay at the positions
        // they were captured at.
        let source_position = |command: &str| {
            backup
                .backup
                .iter()
                .position(|entry| entry.command == command)
                .unwrap()
        };

        for command in ["keymap.custom", "colormap.map", "palette"] {
            let converted_position = converted
                .backup
                .iter()
                .position(|entry| entry.command == command)
                .unwrap();

            assert_eq!(converted_position, source_position(command));
        }

        // Everything else passed through verbatim.
        for entry in &converted.backup {
            if CONVERTED_COMMANDS.contains(&entry.command.as_str()) {
                continue;
            }

            let source = backup
                .backup
                .iter()
                .find(|source| source.command == entry.command)
                .unwrap();

            assert_eq!(entry.data, source.data);
        }

        // The neuron object travels with the conversion except the
        // device descriptor, which becomes the target's.
        assert_eq!(converted.neuron_id, backup.neuron_id);
        assert_eq!(converted.neuron.name, backup.neuron.name);
        assert_eq!(
            converted.neuron.device.as_ref().unwrap()["info"]["product"],
            "Sonsei"
        );
        assert_eq!(
            converted.neuron.device.as_ref().unwrap(),
            &sonsei_device_descriptor()
        );

        // The report matches the tables at full magnitudes.
        assert_eq!(report.keymap.layers, 10);
        assert_eq!(report.keymap.source_positions, DEFY_KEYMAP_LAYER_POSITIONS);
        assert_eq!(
            report.keymap.target_positions,
            SONSEI_KEYMAP_LAYER_POSITIONS
        );
        assert_eq!(report.keymap.mapped_per_layer(), 56);
        assert_eq!(report.keymap.defaulted_per_layer(), 4);
        assert_eq!(report.colormap.mapped_per_layer(), 56);
        assert_eq!(report.colormap.defaulted_per_layer(), 0);
        assert_eq!(report.palette, ColorChannelConversion::RgbwToRgb);

        // No Defy-type re-entry on this direction: no defaulted
        // position is rewritten, so the padding disclosure is empty.
        assert!(report.normalized_padding.is_empty());
    }

    #[test]
    fn defy_wireless_converts_exactly_like_bazecor() {
        let expectation: DefyToSonseiExpectation = serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/expectations/defy-wireless-to-sonsei.json"
        ))
        .unwrap();

        assert_eq!(expectation.source_fixture, "Defy-wireless-backup.json");

        assert_matches_bazecor(&defy_wireless_backup(), &expectation);
    }

    #[test]
    fn defy_wired_converts_exactly_like_bazecor() {
        let expectation: DefyToSonseiExpectation = serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/expectations/defy-wired-to-sonsei.json"
        ))
        .unwrap();

        assert_eq!(expectation.source_fixture, "Defy-wired-backup.json");

        assert_matches_bazecor(&defy_wired_backup(), &expectation);
    }

    #[test]
    fn sonsei_palette_converts_to_defy_rgbw_like_bazecors_rgb2w() {
        // The Sonsei → Defy palette direction has no Bazecor backup
        // conversion to mirror, but its color math is Bazecor's own
        // `rgb2w` (the RGB → RGBW conversion its Raise → Raise2
        // conversion uses). The expectation records what Bazecor's
        // `rgb2w` produces for the shipped Sonsei palette.
        #[derive(serde::Deserialize)]
        struct PaletteExpectation {
            bazecor_commit: String,
            palette: String,
        }

        let expectation: PaletteExpectation = serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/expectations/sonsei-palette-to-defy.json"
        ))
        .unwrap();

        // The provenance is part of the expectation: it must name the
        // Bazecor commit the recorded output came from.
        assert!(
            !expectation.bazecor_commit.is_empty()
                && expectation
                    .bazecor_commit
                    .chars()
                    .all(|c| c.is_ascii_hexdigit()),
            "the recorded commit should be a git sha, got {}",
            expectation.bazecor_commit
        );

        let (converted, report) = convert_sonsei_backup_to_defy(&sonsei_backup()).unwrap();

        let palette = converted
            .backup
            .iter()
            .find(|entry| entry.command == "palette")
            .unwrap();

        assert_eq!(palette.data, expectation.palette);
        assert_eq!(report.palette, ColorChannelConversion::RgbToRgbw);
    }

    #[test]
    fn sonsei_to_defy_output_is_a_valid_defy_restore() {
        let (converted, report) = convert_sonsei_backup_to_defy(&sonsei_backup()).unwrap();

        // The exactly-ten-layer proof: the converted keymap parses as
        // the Defy write shape, and the whole converted backup passes
        // the restore plan the guard would reject the unconverted
        // Sonsei file with.
        let keymap = converted
            .backup
            .iter()
            .find(|entry| entry.command == "keymap.custom")
            .unwrap();

        assert_eq!(keymap.data.split_whitespace().count(), 800);

        DefyKeyboard::backup_restore_plan(&converted).unwrap();

        // And the guard does reject the source, as it should.
        assert!(DefyKeyboard::backup_restore_plan(&sonsei_backup()).is_err());

        assert_eq!(report.from, Product::Sonsei);
        assert_eq!(report.to, Product::Defy);
        assert_eq!(report.keymap.mapped_per_layer(), 56);
        assert_eq!(report.keymap.defaulted_per_layer(), 24);
        assert_eq!(report.colormap.source_positions, SONSEI_COLOR_LAYER_LEDS);
        assert_eq!(report.colormap.target_positions, DEFY_COLOR_LAYER_LEDS);
        assert_eq!(report.colormap.mapped_per_layer(), 56);
        assert_eq!(report.colormap.defaulted_per_layer(), 122);
        assert_eq!(report.keymap.default_value, DefaultValue::Transparent);
        assert_eq!(
            report.colormap.default_value,
            DefaultValue::FirstPaletteColor
        );

        // The padding disclosure: the defaulted positions include the
        // Defy's ten wire-padding slots, and the report names exactly
        // those — the output writes them as `0`, the value this SDK's
        // keymap serializer has always emitted for padding, not the
        // transparent sentinel the table defaults to.
        assert_eq!(
            report.normalized_padding,
            [7, 8, 23, 24, 39, 40, 54, 55, 56, 57]
        );

        // And they really are `0` in the output, on every layer.
        for (position, token) in keymap.data.split_whitespace().enumerate() {
            if report
                .normalized_padding
                .contains(&((position % DEFY_KEYMAP_LAYER_POSITIONS) as u16))
            {
                assert_eq!(
                    token, "0",
                    "padding position {position} did not normalize to 0"
                );
            }
        }
    }

    #[test]
    fn mapped_positions_round_trip_through_both_directions() {
        // Defy → Sonsei → Defy: every Defy position the forward map
        // reads comes back with its original value — the inverse table
        // is the exact inverse, so a position survives iff it is
        // mapped, i.e. iff the Sonsei → Defy table writes it. (The
        // full round trip is lossy by design — the Defy positions the
        // Sonsei has no key for are defaulted — but the positions that
        // survive both maps must be exact.)
        let defy = defy_wireless_backup();
        let (to_sonsei, _) = convert_defy_backup_to_sonsei(&defy).unwrap();
        let (back_to_defy, _) = convert_sonsei_backup_to_defy(&to_sonsei).unwrap();

        let source = defy
            .backup
            .iter()
            .find(|entry| entry.command == "keymap.custom")
            .unwrap()
            .data
            .split_whitespace()
            .map(|token| token.parse::<u16>().unwrap())
            .collect::<Vec<_>>();
        let round_tripped = back_to_defy
            .backup
            .iter()
            .find(|entry| entry.command == "keymap.custom")
            .unwrap()
            .data
            .split_whitespace()
            .map(|token| token.parse::<u16>().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(source.len(), 800);
        assert_eq!(round_tripped.len(), 800);

        for (position, value) in source.iter().enumerate() {
            // The mapping tables are per-layer: index them with the
            // position inside the layer.
            if SONSEI_TO_DEFY_KEYMAP[position % DEFY_KEYMAP_LAYER_POSITIONS] != NO_SOURCE_POSITION {
                assert_eq!(
                    round_tripped[position], *value,
                    "Defy position {position} did not round trip"
                );
            } else {
                // Dropped or defaulted: the round trip either
                // normalized padding to `0` through the Defy keymap
                // type or defaulted the position to the transparent
                // sentinel.
                assert!(
                    round_tripped[position] == 0 || round_tripped[position] == TRANSPARENT_KEY_CODE,
                    "Defy position {position} changed from {value} to {}",
                    round_tripped[position]
                );
            }
        }
    }

    #[test]
    fn conversion_errors_are_typed_and_exact() {
        // Same product: nothing to convert.
        assert!(matches!(
            convert_backup(&defy_wireless_backup(), ConversionTarget::Defy),
            Err(ConversionError::SameProduct {
                product: Product::Defy
            })
        ));

        // Raise backups are recognized but not convertible yet.
        let mut raise = defy_wireless_backup();
        raise.neuron.device = Some(serde_json::json!({
            "info": { "product": "Raise" }
        }));

        assert_eq!(
            convert_backup(&raise, ConversionTarget::Defy).unwrap_err(),
            ConversionError::UnsupportedConversion {
                from: Product::Raise,
                to: Product::Defy
            }
        );

        // A backup missing one of the converted commands (Bazecor's
        // conversion crashes here instead).
        let mut skeleton = sonsei_backup();
        skeleton.backup.retain(|entry| entry.command != "palette");

        assert_eq!(
            convert_backup(&skeleton, ConversionTarget::Defy).unwrap_err(),
            ConversionError::MissingCommand { command: "palette" }
        );

        // A duplicated converted command is refused where Bazecor's
        // findIndex would silently take the first and leave the
        // duplicate unconverted in the output.
        let mut duplicated = sonsei_backup();
        let first_keymap = duplicated
            .backup
            .iter()
            .find(|entry| entry.command == "keymap.custom")
            .unwrap()
            .clone();
        duplicated.backup.push(BackupCommand::new(
            "keymap.custom".to_owned(),
            format!("{} 1", first_keymap.data),
        ));

        assert_eq!(
            convert_backup(&duplicated, ConversionTarget::Defy).unwrap_err(),
            ConversionError::DuplicateCommand {
                command: "keymap.custom",
                count: 2
            }
        );

        // A keymap that is not the source product's shape (the Defy
        // wireless keymap held as if it were Sonsei data) fails the
        // shape check with the found/expected counts.
        let mut mismatched = sonsei_backup();
        mismatched.neuron.device = Some(serde_json::json!({
            "info": { "product": "Sonsei" }
        }));

        let defy_keymap = defy_wireless_backup()
            .backup
            .iter()
            .find(|entry| entry.command == "keymap.custom")
            .unwrap()
            .data
            .clone();

        for entry in &mut mismatched.backup {
            if entry.command == "keymap.custom" {
                entry.data = defy_keymap.clone();
            }
        }

        match convert_backup(&mismatched, ConversionTarget::Defy) {
            Err(ConversionError::InvalidEntry {
                command,
                source: LayeredWireError { kind, .. },
            }) => {
                assert_eq!(command, "keymap.custom");
                assert_eq!(
                    kind,
                    LayeredWireErrorKind::WrongValueCount {
                        found: 800,
                        expected: 600
                    }
                );
            }
            other => panic!("expected InvalidEntry, got {other:?}"),
        }
    }

    #[test]
    fn descriptor_replacement_names_the_target_product() {
        let (to_sonsei, _) = convert_defy_backup_to_sonsei(&defy_wireless_backup()).unwrap();
        let (to_defy, _) = convert_sonsei_backup_to_defy(&sonsei_backup()).unwrap();

        // The Sonsei → Defy output carries the Defy descriptor with
        // the identity fields empty, so it passes the product guard.
        assert_eq!(
            to_defy.neuron.device.as_ref().unwrap(),
            &DefyKeyboard::backup_device_descriptor(true, "")
        );
        assert_eq!(
            to_defy.neuron.device.as_ref().unwrap()["info"]["product"],
            "Defy"
        );

        // The Defy → Sonsei output carries the Sonsei descriptor.
        assert_eq!(
            to_sonsei.neuron.device.as_ref().unwrap(),
            &sonsei_device_descriptor()
        );

        // Names and the neuron id are the source's — the conversion
        // rewrites wire data, not identities.
        assert_eq!(to_sonsei.neuron_id, defy_wireless_backup().neuron_id);
        assert_eq!(to_sonsei.neuron.name, defy_wireless_backup().neuron.name);
    }

    #[test]
    fn a_partial_capture_stays_marked_through_a_conversion() {
        // The marker is data, not damage: converting a partial capture
        // rewrites wire shapes, never adds the missing commands, so
        // the converted file keeps the marker (and its missing list)
        // verbatim — it must not become unmistakable for complete.
        let mut partial = defy_wireless_backup();

        partial.partial = Some(crate::focus_api::parsing::backup::PartialCapture {
            missing: vec!["led.fade".to_owned(), "mouse.speed".to_owned()],
        });

        let (converted, _) = convert_defy_backup_to_sonsei(&partial).unwrap();

        assert_eq!(
            converted.partial,
            Some(crate::focus_api::parsing::backup::PartialCapture {
                missing: vec!["led.fade".to_owned(), "mouse.speed".to_owned()],
            })
        );
        assert!(converted.is_partial());

        // The control: converting the unmarked source stays unmarked.
        let (complete, _) = convert_defy_backup_to_sonsei(&defy_wireless_backup()).unwrap();

        assert!(complete.partial.is_none());
        assert!(!complete.is_partial());
    }

    #[test]
    fn a_partial_capture_stays_marked_through_the_sonsei_to_defy_conversion() {
        // The reverse direction of the carry-through above: the
        // conversion's shared `backup.clone()` makes both directions
        // safe today, but a future per-field rewrite of the Sonsei →
        // Defy path could silently drop the marker on the way into
        // Defy shape — so the marker (and its missing list) is pinned
        // through this direction too.
        let mut partial = sonsei_backup();

        partial.partial = Some(crate::focus_api::parsing::backup::PartialCapture {
            missing: vec!["led.fade".to_owned(), "mouse.speed".to_owned()],
        });

        let (converted, _) = convert_sonsei_backup_to_defy(&partial).unwrap();

        assert_eq!(
            converted.partial,
            Some(crate::focus_api::parsing::backup::PartialCapture {
                missing: vec!["led.fade".to_owned(), "mouse.speed".to_owned()],
            })
        );
        assert!(converted.is_partial());

        // The control: converting the unmarked source stays unmarked.
        let (complete, _) = convert_sonsei_backup_to_defy(&sonsei_backup()).unwrap();

        assert!(complete.partial.is_none());
        assert!(!complete.is_partial());
    }

    /// A recorded Raise-pair expectation: what Bazecor's own Raise
    /// conversion code produced for one shipped fixture and target
    /// layout, plus the provenance and the keyboardType arguments the
    /// recording passed.
    #[derive(serde::Deserialize)]
    struct RaiseExpectation {
        /// The Bazecor commit the expectation was recorded from.
        bazecor_commit: String,
        /// The shipped fixture the input was taken from.
        source_fixture: String,
        /// The source backup's `keyboardType`.
        source_keyboard_type: String,
        /// The target layout the conversion ran against.
        target_keyboard_type: String,
        /// Present only on the Raise2ISO outlier entry: why the
        /// recorded output is not a target for equality.
        outlier: Option<String>,
        /// Present only on the stamped-source entry: what the
        /// recording's `keyboardType` stamp means.
        #[serde(default)]
        stamped_source: Option<String>,
        /// The converted `keymap.custom` data (10 × 80 codes).
        keymap_custom: String,
        /// The converted `colormap.map` data (10 × 176 or 10 × 132
        /// LEDs).
        colormap_map: String,
        /// The converted `palette` data (16 × 4 or 16 × 3 channels).
        palette: String,
    }

    /// The real shipped Raise ANSI backup, verbatim from Bazecor's
    /// `src/defaultBackups/`.
    fn raise_ansi_backup() -> DefyBackup {
        serde_json::from_str::<DefyBackup>(include_str!(
            "../../../../tools/bazecor-differential/fixtures/RaiseANSI.json"
        ))
        .unwrap()
    }

    /// The real shipped Raise ISO backup, verbatim.
    fn raise_iso_backup() -> DefyBackup {
        serde_json::from_str::<DefyBackup>(include_str!(
            "../../../../tools/bazecor-differential/fixtures/RaiseISO.json"
        ))
        .unwrap()
    }

    /// The shipped Raise 2 ANSI hardware-virtual definition, verbatim
    /// (`{device, virtual}` — not a backup).
    fn raise2_ansi_virtual() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/fixtures/Raise2ANSI.json"
        ))
        .unwrap()
    }

    /// The shipped Raise 2 ISO hardware-virtual definition, verbatim.
    fn raise2_iso_virtual() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/fixtures/Raise2ISO.json"
        ))
        .unwrap()
    }

    /// Synthesizes the backup-shape input a Raise 2 conversion reads:
    /// the virtual's `device` descriptor becomes `neuron.device` (it
    /// carries both the `keyboardUnderglow` geometry and the source
    /// `keyboardType`), and the three conversion commands' data
    /// strings travel verbatim. The same synthesis the recording
    /// harness performs and documents.
    fn raise2_virtual_as_backup(virtual_kb: &serde_json::Value) -> DefyBackup {
        let command = |name: &str| {
            BackupCommand::new(
                name.to_owned(),
                virtual_kb["virtual"][name]["data"]
                    .as_str()
                    .expect("the virtual carries the command's data string")
                    .to_owned(),
            )
        };

        DefyBackup {
            neuron: crate::focus_api::parsing::backup::BackupNeuron {
                device: Some(virtual_kb["device"].clone()),
                ..Default::default()
            },
            backup: vec![
                command("keymap.custom"),
                command("colormap.map"),
                command("palette"),
            ],
            ..Default::default()
        }
    }

    /// Compares our conversion of one Raise-pair source against
    /// Bazecor's recorded output, byte for byte.
    fn assert_raise_matches_bazecor(
        backup: &DefyBackup,
        target: ConversionTarget,
        expectation: &RaiseExpectation,
    ) {
        // The provenance is part of the expectation: it must name the
        // Bazecor commit the recorded output came from.
        assert!(
            !expectation.bazecor_commit.is_empty()
                && expectation
                    .bazecor_commit
                    .chars()
                    .all(|c| c.is_ascii_hexdigit()),
            "the recorded commit should be a git sha, got {}",
            expectation.bazecor_commit
        );
        assert!(
            expectation.outlier.is_none(),
            "the outlier entry is pinned by its own test, not output equality"
        );

        let (converted, report) = convert_backup(backup, target).unwrap();

        assert_eq!(
            report.from,
            Product::from_descriptor(backup.neuron.device.as_ref()).unwrap()
        );
        assert_eq!(report.to, target.product());

        let entry = |command: &str| {
            converted
                .backup
                .iter()
                .find(|entry| entry.command == command)
                .unwrap()
        };

        assert_eq!(entry("keymap.custom").data, expectation.keymap_custom);
        assert_eq!(entry("colormap.map").data, expectation.colormap_map);
        assert_eq!(entry("palette").data, expectation.palette);

        // The capture order is kept: the entries stay at the positions
        // they were captured at.
        let source_position = |command: &str| {
            backup
                .backup
                .iter()
                .position(|entry| entry.command == command)
                .unwrap()
        };

        for command in ["keymap.custom", "colormap.map", "palette"] {
            let converted_position = converted
                .backup
                .iter()
                .position(|entry| entry.command == command)
                .unwrap();

            assert_eq!(converted_position, source_position(command));
        }

        // Everything else passed through verbatim.
        for entry in &converted.backup {
            if CONVERTED_COMMANDS.contains(&entry.command.as_str()) {
                continue;
            }

            let source = backup
                .backup
                .iter()
                .find(|source| source.command == entry.command)
                .unwrap();

            assert_eq!(entry.data, source.data);
        }

        // The descriptor is the transcribed target descriptor for the
        // requested layout, and the neuron identity is the source's.
        let expected_descriptor = match target {
            ConversionTarget::Raise2(keyboard_type) => raise2_device_descriptor(keyboard_type),
            ConversionTarget::Raise(keyboard_type) => raise_device_descriptor(keyboard_type),
            other => panic!("not a Raise-pair target: {other:?}"),
        };

        assert_eq!(converted.neuron_id, backup.neuron_id);
        assert_eq!(converted.neuron.name, backup.neuron.name);
        assert_eq!(
            converted.neuron.device.as_ref().unwrap(),
            &expected_descriptor
        );

        // No Raise target is the Defy: no padding normalization story.
        assert!(report.normalized_padding.is_empty());
    }

    #[test]
    fn raise_ansi_converts_to_raise2_exactly_like_bazecor() {
        let expectation: RaiseExpectation = serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/expectations/raiseansi-to-raise2-ansi.json"
        ))
        .unwrap();

        assert_eq!(expectation.source_fixture, "RaiseANSI.json");
        assert_eq!(expectation.target_keyboard_type, "ANSI");

        assert_raise_matches_bazecor(
            &raise_ansi_backup(),
            ConversionTarget::Raise2(KeyboardType::Ansi),
            &expectation,
        );
    }

    #[test]
    fn raise_ansi_converts_to_raise2_iso_exactly_like_bazecor() {
        let expectation: RaiseExpectation = serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/expectations/raiseansi-to-raise2-iso.json"
        ))
        .unwrap();

        assert_eq!(expectation.source_fixture, "RaiseANSI.json");
        assert_eq!(expectation.target_keyboard_type, "ISO");

        assert_raise_matches_bazecor(
            &raise_ansi_backup(),
            ConversionTarget::Raise2(KeyboardType::Iso),
            &expectation,
        );
    }

    #[test]
    fn raise_iso_converts_to_raise2_exactly_like_bazecor() {
        let expectation: RaiseExpectation = serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/expectations/raiseiso-to-raise2-ansi.json"
        ))
        .unwrap();

        assert_eq!(expectation.source_fixture, "RaiseISO.json");
        assert_eq!(expectation.target_keyboard_type, "ANSI");

        assert_raise_matches_bazecor(
            &raise_iso_backup(),
            ConversionTarget::Raise2(KeyboardType::Ansi),
            &expectation,
        );
    }

    #[test]
    fn raise_iso_converts_to_raise2_iso_exactly_like_bazecor() {
        let expectation: RaiseExpectation = serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/expectations/raiseiso-to-raise2-iso.json"
        ))
        .unwrap();

        assert_eq!(expectation.source_fixture, "RaiseISO.json");
        assert_eq!(expectation.target_keyboard_type, "ISO");

        assert_raise_matches_bazecor(
            &raise_iso_backup(),
            ConversionTarget::Raise2(KeyboardType::Iso),
            &expectation,
        );
    }

    #[test]
    fn raise2_ansi_converts_to_raise_exactly_like_bazecor() {
        let expectation: RaiseExpectation = serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/expectations/raise2ansi-to-raise-ansi.json"
        ))
        .unwrap();

        assert_eq!(expectation.source_fixture, "Raise2ANSI.json");
        assert_eq!(expectation.target_keyboard_type, "ANSI");
        assert_eq!(expectation.source_keyboard_type, "ANSI");

        let backup = raise2_virtual_as_backup(&raise2_ansi_virtual());

        assert_raise_matches_bazecor(
            &backup,
            ConversionTarget::Raise(KeyboardType::Ansi),
            &expectation,
        );
    }

    #[test]
    fn raise2_ansi_converts_to_raise_iso_exactly_like_bazecor() {
        let expectation: RaiseExpectation = serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/expectations/raise2ansi-to-raise-iso.json"
        ))
        .unwrap();

        assert_eq!(expectation.source_fixture, "Raise2ANSI.json");
        assert_eq!(expectation.target_keyboard_type, "ISO");

        let backup = raise2_virtual_as_backup(&raise2_ansi_virtual());

        assert_raise_matches_bazecor(
            &backup,
            ConversionTarget::Raise(KeyboardType::Iso),
            &expectation,
        );
    }

    #[test]
    fn raise2_ansi_stamped_iso_converts_exactly_like_bazecor() {
        // The from-ISO colormap branch (the Shift swap the backup's
        // layout keys on an ANSI target) has no shipped fixture with
        // self-consistent geometry — the Raise2ISO virtual's colormap
        // is the ragged outlier — so the recording stamps the
        // Raise2ANSI synthesized backup's `info.keyboardType` to "ISO"
        // (exactly the field Bazecor reads as `backupKeyboardType`)
        // and runs the same mirror. The shipped ANSI virtual's data
        // carries equal indices at LEDs 19/20 of every layer, so the
        // recorded strings are byte-identical to the from-ANSI entry:
        // this pins that the stamped argument flows through Bazecor's
        // branch leaving everything else untouched. The swap's visible
        // effect is pinned by the perturbed-data property test below.
        let expectation: RaiseExpectation = serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/expectations/raise2ansi-stamped-iso-to-raise-ansi.json"
        ))
        .unwrap();

        assert!(expectation.stamped_source.is_some());
        assert_eq!(expectation.source_keyboard_type, "ISO");
        assert_eq!(expectation.target_keyboard_type, "ANSI");

        let mut backup = raise2_virtual_as_backup(&raise2_ansi_virtual());

        backup.neuron.device.as_mut().unwrap()["info"]["keyboardType"] = serde_json::json!("ISO");

        assert_raise_matches_bazecor(
            &backup,
            ConversionTarget::Raise(KeyboardType::Ansi),
            &expectation,
        );

        // Elsewhere-identical: the keymap and palette are the same
        // strings the from-ANSI recording carries — only the colormap
        // swap could differ, and on this data it does not.
        let plain: RaiseExpectation = serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/expectations/raise2ansi-to-raise-ansi.json"
        ))
        .unwrap();

        assert_eq!(expectation.keymap_custom, plain.keymap_custom);
        assert_eq!(expectation.colormap_map, plain.colormap_map);
        assert_eq!(expectation.palette, plain.palette);
    }

    #[test]
    fn the_from_iso_colormap_swap_exchanges_leds_19_and_20() {
        // The swap's visible effect, on data where it is not a no-op:
        // the shipped Raise2ANSI colormap carries equal indices at
        // LEDs 19/20 of every layer, so layer 0's source position 19 is
        // perturbed to a distinct palette index. With an ANSI target,
        // the from-ISO conversion must then output the from-ANSI
        // conversion's colormap with positions 19 and 20 exchanged in
        // every layer — the tables read target 19 from source 20 and
        // target 20 from source 19 — and the keymaps must be
        // identical (the backup's layout never touches them).
        let perturbed = |keyboard_type: &str| {
            let mut backup = raise2_virtual_as_backup(&raise2_ansi_virtual());

            backup.neuron.device.as_mut().unwrap()["info"]["keyboardType"] =
                serde_json::json!(keyboard_type);

            for entry in &mut backup.backup {
                if entry.command == "colormap.map" {
                    let mut tokens: Vec<String> =
                        entry.data.split_whitespace().map(str::to_owned).collect();

                    // Palette index 9 at source LED 19 of layer 0,
                    // where the shipped data carries the same 2 that
                    // LED 20 does.
                    tokens[19] = "9".to_owned();

                    entry.data = tokens.join(" ");
                }
            }

            backup
        };

        let (from_ansi, _) =
            convert_raise2_backup_to_raise(&perturbed("ANSI"), KeyboardType::Ansi).unwrap();
        let (from_iso, _) =
            convert_raise2_backup_to_raise(&perturbed("ISO"), KeyboardType::Ansi).unwrap();

        let colormap = |backup: &DefyBackup| {
            backup
                .backup
                .iter()
                .find(|entry| entry.command == "colormap.map")
                .unwrap()
                .data
                .split_whitespace()
                .map(|token| token.parse::<u16>().unwrap())
                .collect::<Vec<_>>()
        };

        let ansi = colormap(&from_ansi);
        let iso = colormap(&from_iso);

        assert_eq!(ansi.len(), 10 * RAISE_COLOR_LAYER_LEDS);
        assert_eq!(iso.len(), ansi.len());

        for layer in 0..10 {
            let base = layer * RAISE_COLOR_LAYER_LEDS;

            for position in 0..RAISE_COLOR_LAYER_LEDS {
                let expected = match position {
                    19 => ansi[base + 20],
                    20 => ansi[base + 19],
                    _ => ansi[base + position],
                };

                assert_eq!(
                    iso[base + position],
                    expected,
                    "layer {layer} position {position}: the from-ISO map is not the from-ANSI map with 19/20 exchanged"
                );
            }
        }

        // And the perturbation really reached the output: the ANSI
        // map's layer-0 LED 19 is the planted 9 (its neighbor 20 is
        // the shipped 2), so the exchange above is not vacuous.
        assert_eq!(ansi[19], 9);
        assert_eq!(ansi[20], 2);
        assert_eq!(iso[19], 2);
        assert_eq!(iso[20], 9);

        let keymap = |backup: &DefyBackup| {
            backup
                .backup
                .iter()
                .find(|entry| entry.command == "keymap.custom")
                .unwrap()
                .data
                .clone()
        };

        assert_eq!(keymap(&from_ansi), keymap(&from_iso));
    }

    #[test]
    fn raise2_iso_ragged_colormap_is_rejected_where_bazecor_records_garbage() {
        // The shipped-data outlier: the Raise2ISO virtual's
        // colormap.map holds 1440 tokens (10 × 144) against its own
        // descriptor geometry of 5 × 29 = 145 per layer. The recorded
        // OUTLIER entry (raise2iso-to-raise-outlier.json) holds what
        // Bazecor makes of the ragged input — ten misaligned layers —
        // and is documentation, not an equality target: this crate's
        // exact-count shape check refuses the input instead (the same
        // documented-deviation class as the wireless channelHop
        // capture). No bound is widened to match it.
        let expectation: RaiseExpectation = serde_json::from_str(include_str!(
            "../../../../tools/bazecor-differential/expectations/raise2iso-to-raise-outlier.json"
        ))
        .unwrap();

        assert_eq!(expectation.source_fixture, "Raise2ISO.json");
        assert!(
            expectation.outlier.is_some(),
            "the entry must carry its outlier note"
        );

        let backup = raise2_virtual_as_backup(&raise2_iso_virtual());

        assert_eq!(
            convert_backup(&backup, ConversionTarget::Raise(KeyboardType::Ansi)).unwrap_err(),
            ConversionError::InvalidEntry {
                command: "colormap.map",
                source: LayeredWireError {
                    layer_size: 145,
                    layer_count: 10,
                    max_value: u8::MAX as u32,
                    kind: LayeredWireErrorKind::WrongValueCount {
                        found: 1440,
                        expected: 1450,
                    },
                },
            }
        );
    }

    #[test]
    fn raise_keymap_round_trips_through_both_directions() {
        // Raise 2 → Raise → Raise 2 on the real Raise2 ANSI data, for
        // both target layouts: the keymap tables compose to identity,
        // so the keymap data comes back byte for byte. (The colormap
        // and palette are lossy by design — the interpolation cannot
        // recover dropped LEDs and the RGBW round trip folds the white
        // channel into the colors — so only the keymap is pinned.)
        let raise2 = raise2_virtual_as_backup(&raise2_ansi_virtual());
        let raise2_keymap = raise2
            .backup
            .iter()
            .find(|entry| entry.command == "keymap.custom")
            .unwrap()
            .data
            .clone();

        for keyboard_type in [KeyboardType::Ansi, KeyboardType::Iso] {
            let (to_raise, _) = convert_raise2_backup_to_raise(&raise2, keyboard_type).unwrap();
            let (back_to_raise2, _) =
                convert_raise_backup_to_raise2(&to_raise, keyboard_type).unwrap();

            assert_eq!(
                back_to_raise2
                    .backup
                    .iter()
                    .find(|entry| entry.command == "keymap.custom")
                    .unwrap()
                    .data,
                raise2_keymap,
                "the Raise 2 → Raise → Raise 2 keymap round trip diverged at {keyboard_type}"
            );
        }

        // And the other way on the real Raise ANSI data.
        let raise = raise_ansi_backup();
        let raise_keymap = raise
            .backup
            .iter()
            .find(|entry| entry.command == "keymap.custom")
            .unwrap()
            .data
            .clone();

        for keyboard_type in [KeyboardType::Ansi, KeyboardType::Iso] {
            let (to_raise2, _) = convert_raise_backup_to_raise2(&raise, keyboard_type).unwrap();

            // The intermediate is Raise2-shaped data wearing the
            // verbatim Raise 2 descriptor for the requested layout —
            // and the shipped Raise 2 ISO descriptor's underglow
            // geometry (5 × 29) disagrees with the Raise 2 colormap
            // shape (176 per layer), the same shipped-evidence
            // inconsistency the Raise2ISO outlier pins. The round trip
            // here is about the KEYMAP transformation, so the
            // intermediate is re-stamped with the geometry-consistent
            // ANSI virtual's descriptor before converting back;
            // re-converting the ISO-stamped file is (correctly)
            // rejected by the shape check.
            let mut intermediate = to_raise2;

            intermediate.neuron.device = Some(raise2_ansi_virtual()["device"].clone());

            let (back_to_raise, _) =
                convert_raise2_backup_to_raise(&intermediate, keyboard_type).unwrap();

            assert_eq!(
                back_to_raise
                    .backup
                    .iter()
                    .find(|entry| entry.command == "keymap.custom")
                    .unwrap()
                    .data,
                raise_keymap,
                "the Raise → Raise 2 → Raise keymap round trip diverged at {keyboard_type}"
            );
        }
    }

    #[test]
    fn raise_conversion_reports_carry_the_exact_mapping_story() {
        // Raise → Raise 2: every source LED is read (nothing dropped),
        // the underglow stretches 132 → 176, and the palette gains the
        // white channel.
        let (to_raise2, report) =
            convert_raise_backup_to_raise2(&raise_ansi_backup(), KeyboardType::Ansi).unwrap();

        assert_eq!(report.from, Product::Raise);
        assert_eq!(report.to, Product::Raise2);
        assert_eq!(report.keymap.layers, 10);
        assert_eq!(report.keymap.source_positions, RAISE_KEYMAP_LAYER_POSITIONS);
        assert_eq!(report.keymap.target_positions, RAISE_KEYMAP_LAYER_POSITIONS);
        assert_eq!(report.keymap.mapped_per_layer(), 80);
        assert_eq!(report.keymap.defaulted_per_layer(), 0);
        assert_eq!(report.colormap.source_positions, RAISE_COLOR_LAYER_LEDS);
        assert_eq!(report.colormap.target_positions, RAISE2_COLOR_LAYER_LEDS);
        assert_eq!(report.colormap.mapped_per_layer(), 176);
        assert_eq!(report.colormap.defaulted_per_layer(), 0);
        assert!(report.colormap.dropped_sources.is_empty());
        assert_eq!(report.palette, ColorChannelConversion::RgbToRgbw);

        // Full-magnitude output, per the recorded shapes.
        let entry = |backup: &DefyBackup, command: &str| {
            backup
                .backup
                .iter()
                .find(|entry| entry.command == command)
                .unwrap()
                .data
                .split_whitespace()
                .count()
        };

        assert_eq!(entry(&to_raise2, "keymap.custom"), 800);
        assert_eq!(entry(&to_raise2, "colormap.map"), 1760);
        assert_eq!(entry(&to_raise2, "palette"), 64);

        // Raise 2 → Raise: the Raise 2 layer truncates back to 132,
        // and the source LEDs 132–176 are the dropped story.
        let raise2 = raise2_virtual_as_backup(&raise2_ansi_virtual());
        let (to_raise, report) =
            convert_raise2_backup_to_raise(&raise2, KeyboardType::Ansi).unwrap();

        assert_eq!(report.colormap.source_positions, RAISE2_COLOR_LAYER_LEDS);
        assert_eq!(report.colormap.target_positions, RAISE_COLOR_LAYER_LEDS);
        assert_eq!(report.colormap.mapped_per_layer(), 132);
        assert_eq!(report.colormap.defaulted_per_layer(), 0);
        assert_eq!(
            report.colormap.dropped_sources,
            (RAISE_COLOR_LAYER_LEDS as u16..RAISE2_COLOR_LAYER_LEDS as u16).collect::<Vec<_>>()
        );
        assert_eq!(report.palette, ColorChannelConversion::RgbwToRgb);

        assert_eq!(entry(&to_raise, "keymap.custom"), 800);
        assert_eq!(entry(&to_raise, "colormap.map"), 1320);
        assert_eq!(entry(&to_raise, "palette"), 48);
    }

    #[test]
    fn raise_typed_errors_are_exact() {
        // A Raise-pair source without usable underglow geometry: the
        // colormap source width is unknown. (Bazecor has no working
        // answer here either — its converters' `: 132` / `: 176`
        // fallbacks are unreachable through restoreBackup, whose
        // dispatch dereferences the descriptor first, so the converter
        // throws TypeError on `undefined.rows`.)
        let mut no_geometry = raise2_virtual_as_backup(&raise2_ansi_virtual());

        no_geometry
            .neuron
            .device
            .as_mut()
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("keyboardUnderglow");

        assert_eq!(
            convert_raise2_backup_to_raise(&no_geometry, KeyboardType::Ansi).unwrap_err(),
            ConversionError::MissingUnderglowGeometry
        );

        // A Raise 2 source whose layout is not spelled ANSI/ISO:
        // Bazecor's `backupKeyboardType === "ISO"` check would
        // silently treat this as ANSI, skipping the Shift swap.
        let mut foreign_layout = raise2_virtual_as_backup(&raise2_ansi_virtual());

        foreign_layout.neuron.device.as_mut().unwrap()["info"]["keyboardType"] =
            serde_json::json!("wireless");

        assert_eq!(
            convert_raise2_backup_to_raise(&foreign_layout, KeyboardType::Ansi).unwrap_err(),
            ConversionError::UnknownKeyboardType {
                keyboard_type: "wireless".to_owned(),
            }
        );

        // And a source without the field at all.
        let mut unnamed_layout = raise2_virtual_as_backup(&raise2_ansi_virtual());

        unnamed_layout.neuron.device.as_mut().unwrap()["info"]
            .as_object_mut()
            .unwrap()
            .remove("keyboardType");

        assert!(matches!(
            convert_raise2_backup_to_raise(&unnamed_layout, KeyboardType::Ansi),
            Err(ConversionError::NoKeyboardType)
        ));

        // Missing and duplicated converted commands, through the new
        // pair (the same machinery the Defy ↔ Sonsei pairs run).
        let mut missing = raise2_virtual_as_backup(&raise2_ansi_virtual());

        missing.backup.retain(|entry| entry.command != "palette");

        assert_eq!(
            convert_raise2_backup_to_raise(&missing, KeyboardType::Ansi).unwrap_err(),
            ConversionError::MissingCommand { command: "palette" }
        );

        let mut duplicated = raise2_virtual_as_backup(&raise2_ansi_virtual());
        let first_keymap = duplicated
            .backup
            .iter()
            .find(|entry| entry.command == "keymap.custom")
            .unwrap()
            .clone();
        duplicated.backup.push(BackupCommand::new(
            "keymap.custom".to_owned(),
            first_keymap.data,
        ));

        assert_eq!(
            convert_raise2_backup_to_raise(&duplicated, KeyboardType::Ansi).unwrap_err(),
            ConversionError::DuplicateCommand {
                command: "keymap.custom",
                count: 2,
            }
        );

        // Same-generation conversion is refused — a Raise 2 source
        // asked for a Raise 2 target, layout notwithstanding.
        let raise2 = raise2_virtual_as_backup(&raise2_ansi_virtual());

        assert_eq!(
            convert_backup(&raise2, ConversionTarget::Raise2(KeyboardType::Ansi)).unwrap_err(),
            ConversionError::SameProduct {
                product: Product::Raise2
            }
        );

        // The unsupported pairs keep their typed answer — a Raise
        // backup onto the Defy has no table to reach.
        assert_eq!(
            convert_backup(&raise_ansi_backup(), ConversionTarget::Defy).unwrap_err(),
            ConversionError::UnsupportedConversion {
                from: Product::Raise,
                to: Product::Defy,
            }
        );
    }

    #[test]
    fn a_descriptor_narrower_than_the_tables_read_is_rejected() {
        // Hand-crafted self-consistent-but-wrong input: a Raise backup
        // whose descriptor claims a 4 × 29 = 116-LED colormap layer,
        // with its colormap.map truncated to exactly 10 × 116 tokens.
        // Every shape check this crate runs would pass — and then the
        // colormap tables (which read source LEDs 0–131) would index
        // past the layer's end. Bazecor converts such input anyway: a
        // Raise 2 → Raise layer comes out truncated, and the Raise →
        // Raise 2 interpolator's `?: 15` default fires past the
        // layer's end. The guard refuses it with the typed error
        // instead.
        let mut lying = raise_ansi_backup();

        lying.neuron.device.as_mut().unwrap()["keyboardUnderglow"] =
            serde_json::json!({ "rows": 4, "columns": 29 });

        let colormap_data = lying
            .backup
            .iter()
            .find(|entry| entry.command == "colormap.map")
            .unwrap()
            .data
            .clone();
        let truncated: Vec<&str> = colormap_data.split_whitespace().take(10 * 116).collect();

        for entry in &mut lying.backup {
            if entry.command == "colormap.map" {
                entry.data = truncated.join(" ");
            }
        }

        assert_eq!(
            convert_raise_backup_to_raise2(&lying, KeyboardType::Ansi).unwrap_err(),
            ConversionError::UnderglowGeometryTooNarrow {
                claimed: 116,
                needed: RAISE_COLOR_LAYER_LEDS,
            }
        );

        // And the reverse direction, whose truncation tables read the
        // same 132-LED span.
        let mut lying_raise2 = raise2_virtual_as_backup(&raise2_ansi_virtual());

        lying_raise2.neuron.device.as_mut().unwrap()["keyboardUnderglow"] =
            serde_json::json!({ "rows": 1, "columns": 100 });

        lying_raise2.backup.iter_mut().for_each(|entry| {
            if entry.command == "colormap.map" {
                let tokens: Vec<&str> = entry.data.split_whitespace().take(10 * 100).collect();

                entry.data = tokens.join(" ");
            }
        });

        assert_eq!(
            convert_raise2_backup_to_raise(&lying_raise2, KeyboardType::Ansi).unwrap_err(),
            ConversionError::UnderglowGeometryTooNarrow {
                claimed: 100,
                needed: RAISE_COLOR_LAYER_LEDS,
            }
        );

        // The control: geometry at exactly the span (the shipped 6 ×
        // 22 = 132) still converts, tokens and all.
        let (converted, report) =
            convert_raise_backup_to_raise2(&raise_ansi_backup(), KeyboardType::Ansi).unwrap();

        assert_eq!(report.colormap.source_positions, RAISE_COLOR_LAYER_LEDS);
        assert_eq!(
            converted
                .backup
                .iter()
                .find(|entry| entry.command == "colormap.map")
                .unwrap()
                .data
                .split_whitespace()
                .count(),
            10 * RAISE2_COLOR_LAYER_LEDS
        );
    }

    #[test]
    fn absurd_underglow_geometries_are_typed_errors_reaching_no_allocation() {
        // Crafted external input must hit typed errors, never a panic
        // or an allocation sized from the claim: a rows × columns that
        // overflows a u64 would panic the product in a debug build, and
        // a huge-but-representable product used to be preallocated as
        // the per-layer capacity in the shape check — an allocation
        // abort before a single token was examined. Both are refused as
        // unusable geometry now.
        for [rows, columns] in [
            [9_223_372_036_854_775_808u64, 9_223_372_036_854_775_808u64],
            [1_000_000_000u64, 1_000_000_000u64],
        ] {
            let mut absurd = raise2_virtual_as_backup(&raise2_ansi_virtual());

            absurd.neuron.device.as_mut().unwrap()["keyboardUnderglow"] =
                serde_json::json!({ "rows": rows, "columns": columns });

            assert_eq!(
                convert_raise2_backup_to_raise(&absurd, KeyboardType::Ansi).unwrap_err(),
                ConversionError::MissingUnderglowGeometry
            );
        }
    }
}
