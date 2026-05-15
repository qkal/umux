// SPDX-License-Identifier: GPL-3.0-or-later

use floem::prelude::*;
use umux_terminal::{FakePtyBackend, ResolvedShell, TerminalSession, TerminalSessionConfig};

const TERMINAL_BG: Color = Color::rgb8(0x11, 0x13, 0x16);
const TERMINAL_TEXT: Color = Color::rgb8(0xe7, 0xea, 0xf0);
const TERMINAL_MUTED: Color = Color::rgb8(0x9b, 0xa3, 0xaf);

pub fn terminal_status_line(shell: &str, cols: u16, rows: u16) -> String {
    format!("{shell} {cols}x{rows}")
}

pub fn terminal_view() -> impl IntoView {
    let shell = ResolvedShell {
        program: "pwsh".to_string(),
        args: Vec::new(),
        attempted: vec!["pwsh".to_string()],
        used_last_resort: false,
    };
    let mut session = TerminalSession::from_backend(
        TerminalSessionConfig::new(shell, ".", 80, 24),
        FakePtyBackend::new("umux terminal MVP\n"),
    );
    let _ = session.pump_once();
    let health = session.health();
    let text = session.snapshot().visible_text();
    let status = terminal_status_line(&health.shell, health.cols, health.rows);

    v_stack((
        label(move || status.clone()).style(|s| s.color(TERMINAL_MUTED).font_size(12.0)),
        label(move || text.clone()).style(|s| s.color(TERMINAL_TEXT).font_size(13.0)),
    ))
    .style(|s| {
        s.width_full()
            .height_full()
            .padding(12.0)
            .gap(8.0)
            .background(TERMINAL_BG)
            .font_family("Cascadia Mono".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_status_line_mentions_shell_and_size() {
        let line = terminal_status_line("pwsh", 80, 24);

        assert_eq!(line, "pwsh 80x24");
    }
}
