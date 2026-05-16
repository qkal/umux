// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, VecDeque};

use thiserror::Error;

use crate::ResolvedShell;

#[cfg(windows)]
use std::ffi::OsString;
#[cfg(windows)]
use std::io::{Read, Write};
#[cfg(windows)]
use std::path::PathBuf;
#[cfg(windows)]
use std::sync::{Mutex, MutexGuard, OnceLock};
#[cfg(windows)]
use std::time::{Duration, Instant};

#[cfg(windows)]
use alacritty_terminal::event::{OnResize, WindowSize};
#[cfg(windows)]
use alacritty_terminal::tty::{self, ChildEvent, EventedPty, EventedReadWrite, Options, Shell};

#[derive(Debug, Error, Eq, PartialEq)]
pub enum PtyError {
    #[error("pty I/O error: {0}")]
    Io(String),
    #[error("pty backend is unsupported on this platform")]
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PtySpawnConfig {
    pub shell: ResolvedShell,
    pub cwd: String,
    pub env: HashMap<String, String>,
    pub cols: u16,
    pub rows: u16,
}

pub trait PtyBackend {
    fn read_output(&mut self) -> Result<Vec<u8>, PtyError>;
    fn write_input(&mut self, input: &[u8]) -> Result<(), PtyError>;
    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), PtyError>;
    fn child_exited(&mut self) -> Result<Option<i32>, PtyError>;
}

const MAX_PTY_DIMENSION: u16 = i16::MAX as u16;
#[cfg(windows)]
const NON_INHERITED_IPC_ENV_KEYS: [&str; 2] = ["UMUX_SOCKET", "CMUX_SOCKET_PATH"];
#[cfg(windows)]
static SPAWN_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub(crate) fn clamp_pty_size(cols: u16, rows: u16) -> (u16, u16) {
    (
        cols.clamp(1, MAX_PTY_DIMENSION),
        rows.clamp(1, MAX_PTY_DIMENSION),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FakePtyBackend {
    queued_output: VecDeque<Vec<u8>>,
    written_input: Vec<u8>,
    cols: u16,
    rows: u16,
}

impl FakePtyBackend {
    pub fn new(output: impl AsRef<[u8]>) -> Self {
        Self {
            queued_output: VecDeque::from([output.as_ref().to_vec()]),
            written_input: Vec::new(),
            cols: 80,
            rows: 24,
        }
    }

    pub fn written_input(&self) -> &[u8] {
        &self.written_input
    }

    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }
}

impl PtyBackend for FakePtyBackend {
    fn read_output(&mut self) -> Result<Vec<u8>, PtyError> {
        Ok(self.queued_output.pop_front().unwrap_or_default())
    }

    fn write_input(&mut self, input: &[u8]) -> Result<(), PtyError> {
        self.written_input.extend_from_slice(input);
        Ok(())
    }

    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), PtyError> {
        let (cols, rows) = clamp_pty_size(cols, rows);
        self.cols = cols;
        self.rows = rows;
        Ok(())
    }

    fn child_exited(&mut self) -> Result<Option<i32>, PtyError> {
        Ok(None)
    }
}

#[cfg(windows)]
pub struct AlacrittyPtyBackend {
    pty: tty::Pty,
}

#[cfg(windows)]
impl AlacrittyPtyBackend {
    pub fn spawn(config: PtySpawnConfig) -> Result<Self, PtyError> {
        let pty = Self::spawn_raw(config)?;

        Ok(Self { pty })
    }

    pub fn spawn_raw(config: PtySpawnConfig) -> Result<tty::Pty, PtyError> {
        let (cols, rows) = clamp_pty_size(config.cols, config.rows);
        let options = Options {
            shell: Some(Shell::new(config.shell.program, config.shell.args)),
            working_directory: Some(PathBuf::from(config.cwd)),
            drain_on_exit: false,
            env: sanitize_terminal_child_env(config.env),
            escape_args: true,
        };
        let window_size = WindowSize {
            num_lines: rows,
            num_cols: cols,
            cell_width: 0,
            cell_height: 0,
        };

        let _env_guard = SanitizedParentEnv::new(&NON_INHERITED_IPC_ENV_KEYS);
        tty::new(&options, window_size, 0).map_err(io_error)
    }
}

#[cfg(windows)]
impl PtyBackend for AlacrittyPtyBackend {
    fn read_output(&mut self) -> Result<Vec<u8>, PtyError> {
        let mut buf = vec![0; 4096];
        let read = match self.pty.reader().read(&mut buf) {
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(Vec::new()),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => return Ok(Vec::new()),
            Err(error) => return Err(io_error(error)),
        };
        buf.truncate(read);
        Ok(buf)
    }

    fn write_input(&mut self, input: &[u8]) -> Result<(), PtyError> {
        write_all_bounded(self.pty.writer(), input)
    }

    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), PtyError> {
        let (cols, rows) = clamp_pty_size(cols, rows);
        self.pty.on_resize(WindowSize {
            num_lines: rows,
            num_cols: cols,
            cell_width: 0,
            cell_height: 0,
        });
        Ok(())
    }

    fn child_exited(&mut self) -> Result<Option<i32>, PtyError> {
        match self.pty.next_child_event() {
            Some(ChildEvent::Exited(Some(status))) => Ok(status.code()),
            Some(ChildEvent::Exited(None)) => {
                Err(PtyError::Io("child exited without status".to_string()))
            }
            None => Ok(None),
        }
    }
}

#[cfg(windows)]
fn io_error(error: std::io::Error) -> PtyError {
    PtyError::Io(error.to_string())
}

#[cfg(windows)]
fn sanitize_terminal_child_env(mut env: HashMap<String, String>) -> HashMap<String, String> {
    env.retain(|key, _| !is_non_inherited_ipc_env_key(key));
    env
}

