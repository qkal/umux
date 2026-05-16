// SPDX-License-Identifier: GPL-3.0-or-later

use std::mem;
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor, Processor, Rgb};
use umux_notify::{TerminalNotification, parse_osc_notifications};

use crate::{
    TerminalCell, TerminalColor, TerminalCursor, TerminalEvent, TerminalPalette,
    TerminalRendererSnapshot, TerminalSelection,
};

#[derive(Clone, Debug, Default)]
pub struct TerminalEventSink {
    events: Arc<Mutex<Vec<TerminalEvent>>>,
}

impl TerminalEventSink {
    pub fn drain(&self) -> Vec<TerminalEvent> {
        let mut events = self
            .events
            .lock()
            .expect("terminal event sink lock poisoned");
        mem::take(&mut *events)
    }
}

impl EventListener for TerminalEventSink {
    fn send_event(&self, event: Event) {
        let mapped = match event {
            Event::Title(title) => Some(TerminalEvent::TitleChanged(title)),
            Event::ResetTitle => Some(TerminalEvent::TitleChanged("Terminal".to_string())),
            Event::Bell => Some(TerminalEvent::Bell),
            Event::Wakeup => Some(TerminalEvent::Wakeup),
            Event::ChildExit(status) => Some(TerminalEvent::ChildExited(status.code())),
            Event::ClipboardStore(_, _)
            | Event::ClipboardLoad(_, _)
            | Event::ColorRequest(_, _)
            | Event::PtyWrite(_)
            | Event::TextAreaSizeRequest(_)
            | Event::CursorBlinkingChange
            | Event::MouseCursorDirty
            | Event::Exit => None,
        };

        if let Some(event) = mapped {
            self.events
                .lock()
                .expect("terminal event sink lock poisoned")
                .push(event);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalDimensions {
    cols: u16,
    rows: u16,
}

impl TerminalDimensions {
    pub(crate) fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols: cols.max(1),
            rows: rows.max(1),
        }
    }
}

impl Dimensions for TerminalDimensions {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        usize::from(self.rows)
    }

    fn columns(&self) -> usize {
        usize::from(self.cols)
    }
}

pub struct TerminalEmulator {
    processor: Processor,
    term: Term<TerminalEventSink>,
    dimensions: TerminalDimensions,
    sink: TerminalEventSink,
    palette: TerminalPalette,
    pending_output: Vec<u8>,
    version: u64,
}

impl TerminalEmulator {
    pub fn new(cols: u16, rows: u16, scrollback_limit: usize) -> Self {
        let dimensions = TerminalDimensions::new(cols, rows);
        let sink = TerminalEventSink::default();
        let config = Config {
            scrolling_history: scrollback_limit,
            ..Default::default()
        };
        let term = Term::new(config, &dimensions, sink.clone());

        Self {
            processor: Processor::new(),
            term,
            dimensions,
            sink,
            palette: TerminalPalette::default(),
            pending_output: Vec::new(),
            version: 0,
        }
    }

    pub fn feed_bytes(&mut self, bytes: &[u8]) -> Vec<TerminalNotification> {
        self.processor.advance(&mut self.term, bytes);
        self.version = self.version.saturating_add(1);

        let buffered_output = if self.pending_output.is_empty() {
            bytes.to_vec()
        } else {
            let mut buffered_output = Vec::with_capacity(self.pending_output.len() + bytes.len());
            buffered_output.extend_from_slice(&self.pending_output);
            buffered_output.extend_from_slice(bytes);
            buffered_output
        };

        let complete_len = if let Some(osc_start) = trailing_incomplete_osc_start(&buffered_output)
        {
            self.pending_output = buffered_output[osc_start..].to_vec();
            osc_start
        } else {
            self.pending_output.clear();
            buffered_output.len()
        };

        parse_osc_notifications(&String::from_utf8_lossy(&buffered_output[..complete_len]))
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.dimensions = TerminalDimensions::new(cols, rows);
        self.term.resize(self.dimensions);
        self.version = self.version.saturating_add(1);
    }

    pub fn drain_events(&self) -> Vec<TerminalEvent> {
        self.sink.drain()
    }

    pub fn screen_text(&self, include_scrollback: bool) -> String {
        screen_text_from_term(&self.term, include_scrollback)
    }

    pub fn clear_scrollback(&mut self) {
        self.term.grid_mut().clear_history();
        self.version = self.version.saturating_add(1);
    }

