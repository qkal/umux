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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalRenderCell {
    pub cell: TerminalCell,
    pub col: u16,
    pub row: u16,
    pub cursor: bool,
    pub selected: bool,
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

    pub fn selected_text(&self) -> Option<String> {
        let selection = self.selection?;
        let (start_col, start_row, end_col, end_row) = rectangular_selection_bounds(selection);
        let cols = usize::from(self.cols);
        if cols == 0 {
            return Some(String::new());
        }

        let mut lines = Vec::new();
        for row in start_row..=end_row {
            let row_start = usize::from(row) * cols;
            if row_start >= self.cells.len() {
                break;
            }

            let first_col = start_col.min(self.cols.saturating_sub(1));
            let last_col = end_col.min(self.cols.saturating_sub(1));
            let first = row_start + usize::from(first_col);
            let last = row_start + usize::from(last_col);
            if first >= self.cells.len() {
                lines.push(String::new());
                continue;
            }

            let last = last.min(self.cells.len().saturating_sub(1));
            lines.push(
                self.cells[first..=last]
                    .iter()
                    .map(|cell| cell.ch)
                    .collect::<String>()
                    .trim_end_matches(' ')
                    .to_string(),
            );
        }

        Some(lines.join("\n"))
    }

    pub fn render_cells(&self) -> Vec<TerminalRenderCell> {
        let cols = usize::from(self.cols);
        if cols == 0 {
            return Vec::new();
        }

        self.cells
            .iter()
            .take(usize::from(self.rows).saturating_mul(cols))
            .enumerate()
            .map(|(index, cell)| {
                let col = (index % cols) as u16;
                let row = (index / cols) as u16;
                TerminalRenderCell {
                    cell: *cell,
                    col,
                    row,
                    cursor: self.cursor.visible && self.cursor.col == col && self.cursor.row == row,
                    selected: self
                        .selection
                        .is_some_and(|selection| selection_contains(selection, col, row)),
                }
            })
            .collect()
    }
}

fn selection_contains(selection: TerminalSelection, col: u16, row: u16) -> bool {
    let (start_col, start_row, end_col, end_row) = rectangular_selection_bounds(selection);

    row >= start_row && row <= end_row && col >= start_col && col <= end_col
}

fn rectangular_selection_bounds(selection: TerminalSelection) -> (u16, u16, u16, u16) {
    (
        selection.start_col.min(selection.end_col),
        selection.start_row.min(selection.end_row),
        selection.start_col.max(selection.end_col),
        selection.start_row.max(selection.end_row),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_selected_text_extracts_rectangular_visible_cells() {
        let snapshot = TerminalRendererSnapshot {
            cols: 5,
            rows: 3,
            cells: "helloabc  tail ".chars().map(cell).collect::<Vec<_>>(),
            cursor: TerminalCursor {
                col: 0,
                row: 0,
                visible: true,
            },
            selection: Some(TerminalSelection {
                start_col: 1,
                start_row: 0,
                end_col: 3,
                end_row: 1,
            }),
            scrollback_lines: 0,
            version: 7,
        };

        assert_eq!(snapshot.selected_text(), Some("ell\nbc".to_string()));
    }

    #[test]
    fn snapshot_selected_text_uses_rectangular_columns_for_multiline_reverse_column_drag() {
        let snapshot = TerminalRendererSnapshot {
            cols: 10,
            rows: 4,
            cells: "0123456789abcdefghijABCDEFGHIJklmnopqrst"
                .chars()
                .map(cell)
                .collect::<Vec<_>>(),
            cursor: TerminalCursor {
                col: 0,
                row: 0,
                visible: false,
            },
            selection: Some(TerminalSelection {
                start_col: 8,
                start_row: 1,
                end_col: 2,
                end_row: 3,
            }),
            scrollback_lines: 0,
            version: 7,
        };

        assert_eq!(
            snapshot.selected_text(),
            Some("cdefghi\nCDEFGHI\nmnopqrs".to_string())
        );
    }

    #[test]
    fn snapshot_render_cells_marks_reverse_column_multiline_selection_rectangularly() {
        let snapshot = TerminalRendererSnapshot {
            cols: 10,
            rows: 4,
            cells: "0123456789abcdefghijABCDEFGHIJklmnopqrst"
                .chars()
                .map(cell)
                .collect::<Vec<_>>(),
            cursor: TerminalCursor {
                col: 0,
                row: 0,
                visible: false,
            },
            selection: Some(TerminalSelection {
                start_col: 8,
                start_row: 1,
                end_col: 2,
                end_row: 3,
            }),
            scrollback_lines: 0,
            version: 7,
        };

        let cells = snapshot.render_cells();

        assert!(cells[12].selected);
        assert!(cells[18].selected);
        assert!(cells[22].selected);
        assert!(cells[38].selected);
        assert!(!cells[11].selected);
        assert!(!cells[39].selected);
    }

    #[test]
    fn snapshot_marks_cursor_and_selection_cells_for_ui_rendering() {
        let snapshot = TerminalRendererSnapshot {
            cols: 3,
            rows: 2,
            cells: "abcdef".chars().map(cell).collect::<Vec<_>>(),
            cursor: TerminalCursor {
                col: 2,
                row: 1,
                visible: true,
            },
            selection: Some(TerminalSelection {
                start_col: 1,
                start_row: 0,
                end_col: 1,
                end_row: 1,
            }),
            scrollback_lines: 0,
            version: 7,
        };

        let cells = snapshot.render_cells();

        assert!(cells[1].selected);
        assert!(cells[4].selected);
        assert!(cells[5].cursor);
        assert_eq!(cells[5].cell.ch, 'f');
    }

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
