//! Tests for the Sonsei format-level model: wire shapes through the
//! typed maps, the physical layout, and the offline restore-plan
//! validation against the real shipped artifacts.
//!
//! The fixtures are the harness's byte-identity-verified copies of
//! Bazecor's shipped files (`tools/bazecor-differential/fixtures/`),
//! so every full-size parse below runs on real captured data, never a
//! hand-truncated fragment.

use super::*;

/// The shipped Sonsei backup fixture (a byte-identical copy of
/// `bazecor/src/defaultBackups/Sonsei.json`): a real device capture —
/// 600-token keymap, 560-token colormap, 48-decimal palette, 4
/// superkeys, 2 macros, plus the empty-command capture artifact.
const SONSEI_BACKUP_JSON: &str =
    include_str!("../../../../tools/bazecor-differential/fixtures/Sonsei.json");

/// The shipped Defy wireless backup fixture (a byte-identical copy of
/// `bazecor/src/defaultBackups/Defy-wireless-backup.json`) — the
/// Defy-side source of the cross-unit conversion test.
const DEFY_WIRELESS_BACKUP_JSON: &str =
    include_str!("../../../../tools/bazecor-differential/fixtures/Defy-wireless-backup.json");

/// One real layer of the shipped Sonsei backup's `keymap.custom`
/// (layer 1): 60 wire positions, real keys in the thumb row at both
/// 48–51 and 56–59, `0`s in the four unmodeled slots between.
const REAL_LAYER_ONE: &str = "41 30 31 32 33 34 35 36 37 38 39 42 43 20 26 8 21 23 28 24 12 18 19 40 53983 4 22 7 9 10 11 13 14 15 51 52 225 29 27 6 25 5 17 16 54 55 56 229 224 227 44 53981 0 0 0 0 53980 44 53982 54109";

/// The shipped backup's keymap data, split into its ten real layers.
fn real_layers() -> Vec<String> {
    let backup: parsing::backup::DefyBackup =
        serde_json::from_str(SONSEI_BACKUP_JSON).expect("the shipped Sonsei backup parses");

    let keymap = &backup
        .backup
        .iter()
        .find(|entry| entry.command == KEYMAP_CUSTOM_COMMAND_NAME)
        .expect("the shipped backup captures keymap.custom")
        .data;

    let tokens: Vec<&str> = keymap.split_whitespace().collect();

    assert_eq!(tokens.len(), 10 * KEYS_PER_LAYER);

    (0..10)
        .map(|layer| tokens[layer * KEYS_PER_LAYER..(layer + 1) * KEYS_PER_LAYER].join(" "))
        .collect()
}

#[test]
fn geometry_constants_match_the_conversion_modules() {
    // One geometry, two spellings: the conversions module's table
    // shapes and this module's wire constants must never drift.
    assert_eq!(
        KEYS_PER_LAYER,
        crate::devices::conversions::SONSEI_KEYMAP_LAYER_POSITIONS
    );
    assert_eq!(
        LEDS_PER_LAYER,
        crate::devices::conversions::SONSEI_COLOR_LAYER_LEDS
    );
}

