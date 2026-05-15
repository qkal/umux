// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;
use std::env;
use std::path::Path;

const SHELL_ORDER: [&str; 3] = ["pwsh", "powershell.exe", "cmd.exe"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedShell {
    pub program: String,
    pub args: Vec<String>,
    pub attempted: Vec<String>,
    pub used_last_resort: bool,
}

#[derive(Clone, Debug)]
pub struct ShellResolver {
    available: HashSet<String>,
}

impl ShellResolver {
    pub fn from_path() -> Self {
        let mut available = HashSet::new();
        if let Some(path) = env::var_os("PATH") {
            for dir in env::split_paths(&path) {
                for shell in SHELL_ORDER {
                    if path_has_program(&dir, shell) {
                        available.insert(shell.to_string());
                    }
                }
            }
        }
        Self { available }
    }

    pub fn new<I, S>(available: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            available: available
                .into_iter()
                .filter_map(|shell| normalize_available_shell(&shell.into()))
                .collect(),
        }
    }

    pub fn resolve(&self) -> ResolvedShell {
        let attempted = SHELL_ORDER
            .iter()
            .map(|shell| (*shell).to_string())
            .collect::<Vec<_>>();
        for shell in SHELL_ORDER {
            if self.available.contains(shell) {
                return ResolvedShell {
                    program: shell.to_string(),
                    args: Vec::new(),
                    attempted,
                    used_last_resort: false,
                };
            }
        }

        ResolvedShell {
            program: "cmd.exe".to_string(),
            args: Vec::new(),
            attempted,
            used_last_resort: true,
        }
    }
}

fn path_has_program(dir: &Path, program: &str) -> bool {
    if dir.join(program).is_file() {
        return true;
    }

    Path::new(program).extension().is_none() && dir.join(format!("{program}.exe")).is_file()
}

fn normalize_available_shell(shell: &str) -> Option<String> {
    match shell.to_ascii_lowercase().as_str() {
        "pwsh" | "pwsh.exe" => Some("pwsh".to_string()),
        "powershell" | "powershell.exe" => Some("powershell.exe".to_string()),
        "cmd" | "cmd.exe" => Some("cmd.exe".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static PATH_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn resolver_prefers_pwsh_then_windows_powershell_then_cmd() {
        let resolver = ShellResolver::new(["cmd.exe", "powershell.exe", "pwsh"]);

        let shell = resolver.resolve();

        assert_eq!(shell.program, "pwsh");
        assert_eq!(shell.attempted, vec!["pwsh", "powershell.exe", "cmd.exe"]);
    }

    #[test]
    fn resolver_falls_back_to_cmd() {
        let resolver = ShellResolver::new(["cmd.exe"]);

        let shell = resolver.resolve();

        assert_eq!(shell.program, "cmd.exe");
        assert_eq!(shell.args, Vec::<String>::new());
    }

    #[test]
    fn resolver_reports_attempts_when_nothing_is_found() {
        let resolver = ShellResolver::new(std::iter::empty::<&str>());

        let shell = resolver.resolve();

        assert_eq!(shell.program, "cmd.exe");
        assert_eq!(shell.attempted, vec!["pwsh", "powershell.exe", "cmd.exe"]);
        assert!(shell.used_last_resort);
    }

    #[test]
    fn resolver_normalizes_common_windows_spellings() {
        let resolver = ShellResolver::new(["CMD.EXE", "PowerShell.EXE", "PWSH.EXE"]);

        let shell = resolver.resolve();

        assert_eq!(shell.program, "pwsh");
        assert!(!shell.used_last_resort);
    }

    #[test]
    fn path_lookup_falls_back_to_exe_for_extensionless_program_names() {
        let dir = temp_shell_dir();
        let shell_path = dir.join("pwsh.exe");
        fs::write(&shell_path, "").expect("create shell fixture");

        assert!(path_has_program(&dir, "pwsh"));

        fs::remove_file(shell_path).expect("remove shell fixture file");
        fs::remove_dir(dir).expect("remove shell fixture directory");
    }

    #[test]
    fn resolver_from_path_discovers_shells_with_exe_fallback() {
        let _lock = PATH_LOCK.lock().expect("lock PATH mutation");
        let dir = temp_shell_dir();
        let shell_path = dir.join("pwsh.exe");
        fs::write(&shell_path, "").expect("create shell fixture");
        let _guard = PathRestoreGuard::set_path(dir.clone());

        let shell = ShellResolver::from_path().resolve();

        assert_eq!(shell.program, "pwsh");
        assert!(!shell.used_last_resort);

        fs::remove_file(shell_path).expect("remove shell fixture file");
        fs::remove_dir(dir).expect("remove shell fixture directory");
    }

    struct PathRestoreGuard {
        original_path: Option<std::ffi::OsString>,
    }

    impl PathRestoreGuard {
        fn set_path(path: std::path::PathBuf) -> Self {
            let guard = Self {
                original_path: env::var_os("PATH"),
            };
            // SAFETY: This test serializes all PATH mutation with PATH_LOCK and restores PATH on drop.
            unsafe {
                env::set_var("PATH", path);
            }
            guard
        }
    }

    impl Drop for PathRestoreGuard {
        fn drop(&mut self) {
            // SAFETY: This test serializes all PATH mutation with PATH_LOCK and restores PATH on drop.
            unsafe {
                if let Some(path) = &self.original_path {
                    env::set_var("PATH", path);
                } else {
                    env::remove_var("PATH");
                }
            }
        }
    }

    fn temp_shell_dir() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after Unix epoch")
            .as_nanos();
        let dir = env::temp_dir().join(format!("umux-terminal-shell-test-{unique}"));
        fs::create_dir(&dir).expect("create shell fixture directory");
        dir
    }
}
