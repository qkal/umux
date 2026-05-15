// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(windows)]

use std::time::{Duration, Instant};

use umux_terminal::{
    AlacrittyPtyBackend, PtyBackend, PtySpawnConfig, ShellResolver, StartupEnvironment,
};

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
