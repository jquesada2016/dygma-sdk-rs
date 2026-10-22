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
fn raise_backups_are_refused_by_name() {
    // A known product with no modeled restore: the Raise. Refused
    // offline, naming the product and the conversion path.
    let mut backup = defy_backup();
    if let Some(device) = backup.neuron.device.as_mut() {
        device["info"]["product"] = serde_json::Value::String("Raise".to_owned());
    }

    assert!(matches!(
        backup_restore_plan(&backup).unwrap_err(),
        RestorePlanError::NoModeledRestore {
            product: Product::Raise
        }
    ));

    // The override pair routes identically.
    assert!(matches!(
        backup_restore_plan_allowing_partial(&backup).unwrap_err(),
        RestorePlanError::NoModeledRestore {
            product: Product::Raise
        }
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
