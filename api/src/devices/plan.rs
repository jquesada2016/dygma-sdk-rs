//! The shared restore-plan skeleton: the validation-and-planning core
//! every product's restore validator instantiates.
//!
//! A product validator answers one question — *what would restoring
//! this backup send?* — and the answer's shape is product-independent:
//! refuse a partial capture unless allowed; refuse a backup whose
//! readable descriptor names another product (conversion is the
//! explicit pre-step between products; an UNREADABLE product passes,
//! exactly as every product's guard treats it); validate every
//! captured entry; then return
//! [`restore_command_sequence`]'s order with the `led.mode 0` tail
//! Bazecor's restore appends. Only the middle step is product-shaped:
//! which wire shapes the entries are parsed against. This module holds
//! the product-independent skeleton once — the plan order, the
//! product guard, the partial refusal, and the product-independent
//! superkeys/macros entries (whose wire formats every product shares)
//! — so a product's validator is only its own shaped arms and error
//! wording, and no product can get the order or the tail wrong.
//!
//! Instances: the Defy ([`defy`][crate::devices::defy]), the Sonsei
//! ([`sonsei`][crate::devices::sonsei]), the Raise, and the Raise 2.
//! The per-product validation errors stay product-typed (each
//! product's [`ProductRestoreShapes::EntryError`] names its own wire
//! shapes); everything this module does is stated once, here.

use crate::devices::defy::{MacroMap, ParseMacroMapError, ParseSuperkeyMapError, SuperkeyMap};
use crate::focus_api::parsing::backup::{self, BackupCommand};
use std::str::FromStr;

/// The captured commands whose wire formats are product-independent:
/// a `superkeys.map` entry means the same thing on every product, and
/// so does a `macros.map` one. Validated by the shared skeleton, with
/// the failure folded into the product's own entry error through
/// [`ProductRestoreShapes::shared_map_error`].
pub(super) const SUPERKEY_MAP_COMMAND_NAME: &str = "superkeys.map";
pub(super) const MACRO_MAP_COMMAND_NAME: &str = "macros.map";

/// The command Bazecor's restore sends after every backup entry: the
/// `led.mode` finalizer is deny-listed from captures, so restores
/// append it unconditionally.
const RESTORE_FINALIZE_COMMAND_NAME: &str = "led.mode";
const RESTORE_FINALIZE_DATA: &str = "0";

/// The parse failure of one of the product-independent maps — the
/// shared entry arms' error, folded into each product's own entry
/// error type.
#[derive(Clone, Debug, Display, From, Error)]
pub(super) enum SharedMapError {
    /// The `superkeys.map` data is not a valid superkeys map.
    #[display("{_0}")]
    Superkeys(ParseSuperkeyMapError),
    /// The `macros.map` data is not a valid macros map.
    #[display("{_0}")]
    Macros(ParseMacroMapError),
}

/// One product's restore-validation shapes: what a backup of this
/// product is validated against, and how the skeleton's refusals are
/// worded for it.
///
/// Implemented by a private marker type per product (the Defy's, the
/// Sonsei's, the Raise's, the Raise 2's); the validators' public
/// errors stay where they are — this trait only names the pieces the
/// skeleton wires together.
pub(super) trait ProductRestoreShapes {
    /// The product's own validation error — the whole failure a
    /// refused backup reports.
    type ValidationError: Clone + std::fmt::Display + std::error::Error;

    /// One captured entry's parse failure, in the product's own
    /// wording (one variant per wire shape the product validates).
    type EntryError: Clone + std::fmt::Display + std::error::Error;

    /// The product name the guard matches, as the device descriptors'
    /// `neuron.device.info.product` spells it.
    const EXPECTED_PRODUCT: &'static str;

    /// The refusal for a backup whose readable descriptor names
    /// another product.
    fn wrong_product(product: String) -> Self::ValidationError;

    /// The refusal for a backup carrying the partial-capture marker.
    ///
    /// The refusal's wording has one source for every product —
    /// [`PartialCapture::refusal_display`][refusal] — so each
    /// implementation renders that.
    ///
    /// [refusal]: backup::PartialCapture::refusal_display
    fn refused_partial(missing: Vec<String>) -> Self::ValidationError;

    /// The refusal for one captured entry whose data failed
    /// validation.
    fn invalid_entry(command: String, source: Self::EntryError) -> Self::ValidationError;

    /// Folds a product-independent map's parse failure into the
    /// product's entry error.
    fn shared_map_error(source: SharedMapError) -> Self::EntryError;

    /// Validates one product-shaped entry (the keymap, palette, and
    /// colormap — the wire shapes that differ per product), or `None`
    /// for a command this product has no shaped validator for, which
    /// passes through verbatim exactly as Bazecor's restore sends it.
    /// The product-independent maps are validated by the skeleton
    /// itself and never reach this method.
    fn validate_product_entry(command: &str, data: &str) -> Option<Result<(), Self::EntryError>>;
}

/// Computes the exact command sequence a restore of `backup` would
/// send for the shapes named by `P`, validating it first — without
/// any connection.
///
/// The skeleton, in order: a backup carrying the partial-capture
/// marker is refused unless `allow_partial`; a backup whose readable
/// device descriptor names a product other than
/// [`ProductRestoreShapes::EXPECTED_PRODUCT`] is refused (a backup
/// whose product cannot be READ — no descriptor, or no product name
/// in it — is not an error here: it validates, exactly as every
/// product's own guard treats an unreadable product); every captured
/// entry is validated — the product-independent maps through the
/// shared types, the product-shaped commands through
/// [`ProductRestoreShapes::validate_product_entry`], anything else
/// verbatim; and the plan is
/// [`restore_command_sequence`][sequence]'s order with the `led.mode 0`
/// tail appended, so the plan IS the complete device write sequence.
///
/// [sequence]: backup::restore_command_sequence
pub(super) fn restore_plan<P: ProductRestoreShapes>(
    backup: &backup::DefyBackup,
    allow_partial: bool,
) -> Result<Vec<BackupCommand>, P::ValidationError> {
    if let Some(partial) = &backup.partial
        && !allow_partial
    {
        return Err(P::refused_partial(partial.missing.clone()));
    }

    if let Some(product) = backup.neuron.device.as_ref().and_then(|device| {
        device
            .get("info")
            .and_then(|info| info.get("product"))
            .and_then(serde_json::Value::as_str)
    }) && product != P::EXPECTED_PRODUCT
    {
        return Err(P::wrong_product(product.to_owned()));
    }

    for entry in &backup.backup {
        let validated = match entry.command.as_str() {
            SUPERKEY_MAP_COMMAND_NAME => SuperkeyMap::from_str(&entry.data)
                .map(|_| ())
                .map_err(|source| P::shared_map_error(source.into())),
            MACRO_MAP_COMMAND_NAME => MacroMap::from_str(&entry.data)
                .map(|_| ())
                .map_err(|source| P::shared_map_error(source.into())),
            product_shaped => {
                P::validate_product_entry(product_shaped, &entry.data).unwrap_or(Ok(()))
            }
        };

        validated.map_err(|source| P::invalid_entry(entry.command.clone(), source))?;
    }

    let mut plan = backup::restore_command_sequence(&backup.backup);

    plan.push(BackupCommand::new(
        RESTORE_FINALIZE_COMMAND_NAME.to_owned(),
        RESTORE_FINALIZE_DATA.to_owned(),
    ));

    Ok(plan)
}
