// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

use crate::model::SplitAxis;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CoreCommand {
    CreateWorkspace { cwd: String },
    SplitSelectedPane { axis: SplitAxis },
    OpenBrowserSurface { url: String },
    MarkSurfaceUnread { message: String },
}
