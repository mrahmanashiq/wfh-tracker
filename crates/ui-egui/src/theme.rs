//! The look: a calm, light canvas in the spirit of Things 3, with one blue
//! accent, generous spacing and quiet controls, and a matching dark theme.
//! Every colour, size, gap, radius and font size lives here as a token;
//! screens use these instead of literal numbers.

use std::sync::Arc;

use egui::epaint::text::{FontData, FontDefinitions, FontTweak};
use egui::style::{Selection, WidgetVisuals, Widgets};
use egui::{
    Color32, Context, CornerRadius, FontFamily, FontId, Shadow, Stroke, TextStyle, Theme,
    ThemePreference, Visuals,
};

/// Spacing scale in points: gaps, margins and indents.
pub mod space {
    /// Between the rows of a dense list.
    pub const XXS: f32 = 2.0;
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
}

/// Corner radii.
pub mod radius {
    /// Bars and small marks.
    pub const XS: u8 = 3;
    /// Controls, hover fills, icon buttons.
    pub const SM: u8 = 6;
    /// List rows and the sidebar selection.
    pub const MD: u8 = 8;
    /// Cards and panels.
    pub const LG: u8 = 12;
    /// Pills: half the control height.
    pub const PILL: u8 = 15;
    /// The square to-do checkbox, as in Things.
    pub const CHECK: u8 = 4;
    /// Tag capsules: half the tag height.
    pub const TAG: u8 = 9;
    /// The stop square inside the timer capsule.
    pub const STOP: u8 = 2;
    /// Header buttons: half their height.
    pub const HEADER_BUTTON: u8 = 14;
}

/// Line widths.
pub mod stroke {
    /// Separators and card borders.
    pub const HAIRLINE: f32 = 1.0;
    /// Checkbox borders and the small checklist ring.
    pub const CHECK: f32 = 1.2;
    /// The tick inside a checked box.
    pub const TICK: f32 = 1.8;
}

/// Fixed sizes of shared elements.
pub mod size {
    pub const SIDEBAR_WIDTH: f32 = 220.0;
    pub const SIDEBAR_ROW: f32 = 32.0;
    /// A list row: tasks, targets, rules, categories.
    pub const ROW: f32 = 36.0;
    /// Buttons, fields and pills.
    pub const CONTROL: f32 = 30.0;
    pub const ICON_BUTTON: f32 = 28.0;
    pub const ICON: f32 = 16.0;
    /// The hit area of a checkbox; the column every row's check sits in.
    pub const CHECK: f32 = 20.0;
    /// The painted square of a to-do checkbox.
    pub const CHECK_BOX: f32 = 15.0;
    /// The painted ring of a checklist item.
    pub const CHECK_SMALL: f32 = 11.0;
    /// A Things-style task row.
    pub const TASK_ROW: f32 = 32.0;
    /// A tag capsule.
    pub const TAG: f32 = 18.0;
    /// The category dot inside a tag.
    pub const TAG_DOT: f32 = 6.0;
    /// The running timer's capsule.
    pub const CAPSULE: f32 = 22.0;
    /// The time column at the right of a task row.
    pub const TIME_COLUMN: f32 = 64.0;
    /// The icon beside a screen title.
    pub const TITLE_ICON: f32 = 30.0;
    /// The soft circle of an empty state.
    pub const EMPTY_ICON: f32 = 44.0;
    /// Drop-downs such as the category picker.
    pub const PICKER: f32 = 130.0;
    /// The tallest a drop-down's menu grows before it scrolls.
    pub const MENU: f32 = 400.0;
    /// Single-line text fields.
    pub const FIELD: f32 = 180.0;
    /// Thin progress bars.
    pub const BAR: f32 = 6.0;
    /// The name column of an aligned row.
    pub const LABEL_COLUMN: f32 = 180.0;
    /// The value column of an aligned row.
    pub const VALUE_COLUMN: f32 = 110.0;
    /// The bar column of an aligned row.
    pub const BAR_COLUMN: f32 = 200.0;
    /// The header strip's content height.
    pub const HEADER: f32 = 40.0;
    /// Below this content width the header turns compact: icon-only
    /// buttons and a shorter office line.
    pub const HEADER_COMPACT: f32 = 700.0;
    /// Break, Meeting and Resume in the header.
    pub const HEADER_BUTTON: f32 = 28.0;
    /// The running category's dot in the header.
    pub const HEADER_DOT: f32 = 9.0;
    /// The round Stop button beside the running time.
    pub const STOP_BUTTON: f32 = 26.0;
    /// The square inside it.
    pub const STOP_GLYPH: f32 = 8.0;
    /// The office meter's bar: its height and least width.
    pub const METER_BAR: f32 = 4.0;
    pub const METER_MIN: f32 = 96.0;
    /// The task name in a Resume button is cut short past this.
    pub const RESUME_LABEL: f32 = 150.0;
}

