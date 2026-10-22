//! The product-dispatched restore-plan seam: one entry point that
//! validates a backup and computes its restore plan **for whatever
//! product the backup names**, offline.
//!
//! Restore *validation* is per-product (each product's types know its
//! wire shapes — the Defy's in [`defy`][crate::devices::defy], the
//! Sonsei's in [`sonsei`][crate::devices::sonsei]), but a caller
//! holding a backup file should not have to dispatch on the product
//! itself: the descriptor is data, and reading it to pick a validator
//! is this module's whole job. A backup whose product cannot be read
//! dispatches to the Defy validator (the crate's only send target, and
//! the Defy guard's own treatment of an unreadable product); a
//! readable UNKNOWN product refuses. The send path is deliberately
//! **not**
//! generalized — there is no Sonsei transport (issue #24's unit 4),
//! so
//! [`DefyKeyboard::restore_backup`](crate::devices::defy::DefyKeyboard::restore_backup)
//! stays the only device seam and keeps its cross-product guard: a
//! Sonsei plan computed here is validation only, and the CLI says so
//! wherever it prints one.

use super::{conversions::Product, defy::DefyKeyboard, sonsei};
use crate::focus_api::parsing::backup::{self, BackupCommand};
pub use error::*;

mod error {
    use super::*;
    use crate::devices::defy::BackupValidationError as DefyValidationError;
    use crate::devices::sonsei::BackupValidationError as SonseiValidationError;

    /// Error returned by [`backup_restore_plan`]: the backup's product
    /// could not be resolved to a validator, or the product's own
    /// validation refused the backup.
    #[derive(Clone, Debug, Display, Error)]
    pub enum RestorePlanError {
        /// The backup's device descriptor names a product this crate
        /// does not know. (A backup whose product cannot be READ — no
        /// descriptor, or no product name in it — is not an error: it
        /// dispatches to the Defy validator, exactly as the Defy
        /// validator's own guard treats it — only a READABLE foreign
        /// product refuses — and as today's Defy-bound restore does.)
        #[display("the backup's device descriptor names the unknown product `{product}`")]
        UnknownProduct {
            /// The product name the descriptor carries.
            product: String,
        },
        /// The product is known but has no restore validation to
        /// dispatch to (the Raise generations: their backups convert,
        /// but this crate models no restore for them — a Raise backup
        /// converted to the Defy is the supported path).
        #[display(
            "no restore validation is modeled for the {product}; convert the backup to a supported target to restore it"
        )]
        NoModeledRestore {
            /// The known product without a modeled restore.
            product: Product,
        },
        /// The Defy validation refused the backup — the Defy
        /// validator's own error, verbatim.
        #[display("{_0}")]
        Defy(DefyValidationError),
        /// The Sonsei validation refused the backup — the Sonsei
        /// validator's own error, verbatim.
        #[display("{_0}")]
        Sonsei(SonseiValidationError),
    }

    impl From<DefyValidationError> for RestorePlanError {
        fn from(err: DefyValidationError) -> Self {
            Self::Defy(err)
        }
    }

    impl From<SonseiValidationError> for RestorePlanError {
        fn from(err: SonseiValidationError) -> Self {
            Self::Sonsei(err)
        }
    }
}

/// Computes the exact command sequence a restore of `backup` would
/// send, for the product the backup's own device descriptor names —
/// validating it first against that product's wire shapes, **without
/// opening a connection**.
///
/// The dispatch: a Defy backup validates through
/// [`DefyKeyboard::backup_restore_plan`], a Sonsei backup through
/// [`sonsei::backup_restore_plan`]; every other product is a typed
/// refusal ([`RestorePlanError::NoModeledRestore`]). A backup whose
/// product cannot be read (no descriptor, or no product name in it)
/// validates through the Defy's shapes — the crate's only send target,
/// and exactly what the Defy validator's own guard does with an
/// unreadable product — while a readable UNKNOWN product is a typed
/// refusal ([`RestorePlanError::UnknownProduct`]), never a guess.
/// The returned plan
/// is product-shaped data in Bazecor's restore order with the `led.mode 0`
/// tail — what a send path would replay. For a Sonsei backup, no send
/// path exists in this crate yet: the plan is validation only (see the
/// [module docs](self)).
pub fn backup_restore_plan(
    backup: &backup::DefyBackup,
) -> Result<Vec<BackupCommand>, RestorePlanError> {
    backup_restore_plan_with(backup, false)
}

/// [`backup_restore_plan`] with the partial-capture refusal lifted, on
/// the same product dispatch: the per-product validators' explicit
/// overrides, routed identically.
pub fn backup_restore_plan_allowing_partial(
    backup: &backup::DefyBackup,
) -> Result<Vec<BackupCommand>, RestorePlanError> {
    backup_restore_plan_with(backup, true)
}

/// The dispatch behind the pair.
///
/// A backup whose product CAN be read dispatches to that product's
/// validator. A backup whose product CANNOT be read — no descriptor
/// at all, or a descriptor without a product name — dispatches to the
/// Defy validator: the Defy is this crate's only send target, the
/// Defy validator's own guard treats an unreadable product exactly
/// that way (only a READABLE foreign product refuses), and that is
/// what a Defy restore of the file has always done. A readable
/// UNKNOWN product is a typed refusal — guessing the Defy would send
/// an unidentifiable backup's data to a device.
fn backup_restore_plan_with(
    backup: &backup::DefyBackup,
    allow_partial: bool,
) -> Result<Vec<BackupCommand>, RestorePlanError> {
    use crate::devices::conversions::ConversionError;

    // The product dispatch itself is offline data reading — the
    // descriptor — so both validators' refusals and every dispatch
    // refusal happen before any caller would touch a device.
    match Product::from_descriptor(backup.neuron.device.as_ref()) {
        Ok(Product::Defy)
        | Err(ConversionError::MissingDescriptor)
        | Err(ConversionError::NoProductName) => {
            let plan = if allow_partial {
                DefyKeyboard::backup_restore_plan_allowing_partial(backup)
            } else {
                DefyKeyboard::backup_restore_plan(backup)
            };

            Ok(plan?)
        }
        Ok(Product::Sonsei) => {
            let plan = if allow_partial {
                sonsei::backup_restore_plan_allowing_partial(backup)
            } else {
                sonsei::backup_restore_plan(backup)
            };

            Ok(plan?)
        }
        Ok(product @ (Product::Raise | Product::Raise2)) => {
            Err(RestorePlanError::NoModeledRestore { product })
        }
        Err(ConversionError::UnknownProduct { product }) => {
            Err(RestorePlanError::UnknownProduct { product })
        }
        // The match above is exhaustive on purpose: of
        // `Product::from_descriptor`'s errors, only the three handled
        // arms can arise — spelling the remaining `ConversionError`
        // variants out turns any future extension of that enum into a
        // compile error here instead of a silent mis-mapping.
        Err(
            ConversionError::UnsupportedConversion { .. }
            | ConversionError::NoKeyboardType
            | ConversionError::UnknownKeyboardType { .. }
            | ConversionError::MissingUnderglowGeometry
            | ConversionError::UnderglowGeometryTooNarrow { .. }
            | ConversionError::SameProduct { .. }
            | ConversionError::MissingCommand { .. }
            | ConversionError::DuplicateCommand { .. }
            | ConversionError::InvalidEntry { .. },
        ) => unreachable!("a descriptor read cannot produce a conversion-shape error"),
    }
}

#[cfg(test)]
mod tests;
