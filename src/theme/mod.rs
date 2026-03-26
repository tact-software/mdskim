use ratatui::style::Color as RatColor;

use crate::document::Color;

/// Color theme for terminal rendering.
pub struct Theme {
    pub(crate) search_fg: RatColor,
    pub(crate) search_bg: RatColor,
    pub(crate) status_fg: RatColor,
    pub(crate) status_bg: RatColor,
    color_map: ColorMap,
}

struct ColorMap {
    cyan: RatColor,
    green: RatColor,
    yellow: RatColor,
    magenta: RatColor,
    red: RatColor,
    blue: RatColor,
    gray: RatColor,
    dark_gray: RatColor,
    white: RatColor,
    black: RatColor,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            search_fg: RatColor::Black,
            search_bg: RatColor::Yellow,
            status_fg: RatColor::Black,
            status_bg: RatColor::Cyan,

            color_map: ColorMap {
                cyan: RatColor::Cyan,
                green: RatColor::Green,
                yellow: RatColor::Yellow,
                magenta: RatColor::Magenta,
                red: RatColor::Red,
                blue: RatColor::Blue,
                gray: RatColor::DarkGray,
                dark_gray: RatColor::Rgb(40, 40, 40),
                white: RatColor::White,
                black: RatColor::Black,
            },
        }
    }

    pub fn light() -> Self {
        Self {
            search_fg: RatColor::White,
            search_bg: RatColor::Blue,
            status_fg: RatColor::White,
            status_bg: RatColor::Blue,

            color_map: ColorMap {
                cyan: RatColor::Blue,
                green: RatColor::Magenta,
                yellow: RatColor::Red,
                magenta: RatColor::Magenta,
                red: RatColor::Red,
                blue: RatColor::Blue,
                gray: RatColor::Gray,
                dark_gray: RatColor::Rgb(240, 240, 240),
                white: RatColor::White,
                black: RatColor::Black,
            },
        }
    }

    pub fn no_color() -> Self {
        Self {
            search_fg: RatColor::Reset,
            search_bg: RatColor::Reset,
            status_fg: RatColor::Reset,
            status_bg: RatColor::Reset,

            color_map: ColorMap {
                cyan: RatColor::Reset,
                green: RatColor::Reset,
                yellow: RatColor::Reset,
                magenta: RatColor::Reset,
                red: RatColor::Reset,
                blue: RatColor::Reset,
                gray: RatColor::Reset,
                dark_gray: RatColor::Reset,
                white: RatColor::Reset,
                black: RatColor::Reset,
            },
        }
    }

    pub fn map_color(&self, color: &Color) -> RatColor {
        match color {
            Color::Cyan => self.color_map.cyan,
            Color::Green => self.color_map.green,
            Color::Yellow => self.color_map.yellow,
            Color::Magenta => self.color_map.magenta,
            Color::Red => self.color_map.red,
            Color::Blue => self.color_map.blue,
            Color::Gray => self.color_map.gray,
            Color::DarkGray => self.color_map.dark_gray,
            Color::White => self.color_map.white,
            Color::Black => self.color_map.black,
            Color::Rgb(r, g, b) => RatColor::Rgb(*r, *g, *b),
        }
    }
}