#[cfg(windows)]
fn is_non_inherited_ipc_env_key(key: &str) -> bool {
    NON_INHERITED_IPC_ENV_KEYS
        .iter()
        .any(|blocked| key.eq_ignore_ascii_case(blocked))
}

#[cfg(windows)]
struct SanitizedParentEnv {
    _guard: MutexGuard<'static, ()>,
    originals: Vec<(&'static str, Option<OsString>)>,
}

#[cfg(windows)]
impl SanitizedParentEnv {
    fn new(keys: &'static [&'static str]) -> Self {
        let guard = SPAWN_ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("terminal spawn env lock poisoned");
        let mut originals = Vec::with_capacity(keys.len());
        for key in keys {
            originals.push((*key, std::env::var_os(key)));
            // SAFETY: PTY spawning serializes process-environment mutation with SPAWN_ENV_LOCK and
            // restores every touched key before releasing the lock.
            unsafe {
                std::env::remove_var(key);
            }
        }

        Self {
            _guard: guard,
            originals,
        }
    }
}

#[cfg(windows)]
impl Drop for SanitizedParentEnv {
    fn drop(&mut self) {
        for (key, value) in self.originals.drain(..) {
            // SAFETY: This restores process environment while SPAWN_ENV_LOCK is still held by self.
            unsafe {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }
}

#[cfg(windows)]
fn write_all_bounded(writer: &mut impl Write, input: &[u8]) -> Result<(), PtyError> {
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut written = 0;

    while written < input.len() {
        match writer.write(&input[written..]) {
            Ok(0) => {
                if Instant::now() >= deadline {
                    return Err(PtyError::Io(
                        "timed out writing input to pty after writer made no progress".to_string(),
                    ));
                }
                std::thread::yield_now();
            }
            Ok(count) => written += count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(PtyError::Io(
                        "timed out writing input to pty while writer was blocked".to_string(),
                    ));
                }
                std::thread::yield_now();
            }
            Err(error) => return Err(io_error(error)),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_backend_records_input_and_resize() {
        let mut backend = FakePtyBackend::new("ready\n");

        backend.write_input(b"echo hi\r").unwrap();
        backend.resize(120, 30).unwrap();

        assert_eq!(backend.written_input(), b"echo hi\r");
        assert_eq!(backend.size(), (120, 30));
        assert_eq!(backend.read_output().unwrap(), b"ready\n");
    }

    #[test]
    fn fake_backend_clamps_resize_to_conpty_safe_dimensions() {
        let mut backend = FakePtyBackend::new("");

        backend.resize(0, u16::MAX).unwrap();

        assert_eq!(backend.size(), (1, i16::MAX as u16));
    }

    #[test]
    fn fake_backend_returns_empty_output_after_queue_is_drained() {
        let mut backend = FakePtyBackend::new("ready\n");

        assert_eq!(backend.read_output().unwrap(), b"ready\n");
        assert_eq!(backend.read_output().unwrap(), b"");
    }

    #[test]
    fn fake_backend_child_exit_is_absent() {
        let mut backend = FakePtyBackend::new("");
        let result: Result<Option<i32>, PtyError> = backend.child_exited();

        assert_eq!(result.unwrap(), None);
    }

    #[cfg(windows)]
    #[test]
    fn sanitize_terminal_child_env_removes_reserved_ipc_keys() {
        let mut env = HashMap::from([
            ("UMUX_SOCKET".to_string(), "named-pipe".to_string()),
            ("cmux_socket_path".to_string(), "legacy-pipe".to_string()),
            ("UMUX_WORKSPACE_ID".to_string(), "2".to_string()),
        ]);

        env = sanitize_terminal_child_env(env);

        assert!(!env.contains_key("UMUX_SOCKET"));
        assert!(!env.contains_key("cmux_socket_path"));
        assert_eq!(env.get("UMUX_WORKSPACE_ID").map(String::as_str), Some("2"));
    }

    #[cfg(windows)]
    #[test]
    fn sanitized_parent_env_removes_and_restores_reserved_ipc_keys() {
        let _umux_restore = TestEnvRestore::set("UMUX_SOCKET", "named-pipe");
        let _cmux_restore = TestEnvRestore::set("CMUX_SOCKET_PATH", "legacy-pipe");

        {
            let _guard = SanitizedParentEnv::new(&NON_INHERITED_IPC_ENV_KEYS);

            assert_eq!(std::env::var_os("UMUX_SOCKET"), None);
            assert_eq!(std::env::var_os("CMUX_SOCKET_PATH"), None);
        }

        assert_eq!(std::env::var("UMUX_SOCKET").as_deref(), Ok("named-pipe"));
        assert_eq!(
            std::env::var("CMUX_SOCKET_PATH").as_deref(),
            Ok("legacy-pipe")
        );
    }

    #[cfg(windows)]
    struct TestEnvRestore {
        key: &'static str,
        original: Option<OsString>,
    }

    #[cfg(windows)]
    impl TestEnvRestore {
        fn set(key: &'static str, value: &'static str) -> Self {
            let restore = Self {
                key,
                original: std::env::var_os(key),
            };
            // SAFETY: This test restores the touched variable on drop and uses keys not touched by
            // other tests except through SanitizedParentEnv, which serializes its own mutation.
            unsafe {
                std::env::set_var(key, value);
            }
            restore
        }
    }

    #[cfg(windows)]
    impl Drop for TestEnvRestore {
        fn drop(&mut self) {
            // SAFETY: This restores the exact process-environment key saved by the test guard.
            unsafe {
                if let Some(value) = &self.original {
                    std::env::set_var(self.key, value);
                } else {
                    std::env::remove_var(self.key);
                }
            }
        }
    }
}
