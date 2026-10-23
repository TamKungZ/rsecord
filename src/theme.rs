//! Themes and shared widget styles.
//!
//! Every colour in the UI is derived from the active theme's palette, never
//! hard-coded in a view. Two helpers keep text readable on any palette:
//!
//! * [`on`] picks black or white text for a solid fill, and
//! * [`legible`] nudges a foreground colour until it is readable on a background.
//!
//! Both aim for WCAG AA (4.5:1), and a unit test checks every theme.

use crate::settings::AppTheme;
use iced::theme::{Mode, Palette};
use iced::widget::{button, container, text, text_input};
use iced::{border, color, font, Background, Border, Color, Font, Theme};

/// Semibold where the platform font has it, regular otherwise.
pub const BOLD: Font = Font {
    weight: font::Weight::Semibold,
    ..Font::DEFAULT
};

const RADIUS: f32 = 8.0;
const MIN_CONTRAST: f32 = 4.5;
const INK: Color = Color::from_rgb(0.05, 0.055, 0.065);

// Fills are chosen so white text reads on them; foreground uses are lifted
// automatically by `legible`.
const BRAND_DARK: Palette = Palette {
    background: color!(0x1e1e21),
    text: color!(0xf1f2f3),
    primary: color!(0x2563eb),
    success: color!(0x15803d),
    warning: color!(0xb45309),
    danger: color!(0xdc2626),
};

const BRAND_LIGHT: Palette = Palette {
    background: color!(0xf6f7f9),
    text: color!(0x16181d),
    primary: color!(0x2563eb),
    success: color!(0x15803d),
    warning: color!(0xb45309),
    danger: color!(0xdc2626),
};

const MIDNIGHT: Palette = Palette {
    background: color!(0x000000),
    text: color!(0xe8e8ea),
    primary: color!(0x2f66d8),
    success: color!(0x15803d),
    warning: color!(0xb45309),
    danger: color!(0xdc2626),
};

impl AppTheme {
    /// The concrete theme. `system` is only used by [`AppTheme::System`].
    pub fn resolve(self, system: Mode) -> Theme {
        match self {
            AppTheme::System => match system {
                Mode::Light => AppTheme::BrandLight,
                // `None` means the platform could not say; the app has always
                // been dark, so stay dark.
                Mode::Dark | Mode::None => AppTheme::BrandDark,
            }
            .resolve(system),
            AppTheme::BrandDark => Theme::custom("rsecord Dark", BRAND_DARK),
            AppTheme::BrandLight => Theme::custom("rsecord Light", BRAND_LIGHT),
            AppTheme::Midnight => Theme::custom("Midnight", MIDNIGHT),
            AppTheme::Nord => Theme::Nord,
            AppTheme::Dracula => Theme::Dracula,
            AppTheme::Mocha => Theme::CatppuccinMocha,
            AppTheme::Latte => Theme::CatppuccinLatte,
            AppTheme::TokyoNight => Theme::TokyoNight,
            AppTheme::Gruvbox => Theme::GruvboxDark,
            AppTheme::SolarizedLight => Theme::SolarizedLight,
        }
    }
}

// ---------------------------------------------------------------------------
// Colour maths
// ---------------------------------------------------------------------------

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: 1.0,
    }
}

/// Readable text colour for a solid `fill`: white when it reaches AA,
/// otherwise whichever of near-black and white contrasts more.
pub fn on(fill: Color) -> Color {
    let white = fill.relative_contrast(Color::WHITE);
    if white >= MIN_CONTRAST {
        return Color::WHITE;
    }
    if white >= fill.relative_contrast(INK) {
        Color::WHITE
    } else {
        INK
    }
}

/// `fg` moved toward black or white, as little as needed, until it reaches AA
/// on `bg`.
pub fn legible(fg: Color, bg: Color) -> Color {
    if fg.relative_contrast(bg) >= MIN_CONTRAST {
        return fg;
    }
    let target = if Color::WHITE.relative_contrast(bg) >= Color::BLACK.relative_contrast(bg) {
        Color::WHITE
    } else {
        Color::BLACK
    };
    let mut t = 0.0;
    while t < 1.0 {
        t += 0.05;
        let c = mix(fg, target, t);
        if c.relative_contrast(bg) >= MIN_CONTRAST {
            return c;
        }
    }
    target
}

fn page(theme: &Theme) -> Color {
    theme.extended_palette().background.base.color
}

fn card_bg(theme: &Theme) -> Color {
    theme.extended_palette().background.weaker.color
}

/// Readable on both the page and the cards that sit on it.
fn legible_here(fg: Color, theme: &Theme) -> Color {
    legible(legible(fg, page(theme)), card_bg(theme))
}

/// Secondary text: clearly quieter than body text, still AA on the page.
pub fn muted_color(theme: &Theme) -> Color {
    let ep = theme.extended_palette();
    legible_here(
        mix(ep.background.base.text, ep.background.base.color, 0.38),
        theme,
    )
}

