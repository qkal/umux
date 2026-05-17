// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{IntoElement, div, prelude::*, px};
use umux_terminal::TerminalRendererSnapshot;
use umux_ui_kit::{BACKGROUND, BORDER, MUTED_TEXT, TEXT};

pub fn terminal_element(
    status: String,
    snapshot: Option<TerminalRendererSnapshot>,
) -> impl IntoElement {
    let rows = terminal_display_rows(status.clone(), snapshot.as_ref());

    div()
        .flex()
        .flex_col()
        .flex_1()
        .size_full()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .p(px(12.0))
        .gap(px(8.0))
        .bg(BACKGROUND)
        .border_t_1()
        .border_color(BORDER)
        .font_family("Lilex")
        .child(
            div()
                .text_size(px(11.0))
                .text_color(MUTED_TEXT)
                .child(status),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .flex()
                .flex_col()
                .text_size(px(13.0))
                .line_height(px(18.0))
                .text_color(TEXT)
                .children(rows.into_iter().map(terminal_line)),
        )
}

fn terminal_line(line: String) -> impl IntoElement {
    div().whitespace_nowrap().child(line)
}

pub(crate) fn terminal_display_text(
    status: String,
    snapshot: Option<&TerminalRendererSnapshot>,
) -> String {
    snapshot
        .map(TerminalRendererSnapshot::visible_text)
        .filter(|text| !text.trim().is_empty())
        .unwrap_or(status)
}

pub(crate) fn terminal_display_rows(
    status: String,
    snapshot: Option<&TerminalRendererSnapshot>,
) -> Vec<String> {
    terminal_display_text(status, snapshot)
        .split('\n')
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use umux_terminal::{TerminalCell, TerminalColor, TerminalCursor, TerminalRendererSnapshot};

    #[test]
    fn display_text_prefers_non_empty_visible_snapshot_text() {
        let snapshot = snapshot_from_text("ready");

        assert_eq!(
            terminal_display_text("pwsh 80x24 running".to_string(), Some(&snapshot)),
            "ready"
        );
    }

    #[test]
    fn display_text_falls_back_to_status_for_empty_snapshot_text() {
        let snapshot = snapshot_from_text("   ");

        assert_eq!(
            terminal_display_text("pwsh 80x24 starting".to_string(), Some(&snapshot)),
            "pwsh 80x24 starting"
        );
    }

    #[test]
    fn display_text_falls_back_to_status_without_snapshot() {
        assert_eq!(
            terminal_display_text("terminal missing".to_string(), None),
            "terminal missing"
        );
    }

    #[test]
    fn display_rows_preserve_multiline_snapshot_text() {
        let snapshot = snapshot_from_text("first\nsecond");

        assert_eq!(
            terminal_display_rows("pwsh 80x24 running".to_string(), Some(&snapshot)),
            vec!["first".to_string(), "second".to_string()]
        );
    }

    #[test]
    fn display_rows_preserve_multiline_status_fallback() {
        assert_eq!(
            terminal_display_rows("line one\nline two".to_string(), None),
            vec!["line one".to_string(), "line two".to_string()]
        );
    }

    #[test]
    fn display_rows_preserve_trailing_blank_snapshot_rows() {
        let snapshot = snapshot_from_text("ready\n");

        assert_eq!(
            terminal_display_rows("pwsh 80x24 running".to_string(), Some(&snapshot)),
            vec!["ready".to_string(), "".to_string()]
        );
    }

    fn snapshot_from_text(text: &str) -> TerminalRendererSnapshot {
        let rows = text.split('\n').collect::<Vec<_>>();
        let cols = rows.iter().map(|line| line.len()).max().unwrap_or(0);
        let cells = rows
            .iter()
            .flat_map(|line| {
                line.chars()
                    .chain(std::iter::repeat_n(' ', cols.saturating_sub(line.len())))
            })
            .map(cell)
            .collect();

        TerminalRendererSnapshot {
            cols: cols as u16,
            rows: rows.len() as u16,
            cells,
            cursor: TerminalCursor {
                col: 0,
                row: 0,
                visible: false,
            },
            selection: None,
            scrollback_lines: 0,
            version: 1,
        }
    }

    fn cell(ch: char) -> TerminalCell {
        TerminalCell {
            ch,
            fg: TerminalColor::rgb(255, 255, 255),
            bg: TerminalColor::rgb(0, 0, 0),
            bold: false,
            italic: false,
            underline: false,
            inverse: false,
        }
    }
}