/// Type scale, in points.
pub mod text {
    pub const TITLE: f32 = 24.0;
    /// Big numbers on dashboard cards.
    pub const VALUE: f32 = 21.0;
    pub const TASK: f32 = 15.0;
    /// Checklist items (third-level tasks).
    pub const CHECKLIST: f32 = 13.5;
    /// Tag capsules.
    pub const TAG: f32 = 11.5;
    pub const BODY: f32 = 14.0;
    pub const SMALL: f32 = 12.0;
    /// Section labels and chart captions.
    pub const CAPTION: f32 = 11.0;
    pub const MONO: f32 = 13.0;
    /// The running timer's elapsed time in the header.
    pub const HERO: f32 = 20.0;
}

/// How long hover and selection fades take, in seconds.
pub const FADE: f32 = 0.12;

/// Font files to use instead of egui's built-in font (e.g. Segoe UI on Windows).
pub struct UiFonts {
    pub regular: Vec<u8>,
    pub semibold: Vec<u8>,
    /// Fallback for symbols such as ✅ ⏳ (e.g. Segoe UI Emoji).
    pub emoji: Option<Vec<u8>>,
}

/// Named colours for one theme.
#[derive(Clone, Copy)]
pub struct Palette {
    /// Main content background.
    pub canvas: Color32,
    /// Sidebar background; also quiet fills such as the timeline strip.
    pub sidebar: Color32,
    /// Card surface.
    pub card: Color32,
    /// A raised card's surface, such as the inline task editor.
    pub raised: Color32,
    pub text: Color32,
    /// Dates, counts, hints.
    pub secondary: Color32,
    /// Empty checkbox rings, bar tracks, the target tick.
    pub line: Color32,
    /// Separators and card borders: quieter than `line`.
    pub hairline: Color32,
    /// Hover fill.
    pub hover: Color32,
    /// The selected sidebar row; pressed controls.
    pub selected: Color32,
    /// Resting fill of buttons, drop-downs and fields.
    pub control: Color32,
    pub accent: Color32,
    /// Running-timer row and pill background.
    pub accent_soft: Color32,
    /// The selected task row: Things' soft blue.
    pub selection: Color32,
    /// Tag capsule fill.
    pub tag: Color32,
    /// Checklist text: between `text` and `secondary`.
    pub muted: Color32,
    /// Open checkbox borders: a little firmer than `line`.
    pub check: Color32,
    pub danger: Color32,
    /// Behind an active "End meeting".
    pub danger_soft: Color32,
    /// Nearing a limit, or office time on a day off.
    pub warning: Color32,
    /// A met target.
    pub success: Color32,
    pub star: Color32,
    /// Under cards.
    pub shadow: Color32,
    /// Under raised cards such as the inline task editor.
    pub raised_shadow: Color32,
}

const LIGHT: Palette = Palette {
    canvas: Color32::from_rgb(0xff, 0xff, 0xff),
    sidebar: Color32::from_rgb(0xf6, 0xf6, 0xf8),
    card: Color32::from_rgb(0xfc, 0xfc, 0xfd),
    raised: Color32::from_rgb(0xff, 0xff, 0xff),
    text: Color32::from_rgb(0x1f, 0x20, 0x24),
    secondary: Color32::from_rgb(0x86, 0x87, 0x8e),
    line: Color32::from_rgb(0xd6, 0xd7, 0xdc),
    hairline: Color32::from_rgb(0xec, 0xec, 0xf0),
    hover: Color32::from_rgb(0xef, 0xef, 0xf3),
    selected: Color32::from_rgb(0xe5, 0xe6, 0xeb),
    control: Color32::from_rgb(0xf1, 0xf1, 0xf4),
    accent: Color32::from_rgb(0x2f, 0x6f, 0xe4),
    accent_soft: Color32::from_rgb(0xea, 0xf1, 0xfd),
    selection: Color32::from_rgb(0xd9, 0xe6, 0xfb),
    tag: Color32::from_rgb(0xec, 0xed, 0xf1),
    muted: Color32::from_rgb(0x55, 0x57, 0x5e),
    check: Color32::from_rgb(0xc2, 0xc3, 0xc9),
    danger: Color32::from_rgb(0xd9, 0x4c, 0x4c),
    danger_soft: Color32::from_rgb(0xfc, 0xec, 0xec),
    warning: Color32::from_rgb(0xd9, 0x8a, 0x1c),
    success: Color32::from_rgb(0x2e, 0x9a, 0x5e),
    star: Color32::from_rgb(0xf0, 0xb4, 0x00),
    shadow: Color32::from_black_alpha(14),
    raised_shadow: Color32::from_black_alpha(34),
};

