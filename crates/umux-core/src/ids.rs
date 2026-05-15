// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        pub struct $name(pub u64);
    };
}

id_type!(WindowId);
id_type!(WorkspaceId);
id_type!(PaneId);
id_type!(SurfaceId);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IdGen {
    next_id: u64,
}

impl IdGen {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn window_id(&mut self) -> WindowId {
        WindowId(self.next())
    }

    pub fn workspace_id(&mut self) -> WorkspaceId {
        WorkspaceId(self.next())
    }

    pub fn pane_id(&mut self) -> PaneId {
        PaneId(self.next())
    }

    pub fn surface_id(&mut self) -> SurfaceId {
        SurfaceId(self.next())
    }

    fn next(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
