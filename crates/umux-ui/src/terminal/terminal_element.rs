// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{
    App, Bounds, ContentMask, Element, ElementId, FontStyle, FontWeight, GlobalElementId, Hsla,
    IntoElement, LayoutId, Pixels, Point, SharedString, Style, TextRun, TextStyle, UnderlineStyle,
    Window, fill, point, px, relative, rgb, size,
};
use umux_terminal::{TerminalColor, TerminalMetrics, TerminalRendererSnapshot};
use umux_ui_kit::MUTED_TEXT;

use crate::terminal::draw_frame::{
    TERMINAL_DEFAULT_BG, TerminalCellRun, TerminalDrawFrame, TerminalTextRun,
    prepare_terminal_draw_frame,
};

const TERMINAL_FONT: &str = "Lilex";
const STATUS_FONT_SIZE: f32 = 13.0;
const STATUS_LINE_HEIGHT: f32 = 18.0;
const STATUS_PADDING: f32 = 12.0;

pub struct UmuxTerminalElement {
    status: SharedString,
    frame: Option<TerminalDrawFrame>,
    on_bounds: Option<Box<dyn Fn(Bounds<Pixels>)>>,
}

pub fn terminal_element(
    status: String,
    snapshot: Option<TerminalRendererSnapshot>,
) -> UmuxTerminalElement {
    UmuxTerminalElement {
        status: status.into(),
        frame: snapshot
            .map(|snapshot| prepare_terminal_draw_frame(snapshot, TerminalMetrics::new(8.0, 16.0))),
        on_bounds: None,
    }
}

impl UmuxTerminalElement {
    pub fn on_bounds(mut self, callback: impl Fn(Bounds<Pixels>) + 'static) -> Self {
        self.on_bounds = Some(Box::new(callback));
        self
    }
}

impl IntoElement for UmuxTerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for UmuxTerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<TerminalDrawFrame>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.min_size.width = px(0.0).into();
        style.min_size.height = px(0.0).into();
        style.flex_grow = 1.0;
        style.flex_shrink = 1.0;
        style.flex_basis = relative(0.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        self.frame.clone()
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(on_bounds) = &self.on_bounds {
            on_bounds(bounds);
        }

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            let Some(frame) = prepaint.take() else {
                paint_status_lines(bounds, self.status.clone(), window, cx);
                return;
            };

            window.paint_quad(fill(bounds, terminal_color(frame.background)));

            for run in &frame.background_runs {
                window.paint_quad(fill(
                    cell_run_bounds(bounds.origin, run),
                    terminal_color(run.color),
                ));
            }

            for run in &frame.selection_runs {
                window.paint_quad(fill(
                    cell_run_bounds(bounds.origin, run),
                    terminal_color(run.color),
                ));
            }

            if let Some(run) = &frame.cursor_run {
                window.paint_quad(fill(
                    cell_run_bounds(bounds.origin, run),
                    terminal_color(run.color),
                ));
            }

            let base_style = window.text_style();
            for run in &frame.text_runs {
                let text_run = terminal_text_run(&base_style, run);
                let line = window.text_system().shape_line(
                    run.text.clone().into(),
                    px(frame.cell_height),
                    &[text_run],
                    Some(px(frame.cell_width)),
                );
                let _ = line.paint(
                    point(bounds.left() + px(run.x), bounds.top() + px(run.y)),
                    px(frame.cell_height),
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
        });
    }
}

pub fn terminal_color_hex(color: TerminalColor) -> u32 {
    (u32::from(color.r) << 16) | (u32::from(color.g) << 8) | u32::from(color.b)
}

pub fn terminal_color(color: TerminalColor) -> Hsla {
    Hsla::from(rgb(terminal_color_hex(color)))
}

pub fn cell_run_bounds(origin: Point<Pixels>, run: &TerminalCellRun) -> Bounds<Pixels> {
    Bounds::new(
        point(origin.x + px(run.x), origin.y + px(run.y)),
        size(px(run.width), px(run.height)),
    )
}

fn terminal_text_run(base_style: &TextStyle, run: &TerminalTextRun) -> TextRun {
    let mut font = base_style.font();
    font.family = TERMINAL_FONT.into();
    if run.bold {
        font.weight = FontWeight::BOLD;
    }
    if run.italic {
        font.style = FontStyle::Italic;
    }

    TextRun {
        len: run.text.len(),
        font,
        color: terminal_color(run.fg),
        background_color: None,
        underline: run.underline.then(|| UnderlineStyle {
            color: Some(terminal_color(run.fg)),
            thickness: px(1.0),
            wavy: false,
        }),
        strikethrough: None,
    }
}

