// SPDX-License-Identifier: GPL-3.0-or-later

use camino::Utf8PathBuf;

pub fn default_config_dir() -> Utf8PathBuf {
    let base_dir = dirs::config_dir().unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    });

    Utf8PathBuf::from_path_buf(base_dir)
        .unwrap_or_else(|path| Utf8PathBuf::from(path.to_string_lossy().into_owned()))
        .join("umux")
}
