// SPDX-License-Identifier: GPL-3.0-or-later

pub const CRATE_NAME: &str = "umux-config";

pub mod paths;
pub mod shortcuts;

pub use paths::default_config_dir;
pub use shortcuts::{ShortcutBinding, default_shortcuts};
