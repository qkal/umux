// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
#[cfg(windows)]
use std::sync::{Arc, Weak};

use thiserror::Error;
use umux_core::{PaneId, SurfaceId, WorkspaceId};
#[cfg(windows)]
use umux_terminal::{PtySpawnConfig, ShellResolver, StartupEnvironment};
use umux_terminal::{TerminalHealth, TerminalNotification, TerminalRendererSnapshot};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSpawnSpec {
    pub workspace_id: WorkspaceId,
    pub pane_id: PaneId,
    pub surface_id: SurfaceId,
    pub cwd: String,
}

#[derive(Clone)]
pub enum TerminalEntry {
    #[cfg(windows)]
    Running {
        spec: TerminalSpawnSpec,
        session: Arc<umux_terminal::LiveTerminalSession>,
    },
    #[cfg(not(windows))]
    Running { spec: TerminalSpawnSpec },
    Failed {
        spec: TerminalSpawnSpec,
        message: String,
    },
}

#[derive(Clone)]
pub enum TerminalEntryHandle {
    #[cfg(windows)]
    Running {
        session: Weak<umux_terminal::LiveTerminalSession>,
    },
    Inert,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalEntrySnapshot {
    pub health: Option<TerminalHealth>,
    pub renderer_snapshot: Option<TerminalRendererSnapshot>,
}

impl TerminalEntry {
    pub fn surface_id(&self) -> SurfaceId {
        self.spec().surface_id
    }

    pub fn spec(&self) -> &TerminalSpawnSpec {
        match self {
            Self::Running { spec, .. } | Self::Failed { spec, .. } => spec,
        }
    }

    pub fn snapshot(&self) -> Option<TerminalRendererSnapshot> {
        self.snapshot_state().renderer_snapshot
    }

    pub fn health(&self) -> Option<TerminalHealth> {
        self.snapshot_state().health
    }

    pub fn snapshot_state(&self) -> TerminalEntrySnapshot {
        match self {
            #[cfg(windows)]
            Self::Running { session, .. } => {
                let (health, renderer_snapshot) = session.renderer_state();
                TerminalEntrySnapshot {
                    health: Some(health),
                    renderer_snapshot: Some(renderer_snapshot),
                }
            }
            #[cfg(not(windows))]
            Self::Running { .. } => TerminalEntrySnapshot {
                health: None,
                renderer_snapshot: None,
            },
            Self::Failed { .. } => TerminalEntrySnapshot {
                health: None,
                renderer_snapshot: None,
            },
        }
    }

    pub fn weak_handle(&self) -> TerminalEntryHandle {
        match self {
            #[cfg(windows)]
            Self::Running { session, .. } => TerminalEntryHandle::Running {
                session: Arc::downgrade(session),
            },
            #[cfg(not(windows))]
            Self::Running { .. } => TerminalEntryHandle::Inert,
            Self::Failed { .. } => TerminalEntryHandle::Inert,
        }
    }

    pub fn send_input(&self, input: impl AsRef<[u8]>) {
        #[cfg(windows)]
        {
            if let Self::Running { session, .. } = self {
                let _ = session.send_input(input);
            }
        }
        #[cfg(not(windows))]
        let _ = input;
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        #[cfg(windows)]
        {
            if let Self::Running { session, .. } = self {
                let _ = session.resize(cols, rows);
            }
        }
        #[cfg(not(windows))]
        let _ = (cols, rows);
    }

    pub fn drain_notifications(&self) -> Vec<TerminalNotification> {
        match self {
            #[cfg(windows)]
            Self::Running { session, .. } => session.drain_notifications(),
            #[cfg(not(windows))]
            Self::Running { .. } => Vec::new(),
            Self::Failed { .. } => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TerminalRegistryError {
    #[error("terminal surface {0:?} is already registered")]
    AlreadyRegistered(SurfaceId),
}

#[derive(Clone, Default)]
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

        let entry = spawn_terminal_entry(spec);
        self.entries.insert(entry.surface_id(), entry);
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

    pub fn entry(&self, surface_id: SurfaceId) -> Option<&TerminalEntry> {
        self.entries.get(&surface_id)
    }

    pub fn running_surface_ids(&self) -> Vec<SurfaceId> {
        let mut ids = self.entries.keys().copied().collect::<Vec<_>>();
        ids.sort();
        ids
    }
}

#[cfg(windows)]
fn spawn_terminal_entry(spec: TerminalSpawnSpec) -> TerminalEntry {
    let shell = ShellResolver::from_path().resolve();
    let env = StartupEnvironment::new(
        spec.workspace_id.0,
        spec.pane_id.0,
        spec.surface_id.0,
        spec.cwd.clone(),
    )
    .into_pairs();
    let config = PtySpawnConfig {
        shell,
        cwd: spec.cwd.clone(),
        env,
        cols: 80,
        rows: 24,
    };

    match umux_terminal::LiveTerminalSession::spawn(config) {
        Ok(session) => TerminalEntry::Running {
            spec,
            session: Arc::new(session),
        },
        Err(error) => TerminalEntry::Failed {
            spec,
            message: error.to_string(),
        },
    }
}

#[cfg(not(windows))]
fn spawn_terminal_entry(spec: TerminalSpawnSpec) -> TerminalEntry {
    TerminalEntry::Running { spec }
}

impl TerminalEntryHandle {
    pub fn send_input(&self, input: impl AsRef<[u8]>) {
        #[cfg(windows)]
        {
            if let Self::Running { session } = self
                && let Some(session) = session.upgrade()
            {
                let _ = session.send_input(input);
            }
        }
        #[cfg(not(windows))]
        let _ = input;
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        #[cfg(windows)]
        {
            if let Self::Running { session } = self
                && let Some(session) = session.upgrade()
            {
                let _ = session.resize(cols, rows);
            }
        }
        #[cfg(not(windows))]
        let _ = (cols, rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(surface_id: u64) -> TerminalSpawnSpec {
        TerminalSpawnSpec {
            workspace_id: WorkspaceId(1),
            pane_id: PaneId(2),
            surface_id: SurfaceId(surface_id),
            cwd: "C:/work/alpha".to_string(),
        }
    }

    #[test]
    fn registry_removes_closed_sessions() {
        let mut registry = TerminalRegistry::new();
        registry.spawn(spec(20)).unwrap();
        registry.spawn(spec(10)).unwrap();

        let removed = registry.remove(SurfaceId(20)).unwrap();

        assert_eq!(removed.surface_id(), SurfaceId(20));
        assert!(!registry.contains(SurfaceId(20)));
        assert_eq!(registry.running_surface_ids(), vec![SurfaceId(10)]);
    }

    #[test]
    fn registry_exposes_snapshot_entry_for_ui() {
        let mut registry = TerminalRegistry::new();
        let surface_id = SurfaceId(30);
        registry.spawn(spec(surface_id.0)).unwrap();

        let entry = registry.entry(surface_id).unwrap();

        assert_eq!(entry.surface_id(), surface_id);
        #[cfg(not(windows))]
        {
            assert_eq!(entry.snapshot(), None);
            assert_eq!(entry.health(), None);
            assert_eq!(
                entry.snapshot_state(),
                TerminalEntrySnapshot {
                    health: None,
                    renderer_snapshot: None,
                }
            );
        }
    }

    #[test]
    fn failed_entry_snapshot_state_is_empty() {
        let entry = TerminalEntry::Failed {
            spec: spec(40),
            message: "pty refused startup".to_string(),
        };

        assert_eq!(
            entry.snapshot_state(),
            TerminalEntrySnapshot {
                health: None,
                renderer_snapshot: None,
            }
        );
    }
}
