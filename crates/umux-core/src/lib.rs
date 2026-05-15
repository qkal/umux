// SPDX-License-Identifier: GPL-3.0-or-later

pub mod commands;
pub mod events;
pub mod ids;
pub mod model;

pub use commands::CoreCommand;
pub use events::CoreEvent;
pub use ids::{IdGen, PaneId, SurfaceId, WindowId, WorkspaceId};
pub use model::{
    AppModel, AppWindow, ModelError, Pane, SplitAxis, SplitTree, Surface, SurfaceKind, Workspace,
};

pub const CRATE_NAME: &str = "umux-core";
