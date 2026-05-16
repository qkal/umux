// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;

use tracing_subscriber::EnvFilter;

pub(crate) fn init_diagnostics() {
    let filter = log_filter_from_env(
        env::var("RUST_LOG").ok().as_deref(),
        env::var("UMUX_LOG").ok().as_deref(),
    );

    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .compact()
        .try_init();
}

pub(crate) fn log_filter_from_env(rust_log: Option<&str>, umux_log: Option<&str>) -> String {
    rust_log
        .filter(|value| !value.is_empty())
        .or_else(|| umux_log.filter(|value| !value.is_empty()))
        .unwrap_or("umux=info,warn")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_filter_prefers_rust_log_over_umux_log() {
        assert_eq!(
            log_filter_from_env(Some("umux=debug"), Some("umux=trace")),
            "umux=debug"
        );
    }

    #[test]
    fn log_filter_uses_umux_log_when_rust_log_is_absent() {
        assert_eq!(
            log_filter_from_env(None, Some("umux_ui=debug,umux_app=info")),
            "umux_ui=debug,umux_app=info"
        );
    }

    #[test]
    fn log_filter_defaults_to_mvp_support_filter() {
        assert_eq!(log_filter_from_env(None, None), "umux=info,warn");
    }
}
