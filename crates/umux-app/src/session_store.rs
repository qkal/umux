// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    fs::{self, File},
    io::{self, ErrorKind, Write},
    time::{SystemTime, UNIX_EPOCH},
};

use camino::{Utf8Path, Utf8PathBuf};
use thiserror::Error;
use umux_core::AppModel;
use umux_session::AppSnapshot;

#[derive(Debug, Error)]
pub enum SessionStoreError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Session(#[from] umux_session::SessionError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionLoadOutcome {
    Missing,
    Loaded(AppModel),
    RecoveredCorrupt { corrupt_path: Utf8PathBuf },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionStore {
    path: Utf8PathBuf,
}

impl SessionStore {
    pub fn new(path: Utf8PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> Utf8PathBuf {
        umux_config::default_config_dir().join("session.json")
    }

    pub fn save_model(&self, model: &AppModel) -> Result<(), SessionStoreError> {
        let snapshot = AppSnapshot::from_model(model);
        let json = snapshot.to_json_string()?;

        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }

        let temp_path = temp_save_path(&self.path, std::process::id(), current_nanos());
        let mut temp_file = File::create(&temp_path)?;
        temp_file.write_all(json.as_bytes())?;
        temp_file.sync_all()?;
        drop(temp_file);

        if let Err(error) = persist_temp_file(&temp_path, &self.path) {
            let _ = fs::remove_file(&temp_path);
            return Err(error.into());
        }

        Ok(())
    }

    pub fn load_model(&self) -> Result<Option<AppModel>, SessionStoreError> {
        match self.load_model_with_status()? {
            SessionLoadOutcome::Loaded(model) => Ok(Some(model)),
            SessionLoadOutcome::Missing | SessionLoadOutcome::RecoveredCorrupt { .. } => Ok(None),
        }
    }

    pub fn load_model_with_status(&self) -> Result<SessionLoadOutcome, SessionStoreError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(SessionLoadOutcome::Missing);
            }
            Err(error) => return Err(error.into()),
        };
        let json = match std::str::from_utf8(&bytes) {
            Ok(json) => json,
            Err(_) => {
                let corrupt_path = self.rename_corrupt_file()?;
                return Ok(SessionLoadOutcome::RecoveredCorrupt { corrupt_path });
            }
        };

        match AppSnapshot::from_json_str(json).and_then(AppSnapshot::into_model) {
            Ok(model) => Ok(SessionLoadOutcome::Loaded(model)),
            Err(_) => {
                let corrupt_path = self.rename_corrupt_file()?;
                Ok(SessionLoadOutcome::RecoveredCorrupt { corrupt_path })
            }
        }
    }

    fn rename_corrupt_file(&self) -> Result<Utf8PathBuf, std::io::Error> {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let file_name = self.path.file_name().unwrap_or("session.json");
        let corrupt_file_name = format!("{file_name}.corrupt.{secs}");

        for attempt in 0.. {
            let candidate_name = if attempt == 0 {
                corrupt_file_name.clone()
            } else {
                format!("{corrupt_file_name}.{attempt}")
            };
            let candidate = sibling_path(&self.path, &candidate_name);
            if !candidate.exists() {
                fs::rename(&self.path, &candidate)?;
                return Ok(candidate);
            }
        }

        unreachable!("unbounded corrupt-session rename loop should return");
    }
}

fn sibling_path(path: &Utf8Path, file_name: &str) -> Utf8PathBuf {
    match path.parent() {
        Some(parent) if !parent.as_str().is_empty() => parent.join(file_name),
        _ => Utf8PathBuf::from(file_name),
    }
}

fn temp_save_path(path: &Utf8Path, process_id: u32, nanos: u128) -> Utf8PathBuf {
    let file_name = path.file_name().unwrap_or("session.json");
    sibling_path(path, &format!("{file_name}.tmp.{process_id}.{nanos}"))
}

fn current_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(not(windows))]
fn persist_temp_file(temp_path: &Utf8Path, target_path: &Utf8Path) -> Result<(), io::Error> {
    fs::rename(temp_path, target_path)
}

