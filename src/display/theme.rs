use anstyle::Color;
use clap::builder::styling::{Style as ClapStyle, Styles};

pub mod palette {
    use anstyle::{Color, RgbColor};
    pub const BASE: Color = Color::Rgb(RgbColor(35, 33, 54)); // #232136
    pub const SURFACE: Color = Color::Rgb(RgbColor(42, 39, 63)); // #2a273f
    pub const OVERLAY: Color = Color::Rgb(RgbColor(57, 53, 82)); // #393552
    pub const MUTED: Color = Color::Rgb(RgbColor(110, 106, 134)); // #6e6a86
    pub const SUBTLE: Color = Color::Rgb(RgbColor(144, 140, 170)); // #908caa
    pub const TEXT: Color = Color::Rgb(RgbColor(224, 222, 244)); // #e0def4
    pub const LOVE: Color = Color::Rgb(RgbColor(235, 111, 146)); // #eb6f92
    pub const GOLD: Color = Color::Rgb(RgbColor(246, 193, 119)); // #f6c177
    pub const ROSE: Color = Color::Rgb(RgbColor(234, 154, 151)); // #ea9a97
    pub const PINE: Color = Color::Rgb(RgbColor(62, 143, 176)); // #3e8fb0
    pub const FOAM: Color = Color::Rgb(RgbColor(156, 207, 216)); // #9ccfd8
    pub const IRIS: Color = Color::Rgb(RgbColor(196, 167, 231)); // #c4a7e7
}

#[must_use]
pub fn rose_pine_moon() -> Styles {
    let header = ClapStyle::new().fg_color(Some(palette::IRIS)).bold();
    let usage = ClapStyle::new().fg_color(Some(palette::PINE)).bold();
    let literal = ClapStyle::new().fg_color(Some(palette::GOLD));
    let placeholder = ClapStyle::new().fg_color(Some(palette::FOAM));
    let error = ClapStyle::new().fg_color(Some(palette::LOVE)).bold();
    let valid = ClapStyle::new().fg_color(Some(palette::PINE));
    let invalid = ClapStyle::new().fg_color(Some(palette::LOVE));
    let context = ClapStyle::new().fg_color(Some(palette::MUTED));
    let context_val = ClapStyle::new().fg_color(Some(palette::SUBTLE));

    Styles::styled()
        .header(header)
        .usage(usage)
        .literal(literal)
        .placeholder(placeholder)
        .error(error)
        .valid(valid)
        .invalid(invalid)
        .context(context)
        .context_value(context_val)
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub border: Color,
    pub header_fg: Color,
    pub selected_bg: Color,
    pub error_fg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::rose_pine_moon()
    }
}

impl Theme {
    #[must_use]
    pub fn rose_pine_moon() -> Self {
        Self {
            bg: palette::BASE,
            fg: palette::TEXT,
            border: palette::OVERLAY,
            header_fg: palette::IRIS,
            selected_bg: palette::SURFACE,
            error_fg: palette::LOVE,
        }
    }

}