fn disabled_fg(theme: &Theme) -> Color {
    let ep = theme.extended_palette();
    mix(ep.background.base.text, ep.background.weak.color, 0.62)
}

// ---------------------------------------------------------------------------
// Tones
// ---------------------------------------------------------------------------

/// Semantic colour roles used across the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Primary,
    Success,
    Warning,
    Danger,
}

impl Tone {
    /// Solid fill for buttons and chips. Pair it with [`on`].
    pub fn fill(self, theme: &Theme) -> Color {
        let ep = theme.extended_palette();
        match self {
            Tone::Neutral => ep.background.strong.color,
            Tone::Primary => ep.primary.base.color,
            Tone::Success => ep.success.base.color,
            Tone::Warning => ep.warning.base.color,
            Tone::Danger => ep.danger.base.color,
        }
    }

    /// Hover/press shade of a solid fill. It moves away from the text colour
    /// (darker under white text, lighter under dark text), so the text stays
    /// readable instead of relying on iced's generated "strong" shade, which
    /// overshoots on saturated colours.
    fn hover(self, theme: &Theme) -> Color {
        let fill = self.fill(theme);
        let toward = if on(fill) == Color::WHITE {
            Color::BLACK
        } else {
            Color::WHITE
        };
        mix(fill, toward, 0.14)
    }

    /// Colour for text, borders and icons drawn on the page or on a card.
    pub fn fg(self, theme: &Theme) -> Color {
        match self {
            Tone::Neutral => theme.extended_palette().background.base.text,
            _ => legible_here(self.fill(theme), theme),
        }
    }
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

pub fn muted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(muted_color(theme)),
    }
}

pub fn tone_text(tone: Tone) -> impl Fn(&Theme) -> text::Style {
    move |theme| text::Style {
        color: Some(tone.fg(theme)),
    }
}

// ---------------------------------------------------------------------------
// Containers
// ---------------------------------------------------------------------------

/// A raised group of related controls.
pub fn card(theme: &Theme) -> container::Style {
    let ep = theme.extended_palette();
    container::Style {
        background: Some(ep.background.weaker.color.into()),
        text_color: Some(ep.background.base.text),
        border: Border {
            color: ep.background.weak.color,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..container::Style::default()
    }
}

/// A tinted message strip. `Neutral` is plain information.
pub fn banner(tone: Tone) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let ep = theme.extended_palette();
        if tone == Tone::Neutral {
            return container::Style {
                background: Some(ep.background.weak.color.into()),
                text_color: Some(ep.background.base.text),
                border: border::rounded(RADIUS),
                ..container::Style::default()
            };
        }
        let c = tone.fg(theme);
        container::Style {
            background: Some(mix(ep.background.base.color, c, 0.14).into()),
            text_color: Some(c),
            border: Border {
                color: mix(ep.background.base.color, c, 0.55),
                width: 1.0,
                radius: RADIUS.into(),
            },
            ..container::Style::default()
        }
    }
}

/// A small rounded label such as the REC badge.
pub fn pill(tone: Tone) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let ep = theme.extended_palette();
        let c = tone.fg(theme);
        container::Style {
            background: Some(mix(ep.background.base.color, c, 0.12).into()),
            text_color: Some(c),
            border: Border {
                color: mix(ep.background.base.color, c, 0.4),
                width: 1.0,
                radius: 999.0.into(),
            },
            ..container::Style::default()
        }
    }
}

/// A filled circle (status dot, colour swatch).
pub fn dot(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(color.into()),
        border: Border {
            color: Color::BLACK.scale_alpha(0.25),
            width: 1.0,
            radius: 999.0.into(),
        },
        ..container::Style::default()
    }
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

fn base_button(bg: Option<Color>, fg: Color) -> button::Style {
    button::Style {
        background: bg.map(Background::Color),
        text_color: fg,
        border: border::rounded(RADIUS),
        ..button::Style::default()
    }
}

/// A solid, high-emphasis button.
pub fn solid(tone: Tone) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let bg = match status {
            button::Status::Active => tone.fill(theme),
            button::Status::Hovered | button::Status::Pressed => tone.hover(theme),
            button::Status::Disabled => theme.extended_palette().background.weak.color,
        };
        let fg = if status == button::Status::Disabled {
            disabled_fg(theme)
        } else {
            on(bg)
        };
        base_button(Some(bg), fg)
    }
}

/// A quiet outlined button for secondary actions.
pub fn ghost(theme: &Theme, status: button::Status) -> button::Style {
    let ep = theme.extended_palette();
    let (bg, fg) = match status {
        button::Status::Active => (None, ep.background.base.text),
        button::Status::Hovered => (Some(ep.background.weak.color), ep.background.base.text),
        button::Status::Pressed => (Some(ep.background.strong.color), ep.background.base.text),
        button::Status::Disabled => (None, disabled_fg(theme)),
    };
    let mut s = base_button(bg, fg);
    s.border = Border {
        color: ep.background.strong.color,
        width: 1.0,
        radius: RADIUS.into(),
    };
    s
}

