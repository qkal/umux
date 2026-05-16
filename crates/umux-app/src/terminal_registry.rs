// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use thiserror::Error;
use umux_core::{PaneId, SurfaceId, WorkspaceId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSpawnSpec {
    pub workspace_id: WorkspaceId,
    pub pane_id: PaneId,
    pub surface_id: SurfaceId,
    pub cwd: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalEntry {
    Running(TerminalSpawnSpec),
    Failed {
        spec: TerminalSpawnSpec,
        message: String,
    },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TerminalRegistryError {
    #[error("terminal surface {0:?} is already registered")]
    AlreadyRegistered(SurfaceId),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TerminalRegistry {
    entries: HashMap<SurfaceId, TerminalEntry>,
}

impl TerminalRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(&mut self, spec: TerminalSpawnSpec) -> Result<(), TerminalRegistryError> {
        let surface_id = spec.surface_id;
        if self.entries.contains_key(&surface_id) {
            return Err(TerminalRegistryError::AlreadyRegistered(surface_id));
        }
        self.entries
            .insert(surface_id, TerminalEntry::Running(spec));
        Ok(())
    }

    pub fn remove(&mut self, surface_id: SurfaceId) -> Option<TerminalEntry> {
        self.entries.remove(&surface_id)
    }

    pub fn contains(&self, surface_id: SurfaceId) -> bool {
        self.entries.contains_key(&surface_id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
