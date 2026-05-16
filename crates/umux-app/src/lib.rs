// SPDX-License-Identifier: GPL-3.0-or-later

pub mod action;
pub mod controller;
pub mod terminal_registry;

pub use action::{AppAction, AppActionOutcome};
pub use controller::{AppController, AppControllerError};
pub use terminal_registry::{
    TerminalEntry, TerminalRegistry, TerminalRegistryError, TerminalSpawnSpec,
};

pub const CRATE_NAME: &str = "umux-app";