/// An option in a segmented control or a quick-pick row.
pub fn chip(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        if active {
            return solid(Tone::Primary)(theme, status);
        }
        let ep = theme.extended_palette();
        let bg = match status {
            button::Status::Active => ep.background.weak.color,
            _ => ep.background.strong.color,
        };
        let fg = if status == button::Status::Disabled {
            disabled_fg(theme)
        } else {
            ep.background.base.text
        };
        base_button(Some(bg), fg)
    }
}

/// A tab: flat until hovered, filled when selected.
pub fn tab(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        if active {
            return solid(Tone::Primary)(theme, status);
        }
        let ep = theme.extended_palette();
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => Some(ep.background.weak.color),
            _ => None,
        };
        base_button(bg, muted_color(theme))
    }
}

/// A theme preview card. It is painted in the previewed theme's own colours.
pub fn theme_card(
    preview: Palette,
    selected: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let ep = theme.extended_palette();
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let ring = if selected {
            (ep.primary.strong.color, 2.5)
        } else if hovered {
            (ep.background.stronger.color, 1.5)
        } else {
            (ep.background.strong.color, 1.0)
        };
        let mut s = base_button(Some(preview.background), preview.text);
        s.border = Border {
            color: ring.0,
            width: ring.1,
            radius: 10.0.into(),
        };
        s
    }
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

pub fn input(theme: &Theme, status: text_input::Status, invalid: bool) -> text_input::Style {
    let mut s = text_input::default(theme, status);
    s.border.radius = 6.0.into();
    if invalid {
        s.border.color = Tone::Danger.fg(theme);
        s.border.width = 1.5;
    }
    s.placeholder = muted_color(theme);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODES: [Mode; 3] = [Mode::None, Mode::Light, Mode::Dark];
    const TONES: [Tone; 5] = [
        Tone::Neutral,
        Tone::Primary,
        Tone::Success,
        Tone::Warning,
        Tone::Danger,
    ];

    fn every_theme() -> Vec<(String, Theme)> {
        let mut out = Vec::new();
        for t in AppTheme::ALL {
            for m in MODES {
                out.push((format!("{t} / {m:?}"), t.resolve(m)));
            }
        }
        out
    }

    #[test]
    fn body_text_is_readable() {
        for (name, th) in every_theme() {
            let ep = th.extended_palette();
            let c = ep.background.base.text.relative_contrast(ep.background.base.color);
            assert!(c >= MIN_CONTRAST, "{name}: body text contrast {c:.2}");
            let c = ep.background.weaker.text.relative_contrast(ep.background.weaker.color);
            assert!(c >= MIN_CONTRAST, "{name}: card text contrast {c:.2}");
        }
    }

    #[test]
    fn muted_text_is_readable() {
        for (name, th) in every_theme() {
            let c = muted_color(&th).relative_contrast(page(&th));
            assert!(c >= MIN_CONTRAST, "{name}: muted text contrast {c:.2}");
            let c = muted_color(&th).relative_contrast(card_bg(&th));
            assert!(c >= MIN_CONTRAST, "{name}: muted text on card contrast {c:.2}");
        }
    }

    #[test]
    fn solid_buttons_are_readable() {
        for (name, th) in every_theme() {
            for tone in TONES {
                for fill in [tone.fill(&th), tone.hover(&th)] {
                    let c = fill.relative_contrast(on(fill));
                    assert!(c >= MIN_CONTRAST, "{name} {tone:?}: button contrast {c:.2}");
                }
            }
        }
    }

    #[test]
    fn tone_foregrounds_are_readable() {
        for (name, th) in every_theme() {
            for tone in TONES {
                let c = tone.fg(&th).relative_contrast(page(&th));
                assert!(c >= MIN_CONTRAST, "{name} {tone:?}: text contrast {c:.2}");
                let c = tone.fg(&th).relative_contrast(card_bg(&th));
                assert!(c >= MIN_CONTRAST, "{name} {tone:?}: text on card contrast {c:.2}");
            }
        }
    }

    #[test]
    fn system_follows_the_os_mode() {
        assert!(!AppTheme::System.resolve(Mode::Light).extended_palette().is_dark);
        assert!(AppTheme::System.resolve(Mode::Dark).extended_palette().is_dark);
        assert!(AppTheme::System.resolve(Mode::None).extended_palette().is_dark);
    }

    #[test]
    fn theme_labels_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for t in AppTheme::ALL {
            assert!(seen.insert(t.to_string()), "duplicate label {t}");
        }
    }
}
