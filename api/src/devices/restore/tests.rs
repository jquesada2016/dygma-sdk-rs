//! Tests for the product-dispatched restore-plan seam: the dispatch
//! itself, against the real shipped fixtures.

use super::*;
use crate::focus_api::parsing;

/// The shipped Defy wireless backup fixture (the harness's
/// byte-identity-verified copy of Bazecor's
/// `Defy-wireless-backup.json`) — the Defy-side dispatch case.
fn defy_backup() -> parsing::backup::DefyBackup {
    serde_json::from_str(include_str!(
        "../../../../tools/bazecor-differential/fixtures/Defy-wireless-backup.json"
    ))
    .expect("the shipped Defy wireless backup parses")
}

/// The shipped Sonsei backup fixture — the Sonsei-side dispatch case.
fn sonsei_backup() -> parsing::backup::DefyBackup {
    serde_json::from_str(include_str!(
        "../../../../tools/bazecor-differential/fixtures/Sonsei.json"
    ))
    .expect("the shipped Sonsei backup parses")
}

/// The shipped Raise ANSI backup fixture — the Raise-side dispatch
/// case (a real device capture: 800-token keymap, 1320-token
/// colormap, 48-decimal palette, padded superkeys form).
fn raise_backup() -> parsing::backup::DefyBackup {
    serde_json::from_str(include_str!(
        "../../../../tools/bazecor-differential/fixtures/RaiseANSI.json"
    ))
    .expect("the shipped Raise ANSI backup parses")
}

#[test]
fn a_defy_backup_plans_identically_through_the_dispatch() {
    // The dispatch adds nothing to a Defy backup's plan: product
    // dispatch and direct Defy validation answer the same sequence.
    let backup = defy_backup();

    let dispatched = backup_restore_plan(&backup).expect("the Defy backup dispatches");
    let direct = DefyKeyboard::backup_restore_plan(&backup).expect("the Defy backup validates");

    assert_eq!(dispatched, direct);
}

#[test]
fn a_descriptor_less_backup_dispatches_to_the_defy_validator() {
    // The rule that keeps every today-accepted backup accepted: a
    // backup with no readable product validates through the Defy's
    // shapes — the same plan the Defy validator answers directly (the
    // guard passes an unreadable product; the entries do the rest).
    let mut backup = defy_backup();
    backup.neuron.device = None;

    let dispatched =
        backup_restore_plan(&backup).expect("a descriptor-less backup dispatches to the Defy");
    let direct = DefyKeyboard::backup_restore_plan(&backup).expect("the Defy validator plans it");

    assert_eq!(dispatched, direct);
}

#[test]
fn a_sonsei_backup_dispatches_to_the_sonsei_validator() {
    // The shipped Sonsei capture validates through the Sonsei types —
    // and its plan is the Sonsei validator's own answer, not the
    // Defy's (which refuses the file by product).
    let backup = sonsei_backup();

    let dispatched = backup_restore_plan(&backup).expect("the Sonsei backup dispatches");
    let direct = super::sonsei::backup_restore_plan(&backup).expect("it validates as a Sonsei");

    assert_eq!(dispatched, direct);

    // And the Defy validator refuses the same file: the dispatch, not
    // the validator, chose the Sonsei shapes.
    assert!(matches!(
        DefyKeyboard::backup_restore_plan(&backup),
        Err(crate::devices::defy::BackupValidationError::NotADefyBackup { .. })
    ));
}

