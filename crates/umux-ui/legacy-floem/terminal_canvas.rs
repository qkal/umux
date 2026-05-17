// SPDX-License-Identifier: GPL-3.0-or-later

use std::any::Any;

use floem::context::{PaintCx, UpdateCx};
use floem::kurbo::{Point, Rect};
use floem::peniko::Color;
use floem::reactive::create_effect;
use floem::text::{Attrs, AttrsList, FamilyOwned, LineHeightValue, TextLayout};
use floem::{View, ViewId};
use floem_renderer::Renderer;
use umux_terminal::{
    TerminalColor, TerminalCursor, TerminalMetrics, TerminalRendererSnapshot, TerminalSelection,
    snapshot::TerminalRenderCell,
};

pub(crate) const TERMINAL_DEFAULT_BG: TerminalColor = TerminalColor::rgb(0x11, 0x13, 0x16);
pub(crate) const TERMINAL_SELECTION_BG: TerminalColor = TerminalColor::rgb(0x2f, 0x80, 0xff);
pub(crate) const CURSOR_FG: TerminalColor = TERMINAL_DEFAULT_BG;
pub(crate) const CURSOR_BG: TerminalColor = TerminalColor::rgb(0xe7, 0xea, 0xf0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalRendererMode {
    Painted,
    Legacy,
}

impl TerminalRendererMode {
    pub(crate) fn from_env_value(value: Option<&str>) -> Self {
        match value {
            Some(value) if value.trim().eq_ignore_ascii_case("legacy") => Self::Legacy,
            _ => Self::Painted,
        }
    }

    pub(crate) fn current() -> Self {
        Self::from_env_value(std::env::var("UMUX_TERMINAL_RENDERER").ok().as_deref())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalDrawFrameKey {
    version: u64,
    cols: u16,
    rows: u16,
    cursor: TerminalCursor,
    selection: Option<TerminalSelection>,
    cell_width_bits: u32,
    cell_height_bits: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TerminalCellRun {
    pub(crate) row: u16,
    pub(crate) col: u16,
    pub(crate) len: u16,
    pub(crate) x_px: f32,
    pub(crate) y_px: f32,
    pub(crate) width_px: f32,
    pub(crate) height_px: f32,
    pub(crate) color: TerminalColor,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TerminalTextRun {
    pub(crate) row: u16,
    pub(crate) col: u16,
    pub(crate) text: String,
    pub(crate) fg: TerminalColor,
    pub(crate) bg: TerminalColor,
    pub(crate) x_px: f32,
    pub(crate) y_px: f32,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) underline: bool,
    pub(crate) inverse: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TerminalDrawFrame {
    pub(crate) key: TerminalDrawFrameKey,
    pub(crate) cols: u16,
    pub(crate) rows: u16,
    pub(crate) cell_width_px: f32,
    pub(crate) cell_height_px: f32,
    pub(crate) background: TerminalColor,
    pub(crate) background_runs: Vec<TerminalCellRun>,
    pub(crate) selection_runs: Vec<TerminalCellRun>,
    pub(crate) cursor_run: Option<TerminalCellRun>,
    pub(crate) text_runs: Vec<TerminalTextRun>,
}

pub(crate) fn prepare_terminal_draw_frame(
    snapshot: TerminalRendererSnapshot,
    metrics: TerminalMetrics,
) -> TerminalDrawFrame {
    let cell_width_px = metrics.cell_width_px();
    let cell_height_px = metrics.cell_height_px();
    let key = TerminalDrawFrameKey {
        version: snapshot.version,
        cols: snapshot.cols,
        rows: snapshot.rows,
        cursor: snapshot.cursor,
        selection: snapshot.selection,
        cell_width_bits: cell_width_px.to_bits(),
        cell_height_bits: cell_height_px.to_bits(),
    };

    if snapshot.cols == 0 {
        return TerminalDrawFrame {
            key,
            cols: snapshot.cols,
            rows: snapshot.rows,
            cell_width_px,
            cell_height_px,
            background: TERMINAL_DEFAULT_BG,
            background_runs: Vec::new(),
            selection_runs: Vec::new(),
            cursor_run: None,
            text_runs: Vec::new(),
        };
    }

    let render_cells = snapshot.render_cells();
    let mut frame = TerminalDrawFrame {
        key,
        cols: snapshot.cols,
        rows: snapshot.rows,
        cell_width_px,
        cell_height_px,
        background: TERMINAL_DEFAULT_BG,
        background_runs: Vec::new(),
        selection_runs: Vec::new(),
        cursor_run: None,
        text_runs: Vec::new(),
    };

    for row_cells in render_cells.chunks(usize::from(snapshot.cols)) {
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

pub(crate) struct TerminalPaintedGrid {
    id: ViewId,
    frame: TerminalDrawFrame,
}

pub(crate) fn terminal_painted_grid(
    frame: impl Fn() -> TerminalDrawFrame + 'static,
) -> TerminalPaintedGrid {
    let id = ViewId::new();
    let initial = frame();
    create_effect(move |_| {
        id.update_state(frame());
    });

    TerminalPaintedGrid { id, frame: initial }
}

impl View for TerminalPaintedGrid {
    fn id(&self) -> ViewId {
        self.id
    }

    fn debug_name(&self) -> std::borrow::Cow<'static, str> {
        "TerminalPaintedGrid".into()
    }

    fn update(&mut self, _cx: &mut UpdateCx, state: Box<dyn Any>) {
        if let Ok(frame) = state.downcast::<TerminalDrawFrame>() {
            let next = *frame;
            if self.frame.cols != next.cols
                || self.frame.rows != next.rows
                || self.frame.cell_width_px.to_bits() != next.cell_width_px.to_bits()
                || self.frame.cell_height_px.to_bits() != next.cell_height_px.to_bits()
            {
                self.id.request_layout();
            } else {
                self.id.request_paint();
            }
            self.frame = next;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx) {
        let rect = self.id.get_content_rect();
        cx.fill(&rect, color_to_floem(self.frame.background), 0.0);

        for run in &self.frame.background_runs {
            paint_cell_run(cx, run);
        }
        for run in &self.frame.selection_runs {
            paint_cell_run(cx, run);
        }
        if let Some(run) = &self.frame.cursor_run {
            paint_cell_run(cx, run);
        }
        for run in &self.frame.text_runs {
            paint_text_run(cx, run, self.frame.cell_height_px);
        }
    }
}

fn paint_cell_run(cx: &mut PaintCx, run: &TerminalCellRun) {
    let rect = Rect::new(
        f64::from(run.x_px),
        f64::from(run.y_px),
        f64::from(run.x_px + run.width_px),
        f64::from(run.y_px + run.height_px),
    );
    cx.fill(&rect, color_to_floem(run.color), 0.0);
}

fn paint_text_run(cx: &mut PaintCx, run: &TerminalTextRun, cell_height_px: f32) {
    let mut text_layout = TextLayout::new();
    let families: Vec<FamilyOwned> =
        FamilyOwned::parse_list("Cascadia Mono, Consolas, monospace").collect();
    let attrs = Attrs::new()
        .color(color_to_floem(run.fg))
        .family(&families)
        .font_size(13.0)
        .line_height(LineHeightValue::Px(cell_height_px));
    text_layout.set_text(&run.text, AttrsList::new(attrs));
    cx.draw_text(
        &text_layout,
        Point::new(f64::from(run.x_px), f64::from(run.y_px)),
    );
}

fn color_to_floem(color: TerminalColor) -> Color {
    Color::rgb8(color.r, color.g, color.b)
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
        if cell.cell.bg == TERMINAL_DEFAULT_BG || cell.selected || cell.cursor {
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
            Some(_) if cell.cell.bg == run_color && cell.col == previous_col => {}
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
                run_color = cell.cell.bg;
            }
            None => {
                run_start = Some(cell.col);
                run_color = cell.cell.bg;
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
    let mut current: Option<TerminalTextRun> = None;

    for cell in cells {
        let fg = if cell.cursor { CURSOR_FG } else { cell.cell.fg };
        let bg = if cell.cursor {
            CURSOR_BG
        } else if cell.selected {
            TERMINAL_SELECTION_BG
        } else {
            cell.cell.bg
        };

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
                x_px: f32::from(cell.col) * metrics.cell_width_px(),
                y_px: f32::from(cell.row) * metrics.cell_height_px(),
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
        x_px: f32::from(col) * metrics.cell_width_px(),
        y_px: f32::from(row) * metrics.cell_height_px(),
        width_px: f32::from(len) * metrics.cell_width_px(),
        height_px: metrics.cell_height_px(),
        color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use umux_terminal::{
        TerminalCell, TerminalColor, TerminalCursor, TerminalMetrics, TerminalRendererSnapshot,
    };

    const BLACK: TerminalColor = TerminalColor::rgb(0x11, 0x13, 0x16);
    const WHITE: TerminalColor = TerminalColor::rgb(0xe7, 0xea, 0xf0);
    const RED: TerminalColor = TerminalColor::rgb(0xcd, 0x31, 0x31);

    #[test]
    fn renderer_mode_defaults_to_painted_and_parses_legacy() {
        assert_eq!(
            TerminalRendererMode::from_env_value(None),
            TerminalRendererMode::Painted
        );
        assert_eq!(
            TerminalRendererMode::from_env_value(Some("legacy")),
            TerminalRendererMode::Legacy
        );
        assert_eq!(
            TerminalRendererMode::from_env_value(Some("painted")),
            TerminalRendererMode::Painted
        );
        assert_eq!(
            TerminalRendererMode::from_env_value(Some("unknown")),
            TerminalRendererMode::Painted
        );
    }

    #[test]
    fn renderer_mode_trims_env_value_whitespace() {
        assert_eq!(
            TerminalRendererMode::from_env_value(Some(" legacy ")),
            TerminalRendererMode::Legacy
        );
    }

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
        assert_eq!(frame.text_runs[0].x_px, 8.0);
        assert_eq!(frame.text_runs[1].text, "!");
        assert_eq!(frame.text_runs[1].col, 3);
        assert_eq!(frame.text_runs[1].x_px, 24.0);
    }

    #[test]
    fn selection_only_change_affects_frame_key_and_selection_runs() {
        let mut snapshot = snapshot_from_text("abc", 3, 1);
        snapshot.version = 42;
        let unselected =
            prepare_terminal_draw_frame(snapshot.clone(), TerminalMetrics::new(8.0, 16.0));
        snapshot.selection = Some(TerminalSelection {
            start_col: 1,
            start_row: 0,
            end_col: 2,
            end_row: 0,
        });

        let selected = prepare_terminal_draw_frame(snapshot, TerminalMetrics::new(8.0, 16.0));

        assert_ne!(unselected.key, selected.key);
        assert_eq!(selected.selection_runs.len(), 1);
        assert_eq!(selected.selection_runs[0].col, 1);
        assert_eq!(selected.selection_runs[0].len, 2);
    }

    #[test]
    fn cursor_uses_existing_terminal_cursor_colors() {
        let mut snapshot = snapshot_from_text("ab", 2, 1);
        snapshot.cursor = TerminalCursor {
            col: 1,
            row: 0,
            visible: true,
        };

        let frame = prepare_terminal_draw_frame(snapshot, TerminalMetrics::new(8.0, 16.0));

        assert_eq!(frame.cursor_run.unwrap().col, 1);
        assert_eq!(frame.text_runs[1].fg, CURSOR_FG);
    }

    #[test]
    fn repeated_background_cells_are_grouped_into_one_run() {
        let bg = TerminalColor::rgb(10, 20, 30);
        let snapshot = snapshot_from_cells(
            4,
            1,
            vec![
                cell('a', WHITE, bg),
                cell('b', WHITE, bg),
                cell('c', WHITE, BLACK),
                cell('d', WHITE, bg),
            ],
        );

        let frame = prepare_terminal_draw_frame(snapshot, TerminalMetrics::new(8.0, 16.0));

        assert_eq!(frame.background_runs.len(), 2);
        assert_eq!(frame.background_runs[0].col, 0);
        assert_eq!(frame.background_runs[0].len, 2);
        assert_eq!(frame.background_runs[1].col, 3);
        assert_eq!(frame.background_runs[1].len, 1);
    }

    #[test]
    fn zero_column_snapshot_prepares_empty_draw_lists() {
        let snapshot = TerminalRendererSnapshot {
            cols: 0,
            rows: 2,
            cells: vec![cell('x', WHITE, BLACK)],
            cursor: TerminalCursor {
                col: 0,
                row: 0,
                visible: true,
            },
            selection: None,
            scrollback_lines: 0,
            version: 1,
        };

        let frame = prepare_terminal_draw_frame(snapshot, TerminalMetrics::new(8.0, 16.0));

        assert!(frame.background_runs.is_empty());
        assert!(frame.selection_runs.is_empty());
        assert!(frame.text_runs.is_empty());
        assert_eq!(frame.cursor_run, None);
    }

    #[test]
    fn painted_grid_view_is_constructible_from_frame_closure() {
        let frame = prepare_terminal_draw_frame(
            snapshot_from_text("ready", 5, 1),
            TerminalMetrics::new(8.0, 16.0),
        );

        let grid = terminal_painted_grid(move || frame.clone());

        let _ = grid.id();
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
}
