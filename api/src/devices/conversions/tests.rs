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
            convert_backup(&defy_wireless_backup(), Product::Defy),
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
            convert_backup(&raise, Product::Defy).unwrap_err(),
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
            convert_backup(&skeleton, Product::Defy).unwrap_err(),
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
            convert_backup(&duplicated, Product::Defy).unwrap_err(),
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

        match convert_backup(&mismatched, Product::Defy) {
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
}
