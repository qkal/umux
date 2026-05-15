// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl TerminalColor {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalPalette {
    pub foreground: TerminalColor,
    pub background: TerminalColor,
    pub cursor: TerminalColor,
    pub selection: TerminalColor,
    pub ansi: [TerminalColor; 16],
}

impl Default for TerminalPalette {
    fn default() -> Self {
        Self {
            foreground: TerminalColor::rgb(0xe7, 0xea, 0xf0),
            background: TerminalColor::rgb(0x11, 0x13, 0x16),
            cursor: TerminalColor::rgb(0xe7, 0xea, 0xf0),
            selection: TerminalColor::rgb(0x2f, 0x80, 0xff),
            ansi: [
                TerminalColor::rgb(0x00, 0x00, 0x00),
                TerminalColor::rgb(0xcd, 0x31, 0x31),
                TerminalColor::rgb(0x0d, 0xa0, 0x47),
                TerminalColor::rgb(0xe5, 0xe5, 0x10),
                TerminalColor::rgb(0x24, 0x73, 0xc8),
                TerminalColor::rgb(0xbc, 0x3f, 0xbc),
                TerminalColor::rgb(0x11, 0xa8, 0xcd),
                TerminalColor::rgb(0xe5, 0xe5, 0xe5),
                TerminalColor::rgb(0x66, 0x66, 0x66),
                TerminalColor::rgb(0xf1, 0x4c, 0x4c),
                TerminalColor::rgb(0x23, 0xd1, 0x8b),
                TerminalColor::rgb(0xf5, 0xf5, 0x43),
                TerminalColor::rgb(0x3b, 0x8e, 0xff),
                TerminalColor::rgb(0xd6, 0x70, 0xd6),
                TerminalColor::rgb(0x29, 0xb8, 0xdb),
                TerminalColor::rgb(0xff, 0xff, 0xff),
            ],
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TerminalAppearance {
    pub font_family_order: Vec<String>,
    pub font_size_px: f32,
    pub palette: TerminalPalette,
}

impl Default for TerminalAppearance {
    fn default() -> Self {
        Self {
            font_family_order: vec![
                "Cascadia Mono".to_owned(),
                "Consolas".to_owned(),
                "monospace".to_owned(),
            ],
            font_size_px: 14.0,
            palette: TerminalPalette::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct TerminalMetrics {
    cell_width_px: f32,
    cell_height_px: f32,
}

impl TerminalMetrics {
    pub fn new(cell_width_px: f32, cell_height_px: f32) -> Self {
        Self {
            cell_width_px: normalize_cell_size(cell_width_px),
            cell_height_px: normalize_cell_size(cell_height_px),
        }
    }

    pub fn cell_width_px(&self) -> f32 {
        self.cell_width_px
    }

    pub fn cell_height_px(&self) -> f32 {
        self.cell_height_px
    }

    pub fn cols_rows(&self, width_px: f32, height_px: f32) -> TerminalGridSize {
        TerminalGridSize {
            cols: ((width_px / self.cell_width_px).floor() as u16).max(1),
            rows: ((height_px / self.cell_height_px).floor() as u16).max(1),
        }
    }
}

impl<'de> Deserialize<'de> for TerminalMetrics {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct TerminalMetricsFields {
            cell_width_px: f32,
            cell_height_px: f32,
        }

        let fields = TerminalMetricsFields::deserialize(deserializer)?;

        Ok(Self::new(fields.cell_width_px, fields.cell_height_px))
    }
}

fn normalize_cell_size(value: f32) -> f32 {
    if value.is_finite() && value >= 1.0 {
        value
    } else {
        1.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalGridSize {
    pub cols: u16,
    pub rows: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_appearance_uses_windows_monospace_fallbacks() {
        let appearance = TerminalAppearance::default();

        assert_eq!(appearance.font_family_order[0], "Cascadia Mono");
        assert_eq!(appearance.font_family_order[1], "Consolas");
        assert_eq!(appearance.font_size_px, 14.0);
        assert_eq!(appearance.palette.ansi.len(), 16);
    }

    #[test]
    fn default_palette_uses_exact_terminal_colors() {
        let palette = TerminalPalette::default();

        assert_eq!(palette.foreground, TerminalColor::rgb(0xe7, 0xea, 0xf0));
        assert_eq!(palette.background, TerminalColor::rgb(0x11, 0x13, 0x16));
        assert_eq!(palette.cursor, TerminalColor::rgb(0xe7, 0xea, 0xf0));
        assert_eq!(palette.selection, TerminalColor::rgb(0x2f, 0x80, 0xff));
        assert_eq!(
            palette.ansi,
            [
                TerminalColor::rgb(0x00, 0x00, 0x00),
                TerminalColor::rgb(0xcd, 0x31, 0x31),
                TerminalColor::rgb(0x0d, 0xa0, 0x47),
                TerminalColor::rgb(0xe5, 0xe5, 0x10),
                TerminalColor::rgb(0x24, 0x73, 0xc8),
                TerminalColor::rgb(0xbc, 0x3f, 0xbc),
                TerminalColor::rgb(0x11, 0xa8, 0xcd),
                TerminalColor::rgb(0xe5, 0xe5, 0xe5),
                TerminalColor::rgb(0x66, 0x66, 0x66),
                TerminalColor::rgb(0xf1, 0x4c, 0x4c),
                TerminalColor::rgb(0x23, 0xd1, 0x8b),
                TerminalColor::rgb(0xf5, 0xf5, 0x43),
                TerminalColor::rgb(0x3b, 0x8e, 0xff),
                TerminalColor::rgb(0xd6, 0x70, 0xd6),
                TerminalColor::rgb(0x29, 0xb8, 0xdb),
                TerminalColor::rgb(0xff, 0xff, 0xff),
            ]
        );
    }

    #[test]
    fn metrics_never_return_zero_rows_or_columns() {
        let metrics = TerminalMetrics::new(8.0, 16.0);

        assert_eq!(metrics.cell_width_px(), 8.0);
        assert_eq!(metrics.cell_height_px(), 16.0);
        assert_eq!(
            metrics.cols_rows(1.0, 1.0),
            TerminalGridSize { cols: 1, rows: 1 }
        );
        assert_eq!(
            metrics.cols_rows(80.0, 32.0),
            TerminalGridSize { cols: 10, rows: 2 }
        );
    }

    #[test]
    fn metrics_normalize_invalid_constructor_inputs() {
        for metrics in [
            TerminalMetrics::new(0.0, 0.0),
            TerminalMetrics::new(-8.0, -16.0),
            TerminalMetrics::new(f32::NAN, f32::INFINITY),
        ] {
            assert_eq!(metrics.cell_width_px(), 1.0);
            assert_eq!(metrics.cell_height_px(), 1.0);
            assert_eq!(
                metrics.cols_rows(80.0, 32.0),
                TerminalGridSize { cols: 80, rows: 32 }
            );
        }
    }

    #[test]
    fn metrics_normalize_invalid_deserialized_cell_sizes() {
        use serde::Deserialize;
        use serde::de::value::{Error, MapDeserializer};

        let deserializer = MapDeserializer::<_, Error>::new(
            [
                ("cell_width_px", 0.0_f32),
                ("cell_height_px", f32::INFINITY),
            ]
            .into_iter(),
        );
        let metrics = TerminalMetrics::deserialize(deserializer).unwrap();

        assert_eq!(metrics.cell_width_px(), 1.0);
        assert_eq!(metrics.cell_height_px(), 1.0);
        assert_eq!(
            metrics.cols_rows(80.0, 32.0),
            TerminalGridSize { cols: 80, rows: 32 }
        );
    }
}