#[cfg(windows)]
fn persist_temp_file(temp_path: &Utf8Path, target_path: &Utf8Path) -> Result<(), io::Error> {
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    let temp_path = wide_path(temp_path);
    let target_path = wide_path(target_path);
    let result = unsafe {
        MoveFileExW(
            temp_path.as_ptr(),
            target_path.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };

    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn wide_path(path: &Utf8Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_std_path()
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use camino::Utf8PathBuf;
    use umux_core::AppModel;

    use super::{SessionLoadOutcome, SessionStore, temp_save_path};

    #[test]
    fn save_and_load_round_trip_model() {
        let session_path = temp_session_path("round-trip").join("session.json");
        let store = SessionStore::new(session_path);
        let mut model = AppModel::new("C:/work/alpha");
        model
            .create_workspace("C:/work/beta", Some("Beta".to_string()))
            .unwrap();

        store.save_model(&model).unwrap();
        let loaded = store.load_model().unwrap().unwrap();

        assert_eq!(loaded.windows[0].workspaces.len(), 2);
        assert_eq!(loaded.selected_workspace().unwrap().title, "Beta");
    }

    #[test]
    fn saving_twice_replaces_session_and_leaves_no_temp_siblings() {
        let dir = temp_session_path("replace");
        let session_path = dir.join("session.json");
        let store = SessionStore::new(session_path);
        let first = AppModel::new("C:/work/alpha");
        let mut second = AppModel::new("C:/work/alpha");
        second
            .create_workspace("C:/work/beta", Some("Beta".to_string()))
            .unwrap();

        store.save_model(&first).unwrap();
        store.save_model(&second).unwrap();

        let loaded = store.load_model().unwrap().unwrap();
        assert_eq!(loaded.selected_workspace().unwrap().title, "Beta");
        assert!(!fs::read_dir(&dir).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp.")
        }));
    }

    #[test]
    fn temp_save_path_uses_same_directory_target_name_process_and_time() {
        let path = Utf8PathBuf::from("C:/work/session.json");

        let temp_path = temp_save_path(&path, 123, 456);

        assert_eq!(
            temp_path,
            Utf8PathBuf::from("C:/work/session.json.tmp.123.456")
        );
    }

    #[test]
    fn missing_session_returns_none() {
        let session_path = temp_session_path("missing").join("session.json");
        let store = SessionStore::new(session_path);

        assert_eq!(store.load_model().unwrap(), None);
    }

    #[test]
    fn load_model_with_status_reports_missing_session() {
        let session_path = temp_session_path("missing-status").join("session.json");
        let store = SessionStore::new(session_path);

        assert_eq!(
            store.load_model_with_status().unwrap(),
            SessionLoadOutcome::Missing
        );
    }

    #[test]
    fn corrupt_session_is_renamed_aside() {
        let dir = temp_session_path("corrupt");
        fs::create_dir_all(&dir).unwrap();
        let session_path = dir.join("session.json");
        fs::write(&session_path, "{not json").unwrap();
        let store = SessionStore::new(session_path.clone());

        let loaded = store.load_model().unwrap();

        assert_eq!(loaded, None);
        assert!(!session_path.exists());
        assert!(fs::read_dir(&dir).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("session.json.corrupt.")
        }));
    }

    #[test]
    fn load_model_with_status_reports_recovered_corrupt_session() {
        let dir = temp_session_path("corrupt-status");
        fs::create_dir_all(&dir).unwrap();
        let session_path = dir.join("session.json");
        fs::write(&session_path, "{not json").unwrap();
        let store = SessionStore::new(session_path.clone());

        let outcome = store.load_model_with_status().unwrap();

        let SessionLoadOutcome::RecoveredCorrupt { corrupt_path } = outcome else {
            panic!("expected recovered corrupt outcome");
        };
        assert!(!session_path.exists());
        assert!(corrupt_path.exists());
        assert!(
            corrupt_path
                .file_name()
                .unwrap()
                .starts_with("session.json.corrupt.")
        );
    }

    #[test]
    fn invalid_utf8_session_is_renamed_aside() {
        let dir = temp_session_path("invalid-utf8");
        fs::create_dir_all(&dir).unwrap();
        let session_path = dir.join("session.json");
        fs::write(&session_path, [0xff, 0xfe, 0xfd]).unwrap();
        let store = SessionStore::new(session_path.clone());

        let loaded = store.load_model().unwrap();

        assert_eq!(loaded, None);
        assert!(!session_path.exists());
        assert!(fs::read_dir(&dir).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("session.json.corrupt.")
        }));
    }

    #[test]
    fn unsupported_schema_version_is_renamed_aside() {
        let dir = temp_session_path("unsupported");
        fs::create_dir_all(&dir).unwrap();
        let session_path = dir.join("session.json");
        fs::write(
            &session_path,
            r#"{"schema_version":999,"selected_window":1,"windows":[]}"#,
        )
        .unwrap();
        let store = SessionStore::new(session_path.clone());

        let loaded = store.load_model().unwrap();

        assert_eq!(loaded, None);
        assert!(!session_path.exists());
        assert!(fs::read_dir(&dir).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("session.json.corrupt.")
        }));
    }

    fn temp_session_path(name: &str) -> Utf8PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir()
            .join("umux-app-session-store-tests")
            .join(format!("{name}-{nanos}-{}", std::process::id()));
        fs::remove_dir_all(&path).ok();
        Utf8PathBuf::from_path_buf(path).unwrap()
    }
}
