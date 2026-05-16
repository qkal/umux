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

    pub fn from_next_id(next_id: u64) -> Self {
        Self { next_id }
    }

    pub fn advance_past(&mut self, id: u64) {
        self.next_id = self.next_id.max(id);
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

#[cfg(test)]
mod tests {
    use super::IdGen;

    #[test]
    fn id_gen_can_advance_past_restored_ids() {
        let mut ids = IdGen::new();

        ids.advance_past(42);

        assert_eq!(ids.next_window().0, 43);
        assert_eq!(ids.next_workspace().0, 44);
    }

    #[test]
    fn id_gen_from_next_id_treats_value_as_last_issued_id() {
        let mut ids = IdGen::from_next_id(42);

        assert_eq!(ids.next_surface().0, 43);
    }

    #[test]
    fn id_gen_does_not_rewind_when_advancing_past_lower_id() {
        let mut ids = IdGen::new();
        assert_eq!(ids.next_surface().0, 1);

        ids.advance_past(1);
        ids.advance_past(0);

        assert_eq!(ids.next_surface().0, 2);
    }
}
