// SPDX-License-Identifier: GPL-3.0-or-later

pub mod actions;
pub(crate) mod diagnostics;
pub mod runtime;
pub mod startup;
pub mod view_model;
pub mod workspace;

pub use runtime::run;
pub use startup::seed_model;

pub const CRATE_NAME: &str = "umux-ui";
