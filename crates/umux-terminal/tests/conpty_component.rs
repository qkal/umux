// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(windows)]

use std::sync::Mutex;
use std::time::{Duration, Instant};

use umux_terminal::{
    AlacrittyPtyBackend, PtyBackend, PtySpawnConfig, ShellResolver, StartupEnvironment,
};

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn conpty_backend_observes_shell_output() {
    let mut shell = ShellResolver::from_path().resolve();
    shell.args = command_for_output(&shell.program);
    let mut backend = AlacrittyPtyBackend::spawn(PtySpawnConfig {
        shell,
        cwd: "C:/".to_string(),
        env: StartupEnvironment::new(2, 3, 4, "C:/").into_pairs(),
        cols: 80,
        rows: 24,
    })
    .unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut output = String::new();
    while Instant::now() < deadline {
        let chunk = backend.read_output().unwrap();
        if !chunk.is_empty() {
            output.push_str(&String::from_utf8_lossy(&chunk));
            if output.contains("umux-ready") {
                return;
            }
        }

        if backend.child_exited().unwrap().is_some() && output.contains("umux-ready") {
            return;
        }

        std::thread::sleep(Duration::from_millis(20));
    }

    panic!("timed out waiting for shell output, observed: {output:?}");
}

#[test]
fn conpty_backend_does_not_inherit_ipc_socket_env_before_ipc_exists() {
    let _lock = ENV_TEST_LOCK.lock().expect("lock ConPTY env test");
    let _umux_restore = EnvRestore::set("UMUX_SOCKET", "named-pipe");
    let _cmux_restore = EnvRestore::set("CMUX_SOCKET_PATH", "legacy-pipe");
    let mut shell = ShellResolver::from_path().resolve();
    shell.args = command_for_ipc_env_absence(&shell.program);
    let mut backend = AlacrittyPtyBackend::spawn(PtySpawnConfig {
        shell,
        cwd: "C:/".to_string(),
        env: StartupEnvironment::new(2, 3, 4, "C:/").into_pairs(),
        cols: 80,
        rows: 24,
    })
    .unwrap();

    let output = read_until(&mut backend, |output| {
        output.contains("ipc-clean") || output.contains("leaked:")
    });

    assert!(
        output.contains("ipc-clean"),
        "expected child env to omit socket vars, observed: {output:?}"
    );
    assert!(
        !output.contains("leaked:"),
        "socket env leaked to child: {output:?}"
    );
}

fn read_until(backend: &mut AlacrittyPtyBackend, done: impl Fn(&str) -> bool) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut output = String::new();
    while Instant::now() < deadline {
        let chunk = backend.read_output().unwrap();
        if !chunk.is_empty() {
            output.push_str(&String::from_utf8_lossy(&chunk));
            if done(&output) {
                return output;
            }
        }

        if backend.child_exited().unwrap().is_some() && done(&output) {
            return output;
        }

        std::thread::sleep(Duration::from_millis(20));
    }

    output
}

fn command_for_output(program: &str) -> Vec<String> {
    if program.to_ascii_lowercase().contains("cmd") {
        vec!["/C".to_string(), "echo umux-ready".to_string()]
    } else {
        vec![
            "-NoLogo".to_string(),
            "-NoProfile".to_string(),
            "-Command".to_string(),
            "Write-Output umux-ready".to_string(),
        ]
    }
}

fn command_for_ipc_env_absence(program: &str) -> Vec<String> {
    if program.to_ascii_lowercase().contains("cmd") {
        vec![
            "/C".to_string(),
            "if defined UMUX_SOCKET (echo leaked:%UMUX_SOCKET%) else if defined CMUX_SOCKET_PATH (echo leaked:%CMUX_SOCKET_PATH%) else echo ipc-clean".to_string(),
        ]
    } else {
        vec![
            "-NoLogo".to_string(),
            "-NoProfile".to_string(),
            "-Command".to_string(),
            "if ($env:UMUX_SOCKET -or $env:CMUX_SOCKET_PATH) { Write-Output ('leaked:' + $env:UMUX_SOCKET + '|' + $env:CMUX_SOCKET_PATH) } else { Write-Output 'ipc-clean' }".to_string(),
        ]
    }
}

struct EnvRestore {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

impl EnvRestore {
    fn set(key: &'static str, value: &'static str) -> Self {
        let restore = Self {
            key,
            original: std::env::var_os(key),
        };
        // SAFETY: This component test serializes its own environment mutation with ENV_TEST_LOCK
        // and restores the touched key on drop.
        unsafe {
            std::env::set_var(key, value);
        }
        restore
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        // SAFETY: This restores the exact process-environment key saved by EnvRestore.
        unsafe {
            if let Some(value) = &self.original {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}