    pub fn snapshot(&self) -> TerminalRendererSnapshot {
        snapshot_from_term(&self.term, &self.palette, self.version)
    }
}

pub(crate) fn screen_text_from_term(
    term: &Term<TerminalEventSink>,
    include_scrollback: bool,
) -> String {
    let grid = term.grid();
    let cols = term.columns();
    if cols == 0 {
        return String::new();
    }

    let start_line = if include_scrollback {
        grid.topmost_line().0
    } else {
        0
    };
    let end_line = grid.bottommost_line().0;

    (start_line..=end_line)
        .map(|line| {
            (0..cols)
                .map(|col| {
                    let cell = &grid[Line(line)][Column(col)];
                    if cell.flags.contains(Flags::HIDDEN) {
                        ' '
                    } else {
                        cell.c
                    }
                })
                .collect::<String>()
                .trim_end_matches(' ')
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .to_string()
}

pub(crate) fn snapshot_from_term(
    term: &Term<TerminalEventSink>,
    palette: &TerminalPalette,
    version: u64,
) -> TerminalRendererSnapshot {
    let rows = to_u16(term.screen_lines());
    let cols = to_u16(term.columns());
    let mut cells = vec![blank_cell(palette); usize::from(rows) * usize::from(cols)];

    let content = term.renderable_content();
    let display_offset = content.display_offset as i32;
    for indexed in content.display_iter {
        let Some(row) = visible_row(indexed.point.line.0, display_offset, rows) else {
            continue;
        };
        let col = indexed.point.column.0;
        if col >= usize::from(cols) {
            continue;
        }

        let index = usize::from(row) * usize::from(cols) + col;
        cells[index] = map_cell(indexed.cell, content.colors, palette);
    }

    let cursor_row = visible_row(content.cursor.point.line.0, display_offset, rows).unwrap_or(0);
    let cursor_col = content
        .cursor
        .point
        .column
        .0
        .min(usize::from(cols.saturating_sub(1)));
    let cursor = TerminalCursor {
        col: to_u16(cursor_col),
        row: cursor_row,
        visible: content.cursor.shape != CursorShape::Hidden,
    };
    let selection = content.selection.and_then(|selection| {
        clip_selection_to_viewport(
            selection.start.column.0,
            selection.start.line.0,
            selection.end.column.0,
            selection.end.line.0,
            display_offset,
            cols,
            rows,
        )
    });

    TerminalRendererSnapshot {
        cols,
        rows,
        cells,
        cursor,
        selection,
        scrollback_lines: term.grid().history_size().min(u32::MAX as usize) as u32,
        version,
    }
}

fn blank_cell(palette: &TerminalPalette) -> TerminalCell {
    TerminalCell {
        ch: ' ',
        fg: palette.foreground,
        bg: palette.background,
        bold: false,
        italic: false,
        underline: false,
        inverse: false,
    }
}

fn map_cell(cell: &Cell, colors: &Colors, palette: &TerminalPalette) -> TerminalCell {
    TerminalCell {
        ch: if cell.flags.contains(Flags::HIDDEN) {
            ' '
        } else {
            cell.c
        },
        fg: map_color(cell.fg, colors, palette),
        bg: map_color(cell.bg, colors, palette),
        bold: cell.flags.contains(Flags::BOLD),
        italic: cell.flags.contains(Flags::ITALIC),
        underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
        inverse: cell.flags.contains(Flags::INVERSE),
    }
}

fn map_color(color: AnsiColor, colors: &Colors, palette: &TerminalPalette) -> TerminalColor {
    match color {
        AnsiColor::Named(named) => colors[named]
            .map(TerminalColor::from)
            .unwrap_or_else(|| map_named_color(named, palette)),
        AnsiColor::Spec(rgb) => rgb.into(),
        AnsiColor::Indexed(index) => colors[usize::from(index)]
            .map(TerminalColor::from)
            .unwrap_or_else(|| map_indexed_color(index, palette)),
    }
}

fn map_named_color(color: NamedColor, palette: &TerminalPalette) -> TerminalColor {
    match color {
        NamedColor::Black => palette.ansi[0],
        NamedColor::Red => palette.ansi[1],
        NamedColor::Green => palette.ansi[2],
        NamedColor::Yellow => palette.ansi[3],
        NamedColor::Blue => palette.ansi[4],
        NamedColor::Magenta => palette.ansi[5],
        NamedColor::Cyan => palette.ansi[6],
        NamedColor::White => palette.ansi[7],
        NamedColor::BrightBlack => palette.ansi[8],
        NamedColor::BrightRed => palette.ansi[9],
        NamedColor::BrightGreen => palette.ansi[10],
        NamedColor::BrightYellow => palette.ansi[11],
        NamedColor::BrightBlue => palette.ansi[12],
        NamedColor::BrightMagenta => palette.ansi[13],
        NamedColor::BrightCyan => palette.ansi[14],
        NamedColor::BrightWhite => palette.ansi[15],
        NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::DimForeground => {
            palette.foreground
        }
        NamedColor::Background => palette.background,
        NamedColor::Cursor => palette.cursor,
        NamedColor::DimBlack => dim(palette.ansi[0]),
        NamedColor::DimRed => dim(palette.ansi[1]),
        NamedColor::DimGreen => dim(palette.ansi[2]),
        NamedColor::DimYellow => dim(palette.ansi[3]),
        NamedColor::DimBlue => dim(palette.ansi[4]),
        NamedColor::DimMagenta => dim(palette.ansi[5]),
        NamedColor::DimCyan => dim(palette.ansi[6]),
        NamedColor::DimWhite => dim(palette.ansi[7]),
    }
}

fn map_indexed_color(index: u8, palette: &TerminalPalette) -> TerminalColor {
    match index {
        0..=15 => palette.ansi[usize::from(index)],
        16..=231 => {
            let value = index - 16;
            let r = value / 36;
            let g = (value % 36) / 6;
            let b = value % 6;
            TerminalColor::rgb(
                color_cube_channel(r),
                color_cube_channel(g),
                color_cube_channel(b),
            )
        }
        232..=255 => {
            let channel = 8 + (index - 232) * 10;
            TerminalColor::rgb(channel, channel, channel)
        }
    }
}

impl From<Rgb> for TerminalColor {
    fn from(value: Rgb) -> Self {
        Self::rgb(value.r, value.g, value.b)
    }
}

fn visible_row(line: i32, display_offset: i32, rows: u16) -> Option<u16> {
    let row = line + display_offset;
    if row < 0 || row >= i32::from(rows) {
        None
    } else {
        Some(row as u16)
    }
}

fn clip_selection_to_viewport(
    start_col: usize,
    start_line: i32,
    end_col: usize,
    end_line: i32,
    display_offset: i32,
    cols: u16,
    rows: u16,
) -> Option<TerminalSelection> {
    if cols == 0 || rows == 0 {
        return None;
    }

    let start_row = start_line + display_offset;
    let end_row = end_line + display_offset;
    let last_row = i32::from(rows) - 1;
    if end_row < 0 || start_row > last_row {
        return None;
    }

    let last_col = usize::from(cols) - 1;
    let (start_row, start_col) = if start_row < 0 {
        (0, 0)
    } else {
        (start_row as u16, to_u16(start_col.min(last_col)))
    };
    let (end_row, end_col) = if end_row > last_row {
        (last_row as u16, to_u16(last_col))
    } else {
        (end_row as u16, to_u16(end_col.min(last_col)))
    };

    Some(TerminalSelection {
        start_col,
        start_row,
        end_col,
        end_row,
    })
}

fn trailing_incomplete_osc_start(output: &[u8]) -> Option<usize> {
    if let Some(osc_start) = output.windows(2).rposition(|window| window == b"\x1b]") {
        let trailing = &output[osc_start..];
        if !trailing.contains(&b'\x07') && !trailing.windows(2).any(|window| window == b"\x1b\\") {
            return Some(osc_start);
        }
    }

    match output {
        [.., b'\x1b'] => Some(output.len() - 1),
        _ => None,
    }
}

fn to_u16(value: usize) -> u16 {
    value.min(usize::from(u16::MAX)) as u16
}

fn dim(color: TerminalColor) -> TerminalColor {
    TerminalColor::rgb(
        ((u16::from(color.r) * 2) / 3) as u8,
        ((u16::from(color.g) * 2) / 3) as u8,
        ((u16::from(color.b) * 2) / 3) as u8,
    )
}

fn color_cube_channel(value: u8) -> u8 {
    if value == 0 { 0 } else { 55 + value * 40 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TerminalPalette;

    #[test]
    fn emulator_renders_plain_text_snapshot() {
        let mut emulator = TerminalEmulator::new(10, 3, 100);

        let notifications = emulator.feed_bytes(b"hello\r\nworld");
        let snapshot = emulator.snapshot();

        assert!(notifications.is_empty());
        assert!(snapshot.visible_text().contains("hello"));
        assert!(snapshot.visible_text().contains("world"));
        assert_eq!(snapshot.cols, 10);
        assert_eq!(snapshot.rows, 3);
    }

    #[test]
    fn emulator_preserves_ansi_named_colors() {
        let mut emulator = TerminalEmulator::new(10, 2, 100);

        emulator.feed_bytes(b"\x1b[31mred");
        let snapshot = emulator.snapshot();
        let first = snapshot.cells.iter().find(|cell| cell.ch == 'r').unwrap();

        assert_eq!(first.fg, TerminalPalette::default().ansi[1]);
    }

    #[test]
    fn emulator_extracts_osc_notifications() {
        let mut emulator = TerminalEmulator::new(20, 3, 100);

        let notifications = emulator.feed_bytes(b"\x1b]9;ready\x07");

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].message, "ready");
    }

    #[test]
    fn emulator_ignores_empty_feed_without_panicking() {
        let mut emulator = TerminalEmulator::new(20, 3, 100);

        let notifications = emulator.feed_bytes(b"");

        assert!(notifications.is_empty());
    }

    #[test]
    fn emulator_extracts_osc_notifications_split_across_chunks() {
        let mut emulator = TerminalEmulator::new(20, 3, 100);

        assert!(emulator.feed_bytes(b"\x1b]9;rea").is_empty());
        let notifications = emulator.feed_bytes(b"dy\x07");

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].message, "ready");
    }

    #[test]
    fn emulator_extracts_osc_notifications_split_between_escape_and_bracket() {
        let mut emulator = TerminalEmulator::new(20, 3, 100);

        assert!(emulator.feed_bytes(b"\x1b").is_empty());
        let notifications = emulator.feed_bytes(b"]9;ready\x07");

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].message, "ready");
    }

    #[test]
    fn emulator_retains_trailing_escape_after_complete_osc() {
        let mut emulator = TerminalEmulator::new(20, 3, 100);

        let first = emulator.feed_bytes(b"\x1b]9;one\x07\x1b");
        let second = emulator.feed_bytes(b"]9;two\x07");

        assert_eq!(first.len(), 1);
        assert_eq!(first[0].message, "one");
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].message, "two");
    }

    #[test]
    fn emulator_extracts_osc_notifications_with_split_st_terminator() {
        let mut emulator = TerminalEmulator::new(20, 3, 100);

        assert!(emulator.feed_bytes(b"\x1b]9;ready\x1b").is_empty());
        let notifications = emulator.feed_bytes(b"\\");

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].message, "ready");
    }

    #[test]
    fn emulator_extracts_utf8_osc_notifications_split_across_chunks() {
        let mut emulator = TerminalEmulator::new(20, 3, 100);

        assert!(emulator.feed_bytes(b"\x1b]9;build \xe2\x9c").is_empty());
        let notifications = emulator.feed_bytes(b"\x93\x07");

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].message, "build ✓");
    }

    #[test]
    fn emulator_resize_keeps_minimum_size() {
        let mut emulator = TerminalEmulator::new(10, 3, 100);

        emulator.resize(0, 0);
        let snapshot = emulator.snapshot();

        assert_eq!(snapshot.cols, 1);
        assert_eq!(snapshot.rows, 1);
    }

    #[test]
    fn selection_clip_intersects_visible_viewport() {
        let selection = clip_selection_to_viewport(8, 2, 14, 5, 0, 10, 3).unwrap();

        assert_eq!(
            selection,
            TerminalSelection {
                start_col: 8,
                start_row: 2,
                end_col: 9,
                end_row: 2,
            }
        );
    }

    #[test]
    fn selection_clip_preserves_multiline_endpoint_column_order() {
        let selection = clip_selection_to_viewport(8, 1, 2, 3, 0, 10, 5).unwrap();

        assert_eq!(
            selection,
            TerminalSelection {
                start_col: 8,
                start_row: 1,
                end_col: 2,
                end_row: 3,
            }
        );
    }

    #[test]
    fn selection_clip_drops_fully_offscreen_selection() {
        assert_eq!(clip_selection_to_viewport(0, -3, 4, -1, 0, 10, 3), None);
    }

    #[test]
    fn selection_clip_clamps_visible_endpoint_columns_independently() {
        let selection = clip_selection_to_viewport(10, 0, 14, 1, 0, 10, 3).unwrap();

        assert_eq!(
            selection,
            TerminalSelection {
                start_col: 9,
                start_row: 0,
                end_col: 9,
                end_row: 1,
            }
        );
    }
}
