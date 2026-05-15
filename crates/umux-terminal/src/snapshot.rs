// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

use crate::appearance::TerminalColor;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalCell {
    pub ch: char,
    pub fg: TerminalColor,
    pub bg: TerminalColor,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalCursor {
    pub col: u16,
    pub row: u16,
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalSelection {
    pub start_col: u16,
    pub start_row: u16,
    pub end_col: u16,
    pub end_row: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalRendererSnapshot {
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<TerminalCell>,
    pub cursor: TerminalCursor,
    pub selection: Option<TerminalSelection>,
    pub scrollback_lines: u32,
    pub version: u64,
}

impl TerminalRendererSnapshot {
    pub fn visible_text(&self) -> String {
        let cols = usize::from(self.cols);
        if cols == 0 {
            return String::new();
        }

        let visible_cell_count = usize::from(self.rows)
            .saturating_mul(cols)
            .min(self.cells.len());

        self.cells[..visible_cell_count]
            .chunks(cols)
            .map(|row| {
                row.iter()
                    .map(|cell| cell.ch)
                    .collect::<String>()
                    .trim_end_matches(' ')
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_plain_text_trims_padding_per_line() {
        let snapshot = TerminalRendererSnapshot {
            cols: 5,
            rows: 2,
            cells: vec![
                cell('h'),
                cell('i'),
                cell(' '),
                cell(' '),
                cell(' '),
                cell('o'),
                cell('k'),
                cell(' '),
                cell(' '),
                cell(' '),
            ],
            cursor: TerminalCursor {
                col: 2,
                row: 0,
                visible: true,
            },
            selection: None,
            scrollback_lines: 0,
            version: 7,
        };

        assert_eq!(snapshot.visible_text(), "hi\nok");
    }

    #[test]
    fn snapshot_plain_text_handles_zero_columns_without_panicking() {
        let snapshot = TerminalRendererSnapshot {
            cols: 0,
            rows: 2,
            cells: vec![cell('h'), cell('i')],
            cursor: TerminalCursor {
                col: 0,
                row: 0,
                visible: true,
            },
            selection: None,
            scrollback_lines: 0,
            version: 7,
        };

        assert_eq!(snapshot.visible_text(), "");
    }

    #[test]
    fn snapshot_plain_text_ignores_cells_beyond_declared_visible_rows() {
        let snapshot = TerminalRendererSnapshot {
            cols: 2,
            rows: 1,
            cells: vec![cell('o'), cell('k'), cell('n'), cell('o')],
            cursor: TerminalCursor {
                col: 0,
                row: 0,
                visible: true,
            },
            selection: None,
            scrollback_lines: 0,
            version: 7,
        };

        assert_eq!(snapshot.visible_text(), "ok");
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
