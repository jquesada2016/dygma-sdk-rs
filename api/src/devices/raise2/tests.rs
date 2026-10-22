//! Tests for the Raise 2 format-level model: wire shapes through the
//! typed maps, the physical layout, and the offline restore-plan
//! validation against the real shipped artifacts.

use super::*;
use crate::devices::conversions::{self, ConversionTarget};
use crate::devices::defaults::DefaultConfig;
use crate::focus_api::parsing::backup::{BackupCommand, BackupNeuron};

/// The shipped Raise 2 ANSI RECORD, verbatim (`{device, virtual}` —
/// the app's simulated factory device; not a backup — no real-device
/// Raise 2 capture exists). The fixtures are the records; the
/// internally-consistent form both layouts' colormaps ride on is the
/// hardware-virtual DEFINITION the recording recorded (see the
/// module docs).
const RAISE2_ANSI_RECORD_JSON: &str =
    include_str!("../../../../tools/bazecor-differential/fixtures/Raise2ANSI.json");

/// The shipped Raise 2 ISO record, verbatim — its colormap is the
/// drifted 1440-token copy (see the module docs).
const RAISE2_ISO_RECORD_JSON: &str =
    include_str!("../../../../tools/bazecor-differential/fixtures/Raise2ISO.json");

/// The shipped Raise ANSI backup fixture — the source for the
/// cross-generation conversion proofs.
const RAISE_ANSI_BACKUP_JSON: &str =
    include_str!("../../../../tools/bazecor-differential/fixtures/RaiseANSI.json");

/// Synthesizes the backup-shape input a Raise 2 validator reads: the
/// virtual's `device` descriptor becomes `neuron.device`, and all
/// five config commands' data strings travel verbatim (the same
/// synthesis the recording harness performs and documents — extended
/// here to the superkeys and macros maps, so the validator runs at
/// the virtuals' full magnitude).
fn raise2_virtual_as_backup(virtual_kb: &serde_json::Value) -> parsing::backup::DefyBackup {
    let command = |name: &str| {
        BackupCommand::new(
            name.to_owned(),
            virtual_kb["virtual"][name]["data"]
                .as_str()
                .expect("the virtual carries the command's data string")
                .to_owned(),
        )
    };

    parsing::backup::DefyBackup {
        neuron: BackupNeuron {
            device: Some(virtual_kb["device"].clone()),
            ..Default::default()
        },
        backup: vec![
            command("keymap.custom"),
            command("superkeys.map"),
            command("macros.map"),
            command("palette"),
            command("colormap.map"),
        ],
        ..Default::default()
    }
}

