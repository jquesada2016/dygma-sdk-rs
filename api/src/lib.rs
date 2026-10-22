//! Tools for working with Dygma keyboards programmatically over the
//! Focus API their firmware serves: a Rust SDK and the CLI built on it.
//!
//! # Products
//!
//! Device discovery and raw Focus commands cover every product
//! Bazecor matches — the Defy, Raise, Raise 2, and Sonsei. The
//! transport identities that drive matching are documented, with their
//! sources, in [`focus_api::identities`], and
//! [`devices::connection::DygmaConnection`] opens a connection to any
//! of them, identifying which product it reached.
//!
//! Support is not the same depth for every product, and the docs say
//! so plainly:
//!
//! - **Defy** — the reference product, the only one validated against
//!   real hardware, and the only one carrying device-level typed APIs
//!   ([`devices::defy::DefyKeyboard`]: keymap, superkeys, macros,
//!   palette and colormap, settings, wireless, backup/restore).
//! - **Raise, Raise 2, Sonsei** — format-level and transport-level
//!   support only, validated differentially against Bazecor's own
//!   sources: their backup files parse and convert
//!   ([`devices::conversions`]), validate for restore, and ship
//!   default configurations ([`devices::defaults`]) — while raw
//!   command sessions through [`devices::connection::DygmaConnection`]
//!   are the supported surface for talking to one. No device-level
//!   typed APIs exist for them.
//!
//! Community validation on real hardware is welcome and wanted; if
//! you own a Raise, Raise 2, or Sonsei, see the repo README for what
//! would help.

#![deny(missing_docs)]

#[macro_use]
extern crate derive_more;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate tracing;

pub mod devices;
pub mod focus_api;
pub mod names;

pub use focus_api::parsing::keymap::keycode_tables;
