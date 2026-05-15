// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, VecDeque};

use thiserror::Error;

use crate::ResolvedShell;

#[cfg(windows)]
use std::io::{Read, Write};
#[cfg(windows)]
use std::path::PathBuf;
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
            env: config.env,
            escape_args: true,
        };
        let window_size = WindowSize {
            num_lines: rows,
            num_cols: cols,
            cell_width: 0,
            cell_height: 0,
        };

        tty::new(&options, window_size, 0).map_err(io_error)
    }
}

#[cfg(windows)]
impl PtyBackend for AlacrittyPtyBackend {
    fn read_output(&mut self) -> Result<Vec<u8>, PtyError> {
        let mut buf = vec![0; 4096];
        let read = self.pty.reader().read(&mut buf).map_err(io_error)?;
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
}
