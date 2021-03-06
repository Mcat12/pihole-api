// Pi-hole: A black hole for Internet advertisements
// (c) 2019 Pi-hole, LLC (https://pi-hole.net)
// Network-wide ad blocking via your own hardware.
//
// API
// Root Library File
//
// This file is copyright under the latest version of the EUPL.
// Please see LICENSE file for your rights under this license.

#![allow(clippy::cast_lossless)]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate rust_embed;

#[cfg(test)]
#[macro_use]
extern crate lazy_static;

pub use crate::cli::handle_cli;

#[macro_use]
pub mod services;

mod cli;
mod databases;
mod env;
mod ftl;
mod routes;
mod settings;
mod setup;
mod util;

#[cfg(test)]
mod testing;
