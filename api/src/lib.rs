//! This crate provides functions for working with Dygma keyboards, like
//! the Defy, Raise 2, etc.

#![deny(missing_docs)]

#[macro_use]
extern crate derive_more;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate tracing;

pub mod devices;
pub mod focus_api;
pub mod parsing;
pub mod schema;
mod serial_port;