#[test]
fn parses_the_real_ten_layer_keymap_and_round_trips() {
    let backup: parsing::backup::DefyBackup =
        serde_json::from_str(SONSEI_BACKUP_JSON).expect("the shipped Sonsei backup parses");
    let data = &backup
        .backup
        .iter()
        .find(|entry| entry.command == KEYMAP_CUSTOM_COMMAND_NAME)
        .expect("the shipped backup captures keymap.custom")
        .data;

    let keymap: SonseiKeymap = data.parse().expect("the real keymap parses");

    assert_eq!(keymap.layers.len(), 10);

    // The write form matches the recorded data on every MODELED key
    // and normalizes the four unmodeled slots (52–55) to `0` — the
    // recorded layers 2–10 carry `65535` there, the value a real
    // device leaves in padding, and the serializer emits `0`, the
    // value this crate's keymap serializer has always emitted. The
    // exact delta is pinned below (the defaults unit's pattern).
    let ours = keymap.to_keymap_custom_data();
    let ours: Vec<&str> = ours.split_whitespace().collect();
    let recorded: Vec<&str> = data.split_whitespace().collect();

    assert_eq!(ours.len(), recorded.len());

    for layer in 0..10 {
        for position in 0..KEYS_PER_LAYER {
            let index = layer * KEYS_PER_LAYER + position;

            if (52..56).contains(&position) {
                assert_eq!(ours[index], "0", "layer {layer}, unmodeled slot {position}");
                assert!(
                    recorded[index] == "0" || recorded[index] == "65535",
                    "layer {layer}, slot {position}: unexpected recorded padding value"
                );
            } else {
                assert_eq!(
                    ours[index], recorded[index],
                    "layer {layer}, wire position {position} is a modeled \
                     key and must match the recorded data"
                );
            }
        }
    }

    // The recorded output of the Defy → Sonsei conversion (the
    // conversions unit's differentially-validated shape) parses into
    // the same type — one shape, one parse.
    let converted: SonseiKeymap = {
        let source: parsing::backup::DefyBackup = serde_json::from_str(DEFY_WIRELESS_BACKUP_JSON)
            .expect("the shipped Defy wireless backup parses");
        let (converted, _) = crate::devices::conversions::convert_defy_backup_to_sonsei(&source)
            .expect("the shipped Defy backup converts");

        let data = &converted
            .backup
            .iter()
            .find(|entry| entry.command == KEYMAP_CUSTOM_COMMAND_NAME)
            .expect("the converted backup captures keymap.custom")
            .data;

        data.parse().expect("the converted keymap parses")
    };

    assert_eq!(converted.layers.len(), 10);
}

#[test]
fn the_real_layer_maps_every_thumb_key_through_the_layout() {
    // One real layer, ten times: a keymap-shaped stream to parse.
    let data = std::iter::repeat_n(REAL_LAYER_ONE, 10)
        .collect::<Vec<_>>()
        .join(" ");
    let layer: SonseiKeymapLayer = data.parse::<SonseiKeymap>().unwrap().layers[0];

    // The thumb row's real keys land in the human shape: the left
    // cluster (wire 48–51) and the right cluster (wire 56–59).
    assert_eq!(layer.thumb_row.left[0], KeyKind::from(224));
    assert_eq!(layer.thumb_row.left[3], KeyKind::from(53981));
    assert_eq!(layer.thumb_row.right[0], KeyKind::from(53980));
    assert_eq!(layer.thumb_row.right[3], KeyKind::from(54109));

    // The four unmodeled slots (52–55) hold nothing human: the wire
    // `0`s there are answered by `None`.
    let wire = layer.to_keymap_data();

    for position in [52, 53, 54, 55] {
        assert!(wire[position].is_none(), "position {position} is unmodeled");
    }

    // And exactly four slots are unmodeled — 56 modeled keys.
    assert_eq!(wire.iter().filter(|key| key.is_none()).count(), 4);

    // The wire round trip is lossless for this layer: the human shape
    // carries every wire value at its modeled position.
    let layer_data: SonseiLayerData = REAL_LAYER_ONE
        .split_whitespace()
        .map(|token| KeyKind::from(token.parse::<u16>().unwrap()))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    assert_eq!(SonseiKeymapLayer::from(&layer_data), layer);

    for (position, key) in layer_data.iter().enumerate() {
        if let Some(human) = layer.to_keymap_data()[position] {
            assert_eq!(human, *key, "wire position {position}");
        }
    }
}

