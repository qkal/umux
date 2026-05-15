// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartupEnvironment {
    workspace_id: u64,
    pane_id: u64,
    surface_id: u64,
    cwd: String,
}

impl StartupEnvironment {
    pub fn new(workspace_id: u64, pane_id: u64, surface_id: u64, cwd: impl Into<String>) -> Self {
        Self {
            workspace_id,
            pane_id,
            surface_id,
            cwd: cwd.into(),
        }
    }

    pub fn into_pairs(self) -> HashMap<String, String> {
        HashMap::from([
            (
                "UMUX_WORKSPACE_ID".to_string(),
                self.workspace_id.to_string(),
            ),
            ("UMUX_PANE_ID".to_string(), self.pane_id.to_string()),
            ("UMUX_SURFACE_ID".to_string(), self.surface_id.to_string()),
            ("UMUX_CWD".to_string(), self.cwd),
            ("UMUX_TERMINAL".to_string(), "1".to_string()),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_environment_sets_canonical_umux_context() {
        let env = StartupEnvironment::new(2, 3, 4, "C:/work/alpha").into_pairs();

        assert_eq!(env.get("UMUX_WORKSPACE_ID").map(String::as_str), Some("2"));
        assert_eq!(env.get("UMUX_PANE_ID").map(String::as_str), Some("3"));
        assert_eq!(env.get("UMUX_SURFACE_ID").map(String::as_str), Some("4"));
        assert_eq!(
            env.get("UMUX_CWD").map(String::as_str),
            Some("C:/work/alpha")
        );
        assert_eq!(env.get("UMUX_TERMINAL").map(String::as_str), Some("1"));
    }

    #[test]
    fn startup_environment_does_not_advertise_socket_before_ipc_exists() {
        let env = StartupEnvironment::new(2, 3, 4, "C:/work/alpha").into_pairs();

        assert!(!env.contains_key("UMUX_SOCKET"));
        assert!(!env.contains_key("CMUX_SOCKET_PATH"));
    }
}