const DARK: Palette = Palette {
    canvas: Color32::from_rgb(0x1c, 0x1d, 0x20),
    sidebar: Color32::from_rgb(0x23, 0x24, 0x28),
    card: Color32::from_rgb(0x24, 0x25, 0x29),
    raised: Color32::from_rgb(0x2a, 0x2b, 0x30),
    text: Color32::from_rgb(0xe8, 0xe8, 0xeb),
    secondary: Color32::from_rgb(0x8f, 0x90, 0x97),
    line: Color32::from_rgb(0x42, 0x43, 0x49),
    hairline: Color32::from_rgb(0x2f, 0x30, 0x35),
    hover: Color32::from_rgb(0x2c, 0x2d, 0x32),
    selected: Color32::from_rgb(0x34, 0x35, 0x3b),
    control: Color32::from_rgb(0x2b, 0x2c, 0x31),
    accent: Color32::from_rgb(0x5b, 0x9b, 0xff),
    accent_soft: Color32::from_rgb(0x1f, 0x2e, 0x47),
    selection: Color32::from_rgb(0x26, 0x3a, 0x5c),
    tag: Color32::from_rgb(0x32, 0x33, 0x39),
    muted: Color32::from_rgb(0xb4, 0xb5, 0xbb),
    check: Color32::from_rgb(0x5c, 0x5d, 0x64),
    danger: Color32::from_rgb(0xf0, 0x6a, 0x6a),
    danger_soft: Color32::from_rgb(0x3b, 0x25, 0x27),
    warning: Color32::from_rgb(0xf0, 0xa8, 0x4a),
    success: Color32::from_rgb(0x4c, 0xc3, 0x8a),
    star: Color32::from_rgb(0xf5, 0xc5, 0x42),
    shadow: Color32::from_black_alpha(70),
    raised_shadow: Color32::from_black_alpha(120),
};

pub fn palette(ui: &egui::Ui) -> Palette {
    if ui.visuals().dark_mode { DARK } else { LIGHT }
}

/// Font family for titles and emphasis.
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// A regular font of `size` points.
pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

/// A semibold font of `size` points, or the regular one when the semibold
/// face isn't installed (as in tests that skip [`install`]).
pub fn bold(ui: &egui::Ui, size: f32) -> FontId {
    let family = semibold();
    let installed = ui
        .ctx()
        .fonts(|f| f.definitions().families.contains_key(&family));
    FontId::new(
        size,
        if installed {
            family
        } else {
            FontFamily::Proportional
        },
    )
}

/// The soft drop shadow under cards.
pub fn card_shadow(p: &Palette) -> Shadow {
    Shadow {
        offset: [0, 1],
        blur: 6,
        spread: 0,
        color: p.shadow,
    }
}

/// The deeper shadow under a raised card, such as a task opened for editing.
pub fn raised_shadow(p: &Palette) -> Shadow {
    Shadow {
        offset: [0, 3],
        blur: 14,
        spread: 0,
        color: p.raised_shadow,
    }
}

