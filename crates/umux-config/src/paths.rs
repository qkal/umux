// SPDX-License-Identifier: GPL-3.0-or-later

use camino::Utf8PathBuf;

pub fn default_config_dir() -> Utf8PathBuf {
    let base_dir = match dirs::config_dir() {
        Some(base_dir) => Utf8PathBuf::from_path_buf(base_dir).unwrap_or_else(|_| current_dir()),
        None => current_dir(),
    };

    base_dir.join("umux")
}

fn current_dir() -> Utf8PathBuf {
    std::env::current_dir()
        .ok()
        .and_then(|path| Utf8PathBuf::from_path_buf(path).ok())
        .unwrap_or_else(|| Utf8PathBuf::from("."))
}
