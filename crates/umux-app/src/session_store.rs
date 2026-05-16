// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    fs,
    io::ErrorKind,
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
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }

        let snapshot = AppSnapshot::from_model(model);
        fs::write(&self.path, snapshot.to_json_string()?)?;
        Ok(())
    }

    pub fn load_model(&self) -> Result<Option<AppModel>, SessionStoreError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let json = match std::str::from_utf8(&bytes) {
            Ok(json) => json,
            Err(_) => {
                self.rename_corrupt_file()?;
                return Ok(None);
            }
        };

        match AppSnapshot::from_json_str(json).and_then(AppSnapshot::into_model) {
            Ok(model) => Ok(Some(model)),
            Err(_) => {
                self.rename_corrupt_file()?;
                Ok(None)
            }
        }
    }

    fn rename_corrupt_file(&self) -> Result<(), std::io::Error> {
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
                return fs::rename(&self.path, candidate);
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use camino::Utf8PathBuf;
    use umux_core::AppModel;

    use super::SessionStore;

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
    fn missing_session_returns_none() {
        let session_path = temp_session_path("missing").join("session.json");
        let store = SessionStore::new(session_path);

        assert_eq!(store.load_model().unwrap(), None);
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