/// Installs fonts and both themes, starting light until the stored
/// appearance is applied.
pub fn install(ctx: &Context, fonts: Option<UiFonts>) {
    ctx.set_fonts(font_definitions(fonts));
    for (theme, p) in [(Theme::Light, LIGHT), (Theme::Dark, DARK)] {
        ctx.style_mut_of(theme, |style| {
            style.visuals = visuals(theme, p);
            style.text_styles = [
                (TextStyle::Heading, FontId::new(text::TITLE, semibold())),
                (TextStyle::Body, font(text::BODY)),
                (TextStyle::Button, font(text::BODY)),
                (TextStyle::Small, font(text::SMALL)),
                (TextStyle::Monospace, FontId::monospace(text::MONO)),
            ]
            .into();
            style.spacing.item_spacing = egui::vec2(space::SM, 6.0);
            style.spacing.button_padding = egui::vec2(space::MD, 5.0);
            style.spacing.interact_size.y = size::CONTROL;
            style.animation_time = FADE;
        });
    }
    apply(ctx, wt_core::Appearance::Light);
}

/// Switches to the chosen appearance.
pub fn apply(ctx: &Context, appearance: wt_core::Appearance) {
    ctx.set_theme(match appearance {
        wt_core::Appearance::Light => ThemePreference::Light,
        wt_core::Appearance::Dark => ThemePreference::Dark,
        wt_core::Appearance::System => ThemePreference::System,
    });
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
            .insert("ui".into(), Arc::new(centred_font(fonts.regular)));
        defs.font_data
            .insert("ui-semibold".into(), Arc::new(centred_font(fonts.semibold)));
        if let Some(proportional) = defs.families.get_mut(&FontFamily::Proportional) {
            proportional.insert(0, "ui".into());
        }
        semibold_chain.insert(0, "ui-semibold".into());
        if let Some(emoji) = fonts.emoji {
            defs.font_data
                .insert("ui-emoji".into(), Arc::new(FontData::from_owned(emoji)));
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                if let Some(chain) = defs.families.get_mut(&family) {
                    chain.push("ui-emoji".into());
                }
            }
            semibold_chain.push("ui-emoji".into());
        }
    }
    defs.families.insert(semibold(), semibold_chain);
    defs
}

/// `bytes` as a font whose capitals sit in the middle of its row, so text
/// centred by its row (labels, buttons, drop-downs, painted capsules) looks
/// centred. egui puts the baseline `ascent` below the row's top; with Segoe
/// UI's tall ascent that leaves capitals about 0.06 em low, which shows as
/// text sitting low in pills and capsules. The shift is visual only: layout,
/// row heights and hit areas don't change.
fn centred_font(bytes: Vec<u8>) -> FontData {
    let y_offset_factor = cap_centre_offset(&bytes).map_or(0.0, |offset| -offset);
    FontData::from_owned(bytes).tweak(FontTweak {
        y_offset_factor,
        ..FontTweak::default()
    })
}

/// How far below its row's middle a font's capitals are centred, in ems,
/// read from its `head`, `hhea` and `OS/2` tables the way egui lays text
/// out (baseline at the ascent; row = ascent + descent + line gap). `None`
/// when the tables can't be read.
fn cap_centre_offset(font: &[u8]) -> Option<f32> {
    let u16_at = |at: usize| Some(u16::from_be_bytes([*font.get(at)?, *font.get(at + 1)?]));
    let i16_at = |at: usize| u16_at(at).map(|v| v as i16 as f32);
    let table = |tag: &[u8; 4]| -> Option<usize> {
        let count = usize::from(u16_at(4)?);
        (0..count).find_map(|i| {
            let record = 12 + 16 * i;
            (font.get(record..record + 4)? == tag).then(|| {
                let at = font.get(record + 8..record + 12)?;
                usize::try_from(u32::from_be_bytes([at[0], at[1], at[2], at[3]])).ok()
            })?
        })
    };
    let units = f32::from(u16_at(table(b"head")? + 18)?);
    let os2 = table(b"OS/2")?;
    let hhea = table(b"hhea")?;
    // OS/2 fsSelection bit 7: USE_TYPO_METRICS.
    let typo = u16_at(os2 + 62)? & (1 << 7) != 0;
    let metrics = if typo { os2 + 68 } else { hhea + 4 };
    let (ascent, descent, gap) = (i16_at(metrics)?, i16_at(metrics + 2)?, i16_at(metrics + 4)?);
    // sCapHeight exists from OS/2 version 2.
    let cap = if u16_at(os2)? >= 2 {
        i16_at(os2 + 88)?
    } else {
        0.7 * units
    };
    let row = ascent - descent + gap;
    (units > 0.0 && row > 0.0).then(|| ((ascent - cap / 2.0) - row / 2.0) / units)
}

