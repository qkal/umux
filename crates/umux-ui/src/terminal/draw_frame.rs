// SPDX-License-Identifier: GPL-3.0-or-later

use umux_terminal::{
    TerminalColor, TerminalCursor, TerminalMetrics, TerminalRendererSnapshot, TerminalSelection,
    snapshot::TerminalRenderCell,
};

pub const TERMINAL_DEFAULT_BG: TerminalColor = TerminalColor::rgb(0x11, 0x13, 0x16);
pub const TERMINAL_SELECTION_BG: TerminalColor = TerminalColor::rgb(0x2f, 0x80, 0xff);
pub const CURSOR_FG: TerminalColor = TERMINAL_DEFAULT_BG;
pub const CURSOR_BG: TerminalColor = TerminalColor::rgb(0xe7, 0xea, 0xf0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalDrawFrameKey {
    pub version: u64,
    pub cols: u16,
    pub rows: u16,
    pub cursor: TerminalCursor,
    pub selection: Option<TerminalSelection>,
    pub cell_width_bits: u32,
    pub cell_height_bits: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalCellRun {
    pub row: u16,
    pub col: u16,
    pub len: u16,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: TerminalColor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalTextRun {
    pub row: u16,
    pub col: u16,
    pub text: String,
    pub fg: TerminalColor,
    pub bg: TerminalColor,
    pub x: f32,
    pub y: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalDrawFrame {
    pub key: TerminalDrawFrameKey,
    pub cols: u16,
    pub rows: u16,
    pub cell_width: f32,
    pub cell_height: f32,
    pub background: TerminalColor,
    pub background_runs: Vec<TerminalCellRun>,
    pub selection_runs: Vec<TerminalCellRun>,
    pub cursor_run: Option<TerminalCellRun>,
    pub text_runs: Vec<TerminalTextRun>,
}

pub fn prepare_terminal_draw_frame(
    snapshot: TerminalRendererSnapshot,
    metrics: TerminalMetrics,
) -> TerminalDrawFrame {
    let cell_width = metrics.cell_width_px();
    let cell_height = metrics.cell_height_px();
    let key = TerminalDrawFrameKey {
        version: snapshot.version,
        cols: snapshot.cols,
        rows: snapshot.rows,
        cursor: snapshot.cursor,
        selection: snapshot.selection,
        cell_width_bits: cell_width.to_bits(),
        cell_height_bits: cell_height.to_bits(),
    };
    let mut frame = TerminalDrawFrame {
        key,
        cols: snapshot.cols,
        rows: snapshot.rows,
        cell_width,
        cell_height,
        background: TERMINAL_DEFAULT_BG,
        background_runs: Vec::new(),
        selection_runs: Vec::new(),
        cursor_run: None,
        text_runs: Vec::new(),
    };

    if snapshot.cols == 0 {
        return frame;
    }

    for row_cells in snapshot.render_cells().chunks(usize::from(snapshot.cols)) {
        collect_background_runs(row_cells, metrics, &mut frame);
        collect_selection_runs(row_cells, metrics, &mut frame);
        collect_text_runs(row_cells, metrics, &mut frame);
    }

    if snapshot.cursor.visible
        && snapshot.cursor.col < snapshot.cols
        && snapshot.cursor.row < snapshot.rows
    {
        frame.cursor_run = Some(cell_run(
            snapshot.cursor.row,
            snapshot.cursor.col,
            1,
            metrics,
            CURSOR_BG,
        ));
    }

    frame
}

fn collect_background_runs(
    cells: &[TerminalRenderCell],
    metrics: TerminalMetrics,
    frame: &mut TerminalDrawFrame,
) {
    let mut run_start = None::<u16>;
    let mut run_color = TERMINAL_DEFAULT_BG;
    let mut previous_col = 0_u16;
    let row = cells.first().map(|cell| cell.row).unwrap_or_default();

    for cell in cells {
        let (_, bg) = effective_cell_colors(cell);
        if bg == TERMINAL_DEFAULT_BG || cell.selected || cell.cursor {
            flush_cell_run(
                &mut frame.background_runs,
                row,
                &mut run_start,
                previous_col,
                metrics,
                run_color,
            );
            continue;
        }

        match run_start {
            Some(_) if bg == run_color && cell.col == previous_col => {}
            Some(_) => {
                flush_cell_run(
                    &mut frame.background_runs,
                    row,
                    &mut run_start,
                    previous_col,
                    metrics,
                    run_color,
                );
                run_start = Some(cell.col);
                run_color = bg;
            }
            None => {
                run_start = Some(cell.col);
                run_color = bg;
            }
        }
        previous_col = cell.col.saturating_add(1);
    }

    flush_cell_run(
        &mut frame.background_runs,
        row,
        &mut run_start,
        previous_col,
        metrics,
        run_color,
    );
}

fn collect_selection_runs(
    cells: &[TerminalRenderCell],
    metrics: TerminalMetrics,
    frame: &mut TerminalDrawFrame,
) {
    let mut run_start = None::<u16>;
    let mut previous_col = 0_u16;
    let row = cells.first().map(|cell| cell.row).unwrap_or_default();

    for cell in cells {
        if !cell.selected || cell.cursor {
            flush_cell_run(
                &mut frame.selection_runs,
                row,
                &mut run_start,
                previous_col,
                metrics,
                TERMINAL_SELECTION_BG,
            );
            continue;
        }

        if run_start.is_none() {
            run_start = Some(cell.col);
        }
        previous_col = cell.col.saturating_add(1);
    }

    flush_cell_run(
        &mut frame.selection_runs,
        row,
        &mut run_start,
        previous_col,
        metrics,
        TERMINAL_SELECTION_BG,
    );
}

fn collect_text_runs(
    cells: &[TerminalRenderCell],
    metrics: TerminalMetrics,
    frame: &mut TerminalDrawFrame,
) {
    let mut current = None::<TerminalTextRun>;

    for cell in cells {
        let (fg, bg) = effective_cell_colors(cell);

        if cell.cell.ch == ' ' {
            flush_text_run(&mut current, frame);
            continue;
        }

        let style_matches = current.as_ref().is_some_and(|run| {
            run.row == cell.row
                && run.fg == fg
                && run.bg == bg
                && run.bold == cell.cell.bold
                && run.italic == cell.cell.italic
                && run.underline == cell.cell.underline
                && run.inverse == cell.cell.inverse
                && run.col.saturating_add(run.text.chars().count() as u16) == cell.col
        });

        if !style_matches {
            flush_text_run(&mut current, frame);
            current = Some(TerminalTextRun {
                row: cell.row,
                col: cell.col,
                text: String::new(),
                fg,
                bg,
                x: f32::from(cell.col) * metrics.cell_width_px(),
                y: f32::from(cell.row) * metrics.cell_height_px(),
                bold: cell.cell.bold,
                italic: cell.cell.italic,
                underline: cell.cell.underline,
                inverse: cell.cell.inverse,
            });
        }

        if let Some(run) = &mut current {
            run.text.push(cell.cell.ch);
        }
    }

    flush_text_run(&mut current, frame);
}

fn effective_cell_colors(cell: &TerminalRenderCell) -> (TerminalColor, TerminalColor) {
    let (fg, bg) = if cell.cell.inverse {
        (cell.cell.bg, cell.cell.fg)
    } else {
        (cell.cell.fg, cell.cell.bg)
    };

    if cell.cursor {
        (CURSOR_FG, CURSOR_BG)
    } else if cell.selected {
        (fg, TERMINAL_SELECTION_BG)
    } else {
        (fg, bg)
    }
}

fn flush_text_run(current: &mut Option<TerminalTextRun>, frame: &mut TerminalDrawFrame) {
    if let Some(run) = current.take()
        && !run.text.is_empty()
    {
        frame.text_runs.push(run);
    }
}

fn flush_cell_run(
    runs: &mut Vec<TerminalCellRun>,
    row: u16,
    run_start: &mut Option<u16>,
    run_end: u16,
    metrics: TerminalMetrics,
    color: TerminalColor,
) {
    if let Some(start) = run_start.take() {
        let len = run_end.saturating_sub(start);
        if len > 0 {
            runs.push(cell_run(row, start, len, metrics, color));
        }
    }
}

fn cell_run(
    row: u16,
    col: u16,
    len: u16,
    metrics: TerminalMetrics,
    color: TerminalColor,
) -> TerminalCellRun {
    TerminalCellRun {
        row,
        col,
        len,
        x: f32::from(col) * metrics.cell_width_px(),
        y: f32::from(row) * metrics.cell_height_px(),
        width: f32::from(len) * metrics.cell_width_px(),
        height: metrics.cell_height_px(),
        color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use umux_terminal::{
        TerminalCell, TerminalColor, TerminalCursor, TerminalMetrics, TerminalRendererSnapshot,
        TerminalSelection,
    };

    const BLACK: TerminalColor = TerminalColor::rgb(0x11, 0x13, 0x16);
    const WHITE: TerminalColor = TerminalColor::rgb(0xe7, 0xea, 0xf0);
    const RED: TerminalColor = TerminalColor::rgb(0xcd, 0x31, 0x31);

    #[test]
    fn draw_frame_groups_text_runs_with_fixed_cell_coordinates() {
        let snapshot = snapshot_from_cells(
            5,
            1,
            vec![
                cell(' ', WHITE, BLACK),
                cell('h', WHITE, BLACK),
                cell('i', WHITE, BLACK),
                cell('!', RED, BLACK),
                cell(' ', WHITE, BLACK),
            ],
        );

        let frame = prepare_terminal_draw_frame(snapshot, TerminalMetrics::new(8.0, 16.0));

        assert_eq!(frame.text_runs.len(), 2);
        assert_eq!(frame.text_runs[0].text, "hi");
        assert_eq!(frame.text_runs[0].col, 1);
        assert_eq!(frame.text_runs[0].x, 8.0);
        assert_eq!(frame.text_runs[0].y, 0.0);
        assert_eq!(frame.text_runs[1].text, "!");
        assert_eq!(frame.text_runs[1].col, 3);
        assert_eq!(frame.text_runs[1].x, 24.0);
    }

    #[test]
    fn selection_runs_skip_cursor_and_text_uses_selection_and_cursor_overrides() {
        let mut snapshot = snapshot_from_text("abcd", 4, 1);
        snapshot.selection = Some(TerminalSelection {
            start_col: 1,
            start_row: 0,
            end_col: 3,
            end_row: 0,
        });
        snapshot.cursor = TerminalCursor {
            col: 2,
            row: 0,
            visible: true,
        };

        let frame = prepare_terminal_draw_frame(snapshot, TerminalMetrics::new(8.0, 16.0));

        assert_eq!(frame.selection_runs.len(), 2);
        assert_eq!(frame.selection_runs[0].col, 1);
        assert_eq!(frame.selection_runs[0].len, 1);
        assert_eq!(frame.selection_runs[1].col, 3);
        assert_eq!(frame.selection_runs[1].len, 1);
        assert_eq!(frame.cursor_run.as_ref().map(|run| run.col), Some(2));
        assert_eq!(frame.cursor_run.as_ref().map(|run| run.x), Some(16.0));
        assert_eq!(frame.text_runs[1].text, "b");
        assert_eq!(frame.text_runs[1].bg, TERMINAL_SELECTION_BG);
        assert_eq!(frame.text_runs[2].text, "c");
        assert_eq!(frame.text_runs[2].fg, CURSOR_FG);
        assert_eq!(frame.text_runs[2].bg, CURSOR_BG);
    }

    #[test]
    fn background_runs_group_non_default_cells_and_skip_selection_and_cursor() {
        let bg = TerminalColor::rgb(10, 20, 30);
        let mut snapshot = snapshot_from_cells(
            5,
            1,
            vec![
                cell('a', WHITE, bg),
                cell('b', WHITE, bg),
                cell('c', WHITE, bg),
                cell('d', WHITE, BLACK),
                cell('e', WHITE, bg),
            ],
        );
        snapshot.selection = Some(TerminalSelection {
            start_col: 1,
            start_row: 0,
            end_col: 1,
            end_row: 0,
        });
        snapshot.cursor = TerminalCursor {
            col: 2,
            row: 0,
            visible: true,
        };

        let frame = prepare_terminal_draw_frame(snapshot, TerminalMetrics::new(8.0, 16.0));

        assert_eq!(frame.background_runs.len(), 2);
        assert_eq!(frame.background_runs[0].col, 0);
        assert_eq!(frame.background_runs[0].len, 1);
        assert_eq!(frame.background_runs[0].width, 8.0);
        assert_eq!(frame.background_runs[1].col, 4);
        assert_eq!(frame.background_runs[1].len, 1);
        assert_eq!(frame.background_runs[1].x, 32.0);
    }

    #[test]
    fn cursor_run_only_appears_when_visible_and_inside_bounds() {
        let mut snapshot = snapshot_from_text("ab", 2, 1);
        snapshot.cursor = TerminalCursor {
            col: 1,
            row: 0,
            visible: true,
        };

        let frame = prepare_terminal_draw_frame(snapshot.clone(), TerminalMetrics::new(8.0, 16.0));

        assert_eq!(frame.cursor_run.as_ref().map(|run| run.col), Some(1));
        assert_eq!(
            frame.cursor_run.as_ref().map(|run| run.color),
            Some(CURSOR_BG)
        );

        snapshot.cursor.col = 2;
        let frame = prepare_terminal_draw_frame(snapshot.clone(), TerminalMetrics::new(8.0, 16.0));
        assert_eq!(frame.cursor_run, None);

        snapshot.cursor = TerminalCursor {
            col: 1,
            row: 0,
            visible: false,
        };
        let frame = prepare_terminal_draw_frame(snapshot, TerminalMetrics::new(8.0, 16.0));
        assert_eq!(frame.cursor_run, None);
    }

    #[test]
    fn inverse_text_uses_swapped_effective_colors() {
        let snapshot = snapshot_from_cells(1, 1, vec![inverse_cell('x', WHITE, BLACK)]);

        let frame = prepare_terminal_draw_frame(snapshot, TerminalMetrics::new(8.0, 16.0));

        assert_eq!(frame.text_runs.len(), 1);
        assert_eq!(frame.text_runs[0].text, "x");
        assert_eq!(frame.text_runs[0].fg, BLACK);
        assert_eq!(frame.text_runs[0].bg, WHITE);
        assert_eq!(frame.background_runs.len(), 1);
        assert_eq!(frame.background_runs[0].color, WHITE);
    }

    #[test]
    fn inverse_space_emits_background_run() {
        let snapshot = snapshot_from_cells(
            2,
            1,
            vec![inverse_cell(' ', WHITE, BLACK), cell(' ', WHITE, BLACK)],
        );

        let frame = prepare_terminal_draw_frame(snapshot, TerminalMetrics::new(8.0, 16.0));

        assert!(frame.text_runs.is_empty());
        assert_eq!(frame.background_runs.len(), 1);
        assert_eq!(frame.background_runs[0].col, 0);
        assert_eq!(frame.background_runs[0].len, 1);
        assert_eq!(frame.background_runs[0].color, WHITE);
    }

    #[test]
    fn selection_and_cursor_override_inverse_backgrounds() {
        let mut snapshot = snapshot_from_cells(
            2,
            1,
            vec![
                inverse_cell('a', WHITE, BLACK),
                inverse_cell('b', WHITE, BLACK),
            ],
        );
        snapshot.selection = Some(TerminalSelection {
            start_col: 0,
            start_row: 0,
            end_col: 0,
            end_row: 0,
        });
        snapshot.cursor = TerminalCursor {
            col: 1,
            row: 0,
            visible: true,
        };

        let frame = prepare_terminal_draw_frame(snapshot, TerminalMetrics::new(8.0, 16.0));

        assert!(frame.background_runs.is_empty());
        assert_eq!(frame.selection_runs.len(), 1);
        assert_eq!(frame.selection_runs[0].col, 0);
        assert_eq!(frame.cursor_run.as_ref().map(|run| run.col), Some(1));
        assert_eq!(frame.text_runs[0].text, "a");
        assert_eq!(frame.text_runs[0].fg, BLACK);
        assert_eq!(frame.text_runs[0].bg, TERMINAL_SELECTION_BG);
        assert_eq!(frame.text_runs[1].text, "b");
        assert_eq!(frame.text_runs[1].fg, CURSOR_FG);
        assert_eq!(frame.text_runs[1].bg, CURSOR_BG);
    }

    #[test]
    fn draw_frame_key_tracks_snapshot_and_metric_identity() {
        let mut snapshot = snapshot_from_text("ab", 2, 1);
        snapshot.version = 42;
        snapshot.selection = Some(TerminalSelection {
            start_col: 0,
            start_row: 0,
            end_col: 0,
            end_row: 0,
        });
        snapshot.cursor = TerminalCursor {
            col: 1,
            row: 0,
            visible: true,
        };

        let frame = prepare_terminal_draw_frame(snapshot, TerminalMetrics::new(8.0, 16.0));

        assert_eq!(frame.key.version, 42);
        assert_eq!(frame.key.cols, 2);
        assert_eq!(frame.key.rows, 1);
        assert_eq!(frame.key.cursor.col, 1);
        assert!(frame.key.selection.is_some());
        assert_eq!(frame.key.cell_width_bits, 8.0_f32.to_bits());
        assert_eq!(frame.key.cell_height_bits, 16.0_f32.to_bits());
    }

    fn snapshot_from_cells(
        cols: u16,
        rows: u16,
        cells: Vec<TerminalCell>,
    ) -> TerminalRendererSnapshot {
        TerminalRendererSnapshot {
            cols,
            rows,
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

    fn snapshot_from_text(text: &str, cols: u16, rows: u16) -> TerminalRendererSnapshot {
        let mut cells = vec![cell(' ', WHITE, BLACK); usize::from(cols) * usize::from(rows)];
        for (row, line) in text.lines().take(usize::from(rows)).enumerate() {
            for (col, ch) in line.chars().take(usize::from(cols)).enumerate() {
                cells[row * usize::from(cols) + col] = cell(ch, WHITE, BLACK);
            }
        }

        snapshot_from_cells(cols, rows, cells)
    }

    fn cell(ch: char, fg: TerminalColor, bg: TerminalColor) -> TerminalCell {
        TerminalCell {
            ch,
            fg,
            bg,
            bold: false,
            italic: false,
            underline: false,
            inverse: false,
        }
    }

    fn inverse_cell(ch: char, fg: TerminalColor, bg: TerminalColor) -> TerminalCell {
        TerminalCell {
            inverse: true,
            ..cell(ch, fg, bg)
        }
    }
}