#[test]
fn the_layout_models_seventy_three_keys_and_leaves_seven_slots() {
    // The descriptor rows total 73; the unmodeled slots are exactly
    // the Raise's seven (the wire rows are shared across the
    // generations).
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
fn the_ansi_record_validates_at_full_magnitude() {
    // The shipped ANSI record — the self-consistent Raise 2 artifact
    // — validates whole through the Raise 2 validator: every shaped
    // entry parses through the Raise 2 types (the 800-token keymap,
    // 512-token superkeys, 2048-token macros, 64-decimal RGBW
    // palette, 1760-token colormap), and the plan is the complete
    // write sequence. (The fixtures are the shipped RECORDS; the
    // ISO record's drifted colormap is the next test's subject.)
    let virtual_kb: serde_json::Value = serde_json::from_str(RAISE2_ANSI_RECORD_JSON).unwrap();
    let backup = raise2_virtual_as_backup(&virtual_kb);

    let plan = backup_restore_plan(&backup).expect("the Raise 2 record validates");

    assert_eq!(plan.len(), backup.backup.len() + 1); // + the `led.mode 0` tail
    assert_eq!(plan.last().unwrap().command, "led.mode");
    assert_eq!(plan.last().unwrap().data, "0");
}

#[test]
fn the_drifted_iso_record_colormap_is_typed_rejected() {
    // The shipped ISO RECORD's colormap — 1440 tokens (10 × 144)
    // against the 176-LED layer every other Raise 2 artifact holds —
    // is not a Raise 2 colormap: the typed parse refuses the ragged
    // shape (the recorded OUTLIER; the definition's 1760-token form
    // is what the app serves and what the defaults ship). A whole
    // backup carrying it fails validation on that entry, by name.
    let iso_record: serde_json::Value = serde_json::from_str(RAISE2_ISO_RECORD_JSON).unwrap();

    let drifted = iso_record["virtual"]["colormap.map"]["data"]
        .as_str()
        .expect("the record carries the colormap data");

    assert_eq!(drifted.split_whitespace().count(), 1440);
    assert!(Raise2Colormap::from_str(drifted).is_err());

    let backup = raise2_virtual_as_backup(&iso_record);

    assert!(matches!(
        backup_restore_plan(&backup).unwrap_err(),
        BackupValidationError::InvalidEntry { command, .. } if command == "colormap.map"
    ));

    // The same file with the consistent ISO colormap (the definition
    // data the defaults module embeds — the same string the defaults
    // tests pin to the recorded definition) validates: the drift is
    // the colormap record's, not the file's shape.
    let DefaultConfig::Raise2(consistent) = crate::devices::defaults::default_config(
        ConversionTarget::Raise2(conversions::KeyboardType::Iso),
    ) else {
        panic!("the Raise 2 answers a Raise 2 default config");
    };

    let mut repaired = raise2_virtual_as_backup(&iso_record);

    repaired
        .backup
        .iter_mut()
        .find(|entry| entry.command == "colormap.map")
        .expect("the backup carries a colormap")
        .data = consistent.colormap.to_colormap_map_data();

    backup_restore_plan(&repaired).expect("the repaired file validates");
}

#[test]
fn wrong_shapes_are_rejected_by_name() {
    // The Raise 2 validator's shaped entries reject the OTHER
    // products' shapes: the Raise's 132-LED colormap layers, the
    // Defy's 178-LED ones, the Sonsei's 56-LED ones, and the Raise's
    // 48-decimal RGB palette against the Raise 2's 64-decimal RGBW
    // shape.
    let raise_width: String = std::iter::repeat_n("4", 1320).collect::<Vec<_>>().join(" ");

    assert!(raise_width.parse::<Raise2Colormap>().is_err());

    let nine_raise2_layers: String = std::iter::repeat_n("4", 9 * 176)
        .collect::<Vec<_>>()
        .join(" ");

    assert!(matches!(
        Raise2Colormap::from_str(&nine_raise2_layers),
        Err(ParseColormapError::IncorrectLayerCount(_))
    ));

    // A raise colormap entry fails the Raise 2 colormap parse (the
    // real 1320-token data, not just a synthetic stream).
    let raise_backup: parsing::backup::DefyBackup = serde_json::from_str(include_str!(
        "../../../../tools/bazecor-differential/fixtures/RaiseANSI.json"
    ))
    .unwrap();

    let raise_colormap = raise_backup
        .backup
        .iter()
        .find(|entry| entry.command == "colormap.map")
        .expect("the Raise backup carries a colormap")
        .data
        .clone();

    assert!(raise_colormap.parse::<Raise2Colormap>().is_err());

    // The Raise's RGB palette (48 decimals) is not an RGBW palette
    // (64).
    let raise_palette = raise_backup
        .backup
        .iter()
        .find(|entry| entry.command == "palette")
        .expect("the Raise backup carries a palette")
        .data
        .clone();

    assert!(parsing::palette::Palette::from_str(&raise_palette).is_err());
}

#[test]
fn the_converted_backups_revalidate_through_the_raise_generation_validators() {
    // The cross-unit property, both directions: a backup converted by
    // the conversions unit's Raise↔Raise2 recipes validates whole
    // through the TARGET product's validator. (The Raise2→Raise
    // direction sources from the synthesized virtual backups — the
    // same inputs the conversions unit's differential tests convert.)
    let raise_backup: parsing::backup::DefyBackup =
        serde_json::from_str(RAISE_ANSI_BACKUP_JSON).expect("the shipped Raise backup parses");

    // Raise → Raise 2: the converted file validates as a Raise 2.
    let (converted, _) = conversions::convert_backup(
        &raise_backup,
        ConversionTarget::Raise2(conversions::KeyboardType::Ansi),
    )
    .expect("the Raise backup converts");

    crate::devices::raise2::backup_restore_plan(&converted)
        .expect("the converted Raise 2 backup validates as a Raise 2");

    // Raise 2 → Raise: the converted file validates as a Raise. The
    // synthesis carries the record's `device` descriptor, so the
    // conversion reads its geometry; the conversion strips to the
    // three rewritten commands, and the Raise validator plans it.
    // (The ISO record is NOT a source here: its drifted 1440-token
    // colormap is the conversions unit's pinned OUTLIER — the
    // recorded `raise2iso-to-raise-outlier` expectation — refused
    // before any conversion happens.)
    let virtual_kb: serde_json::Value = serde_json::from_str(RAISE2_ANSI_RECORD_JSON).unwrap();
    let source = raise2_virtual_as_backup(&virtual_kb);

    let (converted, _) = conversions::convert_backup(
        &source,
        ConversionTarget::Raise(conversions::KeyboardType::Ansi),
    )
    .expect("the Raise 2 record converts");

    let plan = crate::devices::raise::backup_restore_plan(&converted)
        .expect("the converted Raise backup validates as a Raise");

    assert_eq!(plan.len(), converted.backup.len() + 1);
}
