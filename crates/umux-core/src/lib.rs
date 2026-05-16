// SPDX-License-Identifier: GPL-3.0-or-later

pub mod commands;
pub mod events;
pub mod ids;
pub mod model;

pub use commands::CoreCommand;
pub use events::CoreEvent;
pub use ids::{PaneId, SurfaceId, WindowId, WorkspaceId};
pub use model::{AppModel, ModelError, SplitAxis, SplitTree, SurfaceKind, UnreadTarget};

pub const CRATE_NAME: &str = "umux-core";
