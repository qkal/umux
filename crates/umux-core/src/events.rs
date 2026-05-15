// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

use crate::ids::{PaneId, SurfaceId, WorkspaceId};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CoreEvent {
    WorkspaceSelected(WorkspaceId),
    PaneSelected(PaneId),
    SurfaceSelected(SurfaceId),
    SurfaceMarkedUnread {
        surface_id: SurfaceId,
        message: String,
    },
}
