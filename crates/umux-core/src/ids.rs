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

    pub fn next_window(&mut self) -> WindowId {
        WindowId(self.next())
    }

    pub fn next_workspace(&mut self) -> WorkspaceId {
        WorkspaceId(self.next())
    }

    pub fn next_pane(&mut self) -> PaneId {
        PaneId(self.next())
    }

    pub fn next_surface(&mut self) -> SurfaceId {
        SurfaceId(self.next())
    }

    fn next(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }
}