#[test]
fn wrong_keymap_shapes_reject_with_typed_errors() {
    // A Defy-shaped keymap — the shipped Defy backup's real
    // 800-token `keymap.custom` — is valid wire data at the wrong
    // width: the 60-wide parse cannot close layer 14 from the 20
    // remaining tokens.
    let defy_backup: parsing::backup::DefyBackup =
        serde_json::from_str(DEFY_WIRELESS_BACKUP_JSON).expect("the shipped Defy backup parses");

    let defy_shaped = &defy_backup
        .backup
        .iter()
        .find(|entry| entry.command == KEYMAP_CUSTOM_COMMAND_NAME)
        .expect("the shipped Defy backup captures keymap.custom")
        .data;

    assert_eq!(defy_shaped.split_whitespace().count(), 800);

    let err = defy_shaped.parse::<SonseiKeymap>().unwrap_err();

    assert!(matches!(
        err,
        ParseKeymapError::MalformedData(parsing::keymap::ParseKeymapError { .. })
    ));

    // Nine real layers: valid wire data, not a keymap write.
    let nine = real_layers()[..9].join(" ");
    let err = nine.parse::<SonseiKeymap>().unwrap_err();

    assert!(matches!(
        err,
        ParseKeymapError::IncorrectLayerCount(KeymapDoesNotHave10LayersError)
    ));

    // Eleven layers too.
    let mut eleven = real_layers();
    eleven.push(eleven[0].clone());
    let eleven = eleven.join(" ");

    assert!(matches!(
        eleven.parse::<SonseiKeymap>(),
        Err(ParseKeymapError::IncorrectLayerCount(_))
    ));

    // A token past u16 fails the wire parse.
    let mut tokens: Vec<String> = real_layers()[0]
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    tokens[0] = "65536".to_owned();
    let data = std::iter::repeat_n(tokens.join(" "), 10)
        .collect::<Vec<_>>()
        .join(" ");

    assert!(matches!(
        data.parse::<SonseiKeymap>(),
        Err(ParseKeymapError::MalformedData(_))
    ));
}

