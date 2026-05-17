// SPDX-License-Identifier: GPL-3.0-or-later

pub(crate) mod diagnostics;
pub mod shell;
pub(crate) mod terminal_canvas;
pub mod terminal_view;
pub mod theme;
pub mod view_model;

pub use shell::{run, seed_model};

pub const CRATE_NAME: &str = "umux-ui";