fn visuals(theme: Theme, p: Palette) -> Visuals {
    let mut v = match theme {
        Theme::Light => Visuals::light(),
        Theme::Dark => Visuals::dark(),
    };
    let control_radius = CornerRadius::same(radius::SM);
    let quiet = |fill: Color32, text: Color32| WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::NONE,
        corner_radius: control_radius,
        fg_stroke: Stroke::new(1.0, text),
        expansion: 0.0,
    };
    v.widgets = Widgets {
        // Separators and frames use this stroke: a hairline.
        noninteractive: WidgetVisuals {
            bg_stroke: Stroke::new(1.0, p.hairline),
            ..quiet(p.canvas, p.text)
        },
        inactive: quiet(p.control, p.text),
        hovered: quiet(p.selected, p.text),
        active: quiet(p.line, p.text),
        open: quiet(p.selected, p.text),
    };
    v.selection = Selection {
        bg_fill: p.accent_soft,
        stroke: Stroke::new(1.0, p.accent),
    };
    v.hyperlink_color = p.accent;
    v.panel_fill = p.canvas;
    v.window_fill = p.canvas;
    v.extreme_bg_color = p.control; // text fields
    v.text_edit_bg_color = Some(p.control);
    v.faint_bg_color = p.sidebar;
    v.window_stroke = Stroke::new(1.0, p.hairline);
    v.window_shadow = Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: p.shadow,
    };
    v.popup_shadow = v.window_shadow;
    v.error_fg_color = p.danger;
    v.window_corner_radius = CornerRadius::same(radius::LG);
    v.menu_corner_radius = CornerRadius::same(radius::MD);
    v
}

#[cfg(test)]
mod tests {
    use super::cap_centre_offset;

    /// A font file holding only the tables [`cap_centre_offset`] reads, with
    /// Segoe UI's numbers: 2048 units, ascent 2210, descent 514, cap 1434.
    fn font(use_typo: bool) -> Vec<u8> {
        let mut head = vec![0u8; 54];
        head[18..20].copy_from_slice(&2048u16.to_be_bytes());
        let mut hhea = vec![0u8; 36];
        hhea[4..6].copy_from_slice(&2210i16.to_be_bytes());
        hhea[6..8].copy_from_slice(&(-514i16).to_be_bytes());
        let mut os2 = vec![0u8; 96];
        os2[0..2].copy_from_slice(&4u16.to_be_bytes());
        if use_typo {
            os2[62..64].copy_from_slice(&(1u16 << 7).to_be_bytes());
        }
        os2[68..70].copy_from_slice(&1434i16.to_be_bytes());
        os2[70..72].copy_from_slice(&(-614i16).to_be_bytes());
        os2[88..90].copy_from_slice(&1434i16.to_be_bytes());
        let tables: [(&[u8; 4], Vec<u8>); 3] = [(b"head", head), (b"hhea", hhea), (b"OS/2", os2)];
        let mut file = vec![0u8; 12 + 16 * tables.len()];
        file[4..6].copy_from_slice(&3u16.to_be_bytes());
        for (i, (tag, data)) in tables.into_iter().enumerate() {
            let record = 12 + 16 * i;
            let at = u32::try_from(file.len()).unwrap_or_default();
            file[record..record + 4].copy_from_slice(tag);
            file[record + 8..record + 12].copy_from_slice(&at.to_be_bytes());
            file.extend(data);
        }
        file
    }

    #[test]
    fn segoe_capitals_sit_low_in_their_row_by_about_six_hundredths_of_an_em() {
        let offset = cap_centre_offset(&font(false)).unwrap_or_default();
        assert!((offset - 0.064).abs() < 0.001, "{offset}");
    }

    #[test]
    fn typo_metrics_are_used_when_the_font_asks() {
        // Typo ascent 1434, descent 614: capitals fill the ascent, so their
        // middle is 0.35 em down a 1.0 em row.
        let offset = cap_centre_offset(&font(true)).unwrap_or_default();
        assert!((offset - (-0.15)).abs() < 0.001, "{offset}");
    }

    #[test]
    fn unreadable_bytes_give_no_offset() {
        assert_eq!(cap_centre_offset(b"not a font"), None);
    }
}
