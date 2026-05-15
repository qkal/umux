// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

use crate::ids::SurfaceId;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CoreEvent {
    WorkspaceSelected,
    PaneSelected,
    SurfaceSelected,
    SurfaceMarkedUnread {
        surface_id: SurfaceId,
        message: String,
    },
}
