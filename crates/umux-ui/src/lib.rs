// SPDX-License-Identifier: GPL-3.0-or-later

pub mod shell;
pub mod terminal_view;
pub mod theme;

pub use shell::{run, seed_model};

pub const CRATE_NAME: &str = "umux-ui";