#[test]
fn a_raise_backup_dispatches_to_the_raise_validator() {
    // The shipped Raise ANSI capture — a real device backup —
    // validates through the Raise types, and its plan is the Raise
    // validator's own answer. The guard is per product: the Defy and
    // Sonsei validators refuse the file outright.
    let backup = raise_backup();

    let dispatched = backup_restore_plan(&backup).expect("the Raise backup dispatches");
    let direct = super::raise::backup_restore_plan(&backup).expect("it validates as a Raise");

    assert_eq!(dispatched, direct);
    assert_eq!(dispatched.len(), backup.backup.len() + 1); // + the `led.mode 0` tail

    assert!(matches!(
        DefyKeyboard::backup_restore_plan(&backup),
        Err(crate::devices::defy::BackupValidationError::NotADefyBackup { .. })
    ));
    assert!(matches!(
        super::sonsei::backup_restore_plan(&backup).unwrap_err(),
        super::sonsei::BackupValidationError::NotASonseiBackup { .. }
    ));
}

#[test]
fn a_raise2_product_backup_dispatches_to_the_raise2_validator() {
    // The dispatch routes by product name: a backup naming the Raise2
    // validates through the Raise 2's shapes. This file's data is the
    // shipped Raise capture's — its 48-decimal RGB palette fails the
    // Raise 2's 64-decimal RGBW shape first (the palette entry is
    // captured before the colormap, whose 132-LED layers would fail
    // the 176-LED ones next), which proves the dispatch reached the
    // Raise 2 validator (the Raise validator accepted the same data
    // in the test above).
    let mut backup = raise_backup();
    if let Some(device) = backup.neuron.device.as_mut() {
        device["info"]["product"] = serde_json::Value::String("Raise2".to_owned());
    }

    let dispatched = backup_restore_plan(&backup).expect_err("the Raise 2 validator refuses");

    assert!(matches!(
        dispatched,
        RestorePlanError::Raise2(
            super::raise2::BackupValidationError::InvalidEntry { command, .. }
        ) if command == "palette"
    ));

    // The override pair routes identically.
    assert!(matches!(
        backup_restore_plan_allowing_partial(&backup).unwrap_err(),
        RestorePlanError::Raise2(super::raise2::BackupValidationError::InvalidEntry { .. })
    ));
}

#[test]
fn an_unknown_product_is_refused_not_guessed() {
    // A readable product the crate does not know: a typed refusal,
    // never a silent Defy guess.
    let mut backup = defy_backup();
    if let Some(device) = backup.neuron.device.as_mut() {
        device["info"]["product"] = serde_json::Value::String("Keyboard 3000".to_owned());
    }

    assert!(matches!(
        backup_restore_plan(&backup).unwrap_err(),
        RestorePlanError::UnknownProduct { ref product } if product == "Keyboard 3000"
    ));
}

#[test]
fn the_sonsei_partial_override_routes_like_the_default() {
    // `--allow-partial`-style planning on a Sonsei backup: the
    // override reaches the Sonsei validator's own override, refusing
    // nothing the Sonsei validator would not.
    let mut backup = sonsei_backup();
    backup.partial = Some(parsing::backup::PartialCapture {
        missing: vec!["palette".to_owned()],
    });

    assert!(matches!(
        backup_restore_plan(&backup),
        Err(RestorePlanError::Sonsei(
            sonsei::BackupValidationError::PartialCapture { .. }
        ))
    ));

    let plan = backup_restore_plan_allowing_partial(&backup)
        .expect("the override plans the partial Sonsei capture");

    assert_eq!(plan.len(), backup.backup.len());
}

#[test]
fn the_raise_partial_override_routes_like_the_default() {
    // `--allow-partial`-style planning on a Raise backup: the
    // override reaches the Raise validator's own override.
    let mut backup = raise_backup();
    backup.partial = Some(parsing::backup::PartialCapture {
        missing: vec!["palette".to_owned()],
    });

    assert!(matches!(
        backup_restore_plan(&backup),
        Err(RestorePlanError::Raise(
            raise::BackupValidationError::PartialCapture { .. }
        ))
    ));

    let plan = backup_restore_plan_allowing_partial(&backup)
        .expect("the override plans the partial Raise capture");

    // The Raise capture has no empty-command artifact to drop, so the
    // plan is every entry plus the `led.mode 0` tail.
    assert_eq!(plan.len(), backup.backup.len() + 1);
}
