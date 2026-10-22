//! The look: a calm, light canvas in the spirit of Things 3, with one blue
//! accent, generous spacing and quiet controls. Every colour lives here.

use std::sync::Arc;

use egui::epaint::text::{FontData, FontDefinitions};
use egui::style::{Selection, WidgetVisuals, Widgets};
use egui::{
    Color32, Context, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Theme, ThemePreference,
    Visuals,
};

/// Font files to use instead of egui's built-in font (e.g. Segoe UI on Windows).
pub struct UiFonts {
    pub regular: Vec<u8>,
    pub semibold: Vec<u8>,
}

/// Named colours for one theme.
#[derive(Clone, Copy)]
pub struct Palette {
    /// Main content background.
    pub canvas: Color32,
    /// Sidebar background.
    pub sidebar: Color32,
    pub text: Color32,
    /// Dates, counts, hints.
    pub secondary: Color32,
    /// Hairlines and empty checkbox rings.
    pub line: Color32,
    /// Hover and selected-row fill.
    pub hover: Color32,
    pub accent: Color32,
    /// Running-timer row and pill background.
    pub accent_soft: Color32,
    pub danger: Color32,
    pub star: Color32,
}

const LIGHT: Palette = Palette {
    canvas: Color32::from_rgb(0xff, 0xff, 0xff),
    sidebar: Color32::from_rgb(0xf5, 0xf5, 0xf7),
    text: Color32::from_rgb(0x1d, 0x1d, 0x1f),
    secondary: Color32::from_rgb(0x8a, 0x8a, 0x8e),
    line: Color32::from_rgb(0xd8, 0xd8, 0xdc),
    hover: Color32::from_rgb(0xec, 0xec, 0xef),
    accent: Color32::from_rgb(0x1a, 0x73, 0xe8),
    accent_soft: Color32::from_rgb(0xe8, 0xf0, 0xfd),
    danger: Color32::from_rgb(0xe5, 0x48, 0x4d),
    star: Color32::from_rgb(0xf5, 0xb7, 0x00),
};

const DARK: Palette = Palette {
    canvas: Color32::from_rgb(0x1e, 0x1f, 0x22),
    sidebar: Color32::from_rgb(0x27, 0x28, 0x2c),
    text: Color32::from_rgb(0xec, 0xec, 0xee),
    secondary: Color32::from_rgb(0x8e, 0x8e, 0x93),
    line: Color32::from_rgb(0x48, 0x48, 0x4d),
    hover: Color32::from_rgb(0x33, 0x34, 0x39),
    accent: Color32::from_rgb(0x4c, 0x9a, 0xff),
    accent_soft: Color32::from_rgb(0x1f, 0x33, 0x52),
    danger: Color32::from_rgb(0xff, 0x6b, 0x6b),
    star: Color32::from_rgb(0xff, 0xc8, 0x2e),
};

pub fn palette(ui: &egui::Ui) -> Palette {
    if ui.visuals().dark_mode { DARK } else { LIGHT }
}

/// Font family for titles and emphasis.
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// Installs fonts and both themes, and starts in light mode.
pub fn install(ctx: &Context, fonts: Option<UiFonts>) {
    ctx.set_fonts(font_definitions(fonts));
    for (theme, p) in [(Theme::Light, LIGHT), (Theme::Dark, DARK)] {
        ctx.style_mut_of(theme, |style| {
            style.visuals = visuals(theme, p);
            style.text_styles = [
                (TextStyle::Heading, FontId::new(26.0, semibold())),
                (TextStyle::Body, FontId::proportional(14.0)),
                (TextStyle::Button, FontId::proportional(14.0)),
                (TextStyle::Small, FontId::proportional(12.0)),
                (TextStyle::Monospace, FontId::monospace(13.0)),
            ]
            .into();
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.button_padding = egui::vec2(10.0, 5.0);
            style.spacing.interact_size.y = 28.0;
        });
    }
    ctx.set_theme(ThemePreference::Light);
}

fn font_definitions(fonts: Option<UiFonts>) -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    let fallback = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut semibold_chain = fallback.clone();
    if let Some(fonts) = fonts {
        defs.font_data
            .insert("ui".into(), Arc::new(FontData::from_owned(fonts.regular)));
        defs.font_data.insert(
            "ui-semibold".into(),
            Arc::new(FontData::from_owned(fonts.semibold)),
        );
        if let Some(proportional) = defs.families.get_mut(&FontFamily::Proportional) {
            proportional.insert(0, "ui".into());
        }
        semibold_chain.insert(0, "ui-semibold".into());
    }
    defs.families.insert(semibold(), semibold_chain);
    defs
}

fn visuals(theme: Theme, p: Palette) -> Visuals {
    let mut v = match theme {
        Theme::Light => Visuals::light(),
        Theme::Dark => Visuals::dark(),
    };
    let radius = CornerRadius::same(6);
    let quiet = |fill: Color32, text: Color32| WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::NONE,
        corner_radius: radius,
        fg_stroke: Stroke::new(1.0, text),
        expansion: 0.0,
    };
    v.widgets = Widgets {
        noninteractive: WidgetVisuals {
            bg_stroke: Stroke::new(1.0, p.line),
            ..quiet(p.canvas, p.text)
        },
        inactive: quiet(p.hover, p.text),
        hovered: quiet(p.line, p.text),
        active: quiet(p.line, p.text),
        open: quiet(p.hover, p.text),
    };
    v.selection = Selection {
        bg_fill: p.accent_soft,
        stroke: Stroke::new(1.0, p.accent),
    };
    v.hyperlink_color = p.accent;
    v.panel_fill = p.canvas;
    v.window_fill = p.canvas;
    v.extreme_bg_color = p.sidebar; // text fields
    v.faint_bg_color = p.sidebar;
    v.window_stroke = Stroke::new(1.0, p.line);
    v.error_fg_color = p.danger;
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v
}