#[test]
fn keymap_json_round_trips_with_layer_numbers() {
    let keymap: SonseiKeymap = real_layers().join(" ").parse().unwrap();

    let json = serde_json::to_string_pretty(&keymap).unwrap();

    // The file form: ten layers with 1-based `layer_number` UX aids.
    assert!(json.contains(r#""layer_number": 1,"#));
    assert!(json.contains(r#""layer_number": 10,"#));
    assert!(json.contains(r#""thumb_row""#));

    let reparsed: SonseiKeymap = serde_json::from_str(&json).unwrap();

    assert_eq!(reparsed, keymap);

    // A file whose layer numbers are garbage reads the same: the
    // numbers are write-only UX aids.
    let relabeled = json.replace(r#""layer_number": 1,"#, r#""layer_number": 7,"#);
    let relabeled: SonseiKeymap = serde_json::from_str(&relabeled).unwrap();

    assert_eq!(relabeled, keymap);

    // A wrong layer count cannot exist as the type.
    let json = serde_json::to_string(&keymap.layers.to_vec()[..9].to_vec()).unwrap();

    assert!(serde_json::from_str::<SonseiKeymap>(&json).is_err());
}

#[test]
fn parses_the_real_colormap_and_palette_and_round_trips() {
    let backup: parsing::backup::DefyBackup =
        serde_json::from_str(SONSEI_BACKUP_JSON).expect("the shipped Sonsei backup parses");

    let colormap_data = &backup
        .backup
        .iter()
        .find(|entry| entry.command == COLORMAP_MAP_COMMAND_NAME)
        .expect("the shipped backup captures colormap.map")
        .data;
    let palette_data = &backup
        .backup
        .iter()
        .find(|entry| entry.command == PALETTE_COMMAND_NAME)
        .expect("the shipped backup captures palette")
        .data;

    let colormap: SonseiColormap = colormap_data.parse().expect("the real colormap parses");

    assert_eq!(colormap.layers.len(), 10);
    assert_eq!(colormap.to_colormap_map_data(), colormap_data.trim());

    // Spot-check the real capture: layer 1 paints LED 0 with palette
    // color 4 (the Sonsei's silver) and its thumb row reaches the
    // darker accents (8 at LED 55).
    assert_eq!(colormap.layers[0].keyboard[0].get(), 4);
    assert_eq!(colormap.layers[0].keyboard[55].get(), 8);

    let palette: parsing::palette::RgbPalette =
        palette_data.parse().expect("the real palette parses");

    assert_eq!(palette.0.len(), 16);
    assert_eq!(palette.to_string(), palette_data.trim());

    // Color 0 is the white the Sonsei factory theme frames its keys
    // with; the recorded colormap's default color for converted
    // positions.
    assert_eq!(
        palette.0[0],
        parsing::palette::RgbColor {
            r: 255,
            g: 255,
            b: 255
        }
    );

    // The colormap JSON file round trips (56 fits the derived serde
    // arrays, unlike the Defy's hand-rolled 70+108 split).
    let json = serde_json::to_string_pretty(&colormap).unwrap();
    let reparsed: SonseiColormap = serde_json::from_str(&json).unwrap();

    assert_eq!(reparsed, colormap);
}

#[test]
fn wrong_colormap_and_palette_shapes_reject_with_typed_errors() {
    // A Defy-shaped colormap: ten layers of 178 — valid wire data, the
    // wrong width (the 56-wide parse runs out of layer mid-stream).
    let layer = "4 0 0 0 0 ".repeat(178);
    let defy_shaped = layer.repeat(10);

    assert!(matches!(
        defy_shaped.parse::<SonseiColormap>(),
        Err(ParseColormapError::MalformedData(_))
    ));

    // Nine real layers: not a write.
    let backup: parsing::backup::DefyBackup =
        serde_json::from_str(SONSEI_BACKUP_JSON).expect("the shipped Sonsei backup parses");
    let data = &backup
        .backup
        .iter()
        .find(|entry| entry.command == COLORMAP_MAP_COMMAND_NAME)
        .unwrap()
        .data;
    let tokens: Vec<&str> = data.split_whitespace().collect();

    let nine = tokens[..9 * LEDS_PER_LAYER].join(" ");

    assert!(matches!(
        nine.parse::<SonseiColormap>(),
        Err(ParseColormapError::IncorrectLayerCount(_))
    ));

    // An index past the palette's 16 colors fails the wire parse (the
    // same nibble-domain rule the Defy's map enforces — the palette
    // is 16 colors on every product).
    let mut bad: Vec<String> = tokens[..LEDS_PER_LAYER]
        .iter()
        .map(|s| s.to_string())
        .collect();
    bad[0] = "16".to_owned();
    let bad = std::iter::repeat_n(bad.join(" "), 10)
        .collect::<Vec<_>>()
        .join(" ");

    assert!(matches!(
        bad.parse::<SonseiColormap>(),
        Err(ParseColormapError::MalformedData(_))
    ));

    // A Defy RGBW palette (64 decimals) is not an RGB palette.
    let rgbw = "255 196 0 0 ".repeat(16);

    assert!(matches!(
        rgbw.parse::<parsing::palette::RgbPalette>(),
        Err(parsing::palette::ParseRgbPaletteError::WrongColorCount { found: 64 })
    ));

    // 47 decimals (one channel short) is not either: the recorded
    // palette's own tokens, one short.
    let recorded_palette = &backup
        .backup
        .iter()
        .find(|entry| entry.command == PALETTE_COMMAND_NAME)
        .unwrap()
        .data;
    let short = recorded_palette
        .split_whitespace()
        .take(parsing::palette::RGB_PALETTE_DECIMALS - 1)
        .collect::<Vec<_>>()
        .join(" ");

    assert!(matches!(
        short.parse::<parsing::palette::RgbPalette>(),
        Err(parsing::palette::ParseRgbPaletteError::WrongColorCount { found: 47 })
    ));
}

#[test]
fn the_shipped_sonsei_backup_validates_whole() {
    let backup: parsing::backup::DefyBackup =
        serde_json::from_str(SONSEI_BACKUP_JSON).expect("the shipped Sonsei backup parses");

    // The real capture — every product-shaped entry at full size —
    // validates through the Sonsei types, and the plan is Bazecor's
    // restore order plus the `led.mode 0` tail.
    let plan = backup_restore_plan(&backup).expect("the shipped Sonsei backup validates");

    let commands: Vec<&str> = plan.iter().map(|entry| entry.command.as_str()).collect();

    // One more than the capture (the empty-command artifact is
    // dropped, `led.mode` appended).
    assert_eq!(plan.len(), backup.backup.len());
    assert_eq!(commands.last(), Some(&"led.mode"));
    assert!(!commands.contains(&""));

    // The superkeys land before the keymap (Bazecor's reorder).
    let superkeys = commands
        .iter()
        .position(|command| *command == "superkeys.map")
        .unwrap();
    let keymap = commands
        .iter()
        .position(|command| *command == KEYMAP_CUSTOM_COMMAND_NAME)
        .unwrap();

    assert!(superkeys < keymap);
}

#[test]
fn a_converted_defy_backup_revalidates_through_the_sonsei_types() {
    // The cross-unit integration: the conversions unit's output — a
    // full Defy wireless backup converted to the Sonsei shape — is a
    // valid Sonsei restore target by this module's own validation,
    // with no Sonsei-specific fixing in between.
    let source: parsing::backup::DefyBackup =
        serde_json::from_str(DEFY_WIRELESS_BACKUP_JSON).expect("the shipped Defy backup parses");

    let (converted, _) = crate::devices::conversions::convert_defy_backup_to_sonsei(&source)
        .expect("the shipped Defy backup converts");

    let plan =
        backup_restore_plan(&converted).expect("the converted backup validates as a Sonsei backup");

    // The shipped Defy capture carries no empty-command artifacts, so
    // the plan is every entry plus the `led.mode 0` tail.
    assert_eq!(plan.len(), converted.backup.len() + 1);
    assert_eq!(plan.last().unwrap().command, "led.mode");
}

#[test]
fn cross_product_backups_are_refused_by_name() {
    // The guard, mirrored: a Defy backup is not a Sonsei backup, and
    // the Sonsei validator says so the same way the Defy's says the
    // reverse.
    let defy_backup: parsing::backup::DefyBackup =
        serde_json::from_str(DEFY_WIRELESS_BACKUP_JSON).expect("the shipped Defy backup parses");

    let err = backup_restore_plan(&defy_backup).unwrap_err();

    assert!(matches!(
        err,
        BackupValidationError::NotASonseiBackup { ref product } if product == "Defy"
    ));

    // And a descriptor-less backup passes the guard (mirroring the
    // Defy validator: only a READABLE foreign product is refused).
    let mut anonymous = defy_backup.clone();
    anonymous.neuron.device = None;

    // (It then fails on the Defy-shaped data — the guard is not the
    // only check.)
    assert!(matches!(
        backup_restore_plan(&anonymous),
        Err(BackupValidationError::InvalidEntry { .. })
    ));
}

#[test]
fn wrong_shaped_entries_reject_with_typed_errors_in_context() {
    let backup: parsing::backup::DefyBackup =
        serde_json::from_str(SONSEI_BACKUP_JSON).expect("the shipped Sonsei backup parses");

    // A Defy-shaped keymap inside a Sonsei backup fails on the entry,
    // naming the command.
    let mut wrong = backup.clone();
    wrong
        .backup
        .iter_mut()
        .find(|entry| entry.command == KEYMAP_CUSTOM_COMMAND_NAME)
        .unwrap()
        .data = "41 30 31 ".repeat(267) + "41"; // 800 tokens: Defy-shaped

    let err = backup_restore_plan(&wrong).unwrap_err();

    match err {
        BackupValidationError::InvalidEntry { command, source } => {
            assert_eq!(command, KEYMAP_CUSTOM_COMMAND_NAME);
            assert!(matches!(source, BackupEntryError::Keymap(_)));
        }
        other => panic!("expected an invalid entry, got {other:?}"),
    }

    // A partial capture is refused with its missing list.
    let mut partial = backup.clone();
    partial.partial = Some(parsing::backup::PartialCapture {
        missing: vec!["palette".to_owned()],
    });

    assert!(matches!(
        backup_restore_plan(&partial).unwrap_err(),
        BackupValidationError::PartialCapture { ref missing }
            if missing == &["palette".to_owned()]
    ));

    // The override plans it.
    assert_eq!(
        backup_restore_plan_allowing_partial(&partial)
            .expect("the override plans a partial capture")
            .len(),
        partial.backup.len()
    );
}