fn paint_status_lines(
    bounds: Bounds<Pixels>,
    status: SharedString,
    window: &mut Window,
    cx: &mut App,
) {
    window.paint_quad(fill(bounds, terminal_color(TERMINAL_DEFAULT_BG)));

    let base_style = window.text_style();
    for (row, line_text) in status_lines(status.as_ref()).into_iter().enumerate() {
        let y = bounds.top() + px(STATUS_PADDING + (row as f32 * STATUS_LINE_HEIGHT));
        if y >= bounds.bottom() {
            break;
        }

        let mut font = base_style.font();
        font.family = TERMINAL_FONT.into();
        let text_run = TextRun {
            len: line_text.len(),
            font,
            color: MUTED_TEXT,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window.text_system().shape_line(
            line_text.into(),
            px(STATUS_FONT_SIZE),
            &[text_run],
            None,
        );
        let _ = line.paint(
            point(bounds.left() + px(STATUS_PADDING), y),
            px(STATUS_LINE_HEIGHT),
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        );
    }
}

fn status_lines(status: &str) -> Vec<String> {
    status.split('\n').map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::draw_frame::TerminalTextRun;
    use gpui::{FontStyle, FontWeight};
    use gpui::{point, size};
    use umux_terminal::{TerminalCell, TerminalColor, TerminalCursor, TerminalRendererSnapshot};

    #[test]
    fn terminal_color_hex_packs_rgb_bytes() {
        assert_eq!(
            terminal_color_hex(TerminalColor::rgb(0x11, 0x22, 0x33)),
            0x112233
        );
    }

    #[test]
    fn cell_run_bounds_offsets_from_origin() {
        let run = TerminalCellRun {
            row: 1,
            col: 2,
            len: 3,
            x: 16.0,
            y: 20.0,
            width: 24.0,
            height: 10.0,
            color: TerminalColor::rgb(1, 2, 3),
        };

        let bounds = cell_run_bounds(point(px(4.0), px(5.0)), &run);

        assert_eq!(bounds.origin, point(px(20.0), px(25.0)));
        assert_eq!(bounds.size, size(px(24.0), px(10.0)));
    }

    #[test]
    fn terminal_element_prepares_draw_frame_from_snapshot() {
        let element = terminal_element(
            "pwsh 80x24 running".to_string(),
            Some(snapshot_from_text("ready")),
        );
        let frame = element.frame.expect("snapshot should produce a draw frame");

        assert_eq!(frame.cell_width, 8.0);
        assert_eq!(frame.cell_height, 16.0);
        assert_eq!(frame.background, TERMINAL_DEFAULT_BG);
        assert_eq!(frame.text_runs.len(), 1);
        assert_eq!(frame.text_runs[0].text, "ready");
    }

    #[test]
    fn terminal_element_keeps_status_fallback_without_snapshot() {
        let element = terminal_element("terminal missing".to_string(), None);

        assert_eq!(element.status.as_ref(), "terminal missing");
        assert!(element.frame.is_none());
    }

    #[test]
    fn terminal_text_run_applies_terminal_style_flags() {
        let base_style = gpui::TextStyle::default();
        let run = TerminalTextRun {
            row: 0,
            col: 0,
            text: "styled".to_string(),
            fg: TerminalColor::rgb(0xee, 0xee, 0xee),
            bg: TerminalColor::rgb(0, 0, 0),
            x: 0.0,
            y: 0.0,
            bold: true,
            italic: true,
            underline: true,
            inverse: false,
        };

        let text_run = terminal_text_run(&base_style, &run);

        assert_eq!(text_run.len, "styled".len());
        assert_eq!(text_run.font.family.as_ref(), TERMINAL_FONT);
        assert_eq!(text_run.font.weight, FontWeight::BOLD);
        assert_eq!(text_run.font.style, FontStyle::Italic);
        assert!(text_run.underline.is_some());
    }

    #[test]
    fn status_lines_preserve_newlines_for_single_line_shaping() {
        assert_eq!(
            status_lines("terminal failed:\nread refused"),
            vec!["terminal failed:".to_string(), "read refused".to_string()]
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
