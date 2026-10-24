//! Small painted widgets shared by the screens. Everything that has no visible
//! text gets an accessible name, so screen readers (and the UI tests) find it.
//! Sizes, gaps and colours come from [`crate::theme`].

use std::f32::consts::TAU;
use std::sync::Arc;

use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align, Align2, Button, Color32, CornerRadius, FontId, Frame, Galley, Id, InnerResponse, Label,
    Layout, Margin, Painter, Pos2, Rect, Response, RichText, Role, Sense, Shape, Stroke,
    StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2,
};
use jiff::SignedDuration;
use jiff::civil::Weekday;
use wt_core::{Rgb, hours_text};

use crate::theme::{
    FADE, Palette, bold, card_shadow, font, palette, radius, raised_shadow, size, space, stroke,
    text,
};

const DAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// `Mon`
pub fn day_name(day: Weekday) -> &'static str {
    DAY_NAMES[day.to_sunday_zero_offset() as usize]
}

/// `3h 20m / 5h`: time against a target of whole hours.
pub fn target_progress_text(done: SignedDuration, target_minutes: u32) -> String {
    format!("{} / {}h", hours_text(done), target_minutes / 60)
}

pub fn color32(c: Rgb) -> Color32 {
    Color32::from_rgb(c.0, c.1, c.2)
}

/// 0 to 1: how far a hover or selection fade for `id` has got towards `on`.
pub fn fade(ui: &Ui, id: Id, on: bool) -> f32 {
    ui.ctx().animate_bool_with_time(id, on, FADE)
}

/// Paints `color` at `t` (0..=1) of its strength; nothing at 0.
fn faded_fill(painter: &Painter, rect: Rect, rounding: u8, color: Color32, t: f32) {
    if t > 0.0 {
        painter.rect_filled(rect, CornerRadius::same(rounding), color.gamma_multiply(t));
    }
}

/// A small filled circle in a category's colour.
pub fn color_dot(ui: &mut Ui, color: Rgb) {
    let (rect, _) = ui.allocate_exact_size(vec2(size::ICON, size::ICON), Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), 4.5, color32(color));
}

/// A thin bar `width` wide, filled to `share` (clamped to 0..=1).
pub fn progress_bar(ui: &mut Ui, width: f32, share: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, size::BAR), Sense::hover());
    paint_bar(ui.painter(), rect, share, color, palette(ui).hover);
}

/// A rounded track with its first `share` filled.
pub fn paint_bar(painter: &Painter, rect: Rect, share: f32, color: Color32, track: Color32) {
    let rounding = CornerRadius::same((rect.height() / 2.0) as u8);
    painter.rect_filled(rect, rounding, track);
    let share = share.clamp(0.0, 1.0);
    if share > 0.0 {
        let mut done = rect;
        // At least round-cap wide, so a small share still reads as a dot.
        done.set_width((rect.width() * share).max(rect.height()));
        painter.rect_filled(done, rounding, color);
    }
}

/// Shows the result of the last action in the theme's danger colour.
pub fn message(ui: &mut Ui, message: &Option<String>) {
    if let Some(text) = message {
        ui.label(RichText::new(text).color(palette(ui).danger));
    }
}

/// The two to-do checkbox shapes in a Things list.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CheckShape {
    /// A to-do: a rounded square.
    Square,
    /// A checklist item: a smaller ring.
    Small,
}

/// A Things-style checkbox in a [`size::CHECK`] column. `label` names it for
/// accessibility; the visible title is painted separately by the row.
pub fn task_check(ui: &mut Ui, done: bool, label: &str, shape: CheckShape) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size::CHECK, size::CHECK), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::CheckBox, true, done, label));
    let t = fade(ui, response.id, response.hovered());
    paint_check(ui, rect.center(), shape, done, t);
    response
}

/// An inert, faint checkbox: the "New To-Do" placeholder row's.
pub fn placeholder_check(ui: &mut Ui, shape: CheckShape) {
    let (rect, _) = ui.allocate_exact_size(vec2(size::CHECK, size::CHECK), Sense::hover());
    paint_check(ui, rect.center(), shape, false, 0.0);
}

/// `hover` (0..=1) tints an open box towards the accent.
fn paint_check(ui: &Ui, center: Pos2, shape: CheckShape, done: bool, hover: f32) {
    let p = palette(ui);
    let painter = ui.painter();
    let border = Stroke::new(stroke::CHECK, p.check.lerp_to_gamma(p.accent, hover));
    match shape {
        CheckShape::Square => {
            let side = size::CHECK_BOX;
            let rect = Rect::from_center_size(center, vec2(side, side));
            let rounding = CornerRadius::same(radius::CHECK);
            if done {
                painter.rect_filled(rect, rounding, p.accent);
                check_mark(painter, center, 1.0, Color32::WHITE);
            } else {
                painter.rect_filled(rect, rounding, p.accent_soft.gamma_multiply(hover));
                painter.rect_stroke(rect, rounding, border, StrokeKind::Inside);
            }
        }
        CheckShape::Small => {
            let r = size::CHECK_SMALL / 2.0;
            if done {
                painter.circle_filled(center, r, p.accent);
                check_mark(
                    painter,
                    center,
                    size::CHECK_SMALL / size::CHECK_BOX,
                    Color32::WHITE,
                );
            } else {
                painter.circle_filled(center, r, p.accent_soft.gamma_multiply(hover));
                painter.circle_stroke(center, r - stroke::CHECK / 2.0, border);
            }
        }
    }
}

/// A tick drawn for a [`size::CHECK_BOX`] box, scaled by `scale`.
fn check_mark(painter: &Painter, c: Pos2, scale: f32, color: Color32) {
    let s = Stroke::new(stroke::TICK * scale.max(0.8), color);
    let at = |x: f32, y: f32| c + vec2(x, y) * scale;
    painter.line_segment([at(-3.6, 0.2), at(-1.1, 2.8)], s);
    painter.line_segment([at(-1.1, 2.8), at(3.8, -2.9)], s);
}

/// A small pie showing `finished` of `total`, like a Things project icon.
/// Clicking it ticks the project by hand (`done` is its own state). Its
/// accessible name is the plain `finished/total` text.
pub fn progress_pie(ui: &mut Ui, finished: usize, total: usize, done: bool) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(size::CHECK, size::CHECK), Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(Role::CheckBox, true, done, format!("{finished}/{total}"))
    });
    let t = fade(ui, response.id, response.hovered());
    let (c, r) = (rect.center(), size::CHECK_BOX / 2.0 - 1.0);
    let ring = p.accent.lerp_to_gamma(p.text, 0.25 * t);
    let painter = ui.painter();
    painter.circle_stroke(c, r, Stroke::new(stroke::CHECK, ring));
    let share = if total == 0 {
        0.0
    } else {
        finished as f32 / total as f32
    };
    let inner = r - 2.0;
    if share >= 1.0 || done {
        painter.circle_filled(c, inner, ring);
    } else if share > 0.0 {
        let steps = (32.0 * share).ceil().max(2.0) as usize;
        let mut points = vec![c];
        points.extend((0..=steps).map(|i| {
            let a = -TAU / 4.0 + TAU * share * i as f32 / steps as f32;
            c + vec2(a.cos(), a.sin()) * inner
        }));
        painter.add(Shape::convex_polygon(points, ring, Stroke::NONE));
    }
    response
}

/// A Things-style tag: a small muted capsule with a dot in the category's
/// colour. Its accessible name is `name`.
pub fn tag(ui: &mut Ui, name: &str, color: Color32) -> Response {
    let p = palette(ui);
    let galley = ui
        .painter()
        .layout_no_wrap(name.to_owned(), font(text::TAG), p.secondary);
    let dot = size::TAG_DOT;
    let pad = space::SM;
    let width = pad + dot + space::XS + galley.size().x + pad;
    let (rect, response) = ui.allocate_exact_size(vec2(width, size::TAG), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, name));
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(radius::TAG), p.tag);
    let y = rect.center().y;
    painter.circle_filled(pos2(rect.left() + pad + dot / 2.0, y), dot / 2.0, color);
    let x = rect.left() + pad + dot + space::XS;
    painter.galley(pos2(x, y - galley.size().y / 2.0), galley, p.secondary);
    response
}

/// Restyles `ui` so a [`egui::ComboBox`] in it looks like a [`tag`]: a
/// small muted capsule that fits its text.
pub fn tag_style(ui: &mut Ui) {
    let p = palette(ui);
    let style = ui.style_mut();
    style.spacing.button_padding = vec2(space::SM, 0.0);
    style.spacing.interact_size.y = size::TAG;
    style.spacing.icon_width = space::SM;
    style.spacing.icon_spacing = space::XS;
    style.override_font_id = Some(font(text::TAG));
    let w = &mut style.visuals.widgets;
    for (v, fill) in [
        (&mut w.inactive, p.tag),
        (&mut w.hovered, p.selected),
        (&mut w.active, p.line),
        (&mut w.open, p.selected),
    ] {
        v.bg_fill = fill;
        v.weak_bg_fill = fill;
        v.bg_stroke = Stroke::NONE;
        v.corner_radius = CornerRadius::same(radius::TAG);
        v.fg_stroke = Stroke::new(stroke::HAIRLINE, p.secondary);
    }
}

/// The running task's live time and its stop control: a small blue capsule.
/// The whole capsule is the "Stop" button; the time shows inside it.
pub fn timer_capsule(ui: &mut Ui, time: &str) -> Response {
    let p = palette(ui);
    let galley = ui
        .painter()
        .layout_no_wrap(time.to_owned(), font(text::SMALL), p.accent);
    let pad = space::SM;
    let stop = space::SM;
    let width = pad + galley.size().x + space::SM + stop + pad;
    let (rect, response) = ui.allocate_exact_size(vec2(width, size::CAPSULE), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Stop"));
    let t = fade(ui, response.id, response.hovered());
    let painter = ui.painter();
    let fill = p.accent_soft.lerp_to_gamma(p.selection, t);
    painter.rect_filled(rect, CornerRadius::same(radius::PILL), fill);
    let y = rect.center().y;
    painter.galley(
        pos2(rect.left() + pad, y - galley.size().y / 2.0),
        galley,
        p.accent,
    );
    let square = Rect::from_center_size(pos2(rect.right() - pad - stop / 2.0, y), vec2(stop, stop));
    painter.rect_filled(square, CornerRadius::same(radius::STOP), p.accent);
    response.on_hover_text("Stop")
}

#[derive(Clone, Copy)]
pub enum Icon {
    Play,
    Star,
    Tag,
    Gear,
    Pencil,
    Trash,
    Plus,
    Doc,
    Back,
    Forward,
    Target,
    Focus,
    Chart,
    /// A cup with steam: a break.
    Cup,
    /// Two people: a meeting.
    People,
}

impl Icon {
    pub fn paint(self, painter: &Painter, rect: Rect, color: Color32) {
        let c = rect.center();
        match self {
            Icon::Play => {
                let pts = vec![
                    c + vec2(-3.5, -5.0),
                    c + vec2(5.0, 0.0),
                    c + vec2(-3.5, 5.0),
                ];
                painter.add(Shape::convex_polygon(pts, color, Stroke::NONE));
            }
            Icon::Star => star(painter, c, 7.5, color),
            Icon::Tag => {
                for (i, dy) in [-4.0, 0.0, 4.0].into_iter().enumerate() {
                    painter.circle_filled(c + vec2(-4.5, dy), 1.6, color);
                    let w = [7.0, 5.0, 6.0][i];
                    painter.line_segment(
                        [c + vec2(-1.5, dy), c + vec2(-1.5 + w, dy)],
                        Stroke::new(1.6, color),
                    );
                }
            }
            Icon::Gear => {
                painter.circle_stroke(c, 5.0, Stroke::new(2.0, color));
                painter.circle_filled(c, 1.8, color);
            }
            Icon::Pencil => {
                // A diagonal shaft with a short tip and an eraser band.
                painter.line_segment(
                    [c + vec2(-3.5, 3.5), c + vec2(4.0, -4.0)],
                    Stroke::new(2.6, color),
                );
                painter.line_segment(
                    [c + vec2(-5.0, 5.0), c + vec2(-3.8, 3.8)],
                    Stroke::new(1.4, color),
                );
            }
            Icon::Trash => {
                let s = Stroke::new(1.5, color);
                painter.line_segment([c + vec2(-5.5, -3.5), c + vec2(5.5, -3.5)], s);
                painter.line_segment([c + vec2(-1.8, -5.2), c + vec2(1.8, -5.2)], s);
                let body = [
                    c + vec2(-4.0, -3.5),
                    c + vec2(-3.2, 5.0),
                    c + vec2(3.2, 5.0),
                    c + vec2(4.0, -3.5),
                ];
                painter.add(Shape::line(body.to_vec(), s));
                painter.line_segment([c + vec2(0.0, -1.5), c + vec2(0.0, 3.0)], s);
            }
            Icon::Doc => {
                let s = Stroke::new(1.5, color);
                let page = Rect::from_center_size(c, vec2(10.0, 13.0));
                painter.rect_stroke(page, CornerRadius::same(2), s, StrokeKind::Middle);
                for dy in [-2.5, 0.5, 3.5] {
                    painter.line_segment([c + vec2(-2.5, dy), c + vec2(2.5, dy)], s);
                }
            }
            Icon::Back | Icon::Forward => {
                let dir = if matches!(self, Icon::Back) {
                    -1.0
                } else {
                    1.0
                };
                // The tip points the way it goes: left for Back.
                let s = Stroke::new(1.8, color);
                painter.line_segment([c + vec2(-2.0 * dir, -5.0), c + vec2(3.0 * dir, 0.0)], s);
                painter.line_segment([c + vec2(3.0 * dir, 0.0), c + vec2(-2.0 * dir, 5.0)], s);
            }
            Icon::Target => {
                painter.circle_stroke(c, 6.5, Stroke::new(1.5, color));
                painter.circle_stroke(c, 3.5, Stroke::new(1.5, color));
                painter.circle_filled(c, 1.3, color);
            }
            Icon::Focus => {
                // A ring with four short ticks: a viewfinder.
                let s = Stroke::new(1.5, color);
                painter.circle_stroke(c, 4.5, s);
                for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                    let d = vec2(dx, dy);
                    painter.line_segment([c + d * 6.0, c + d * 8.0], s);
                }
            }
            Icon::Chart => {
                // Three bars of rising height on a baseline.
                for (dx, h) in [(-4.5, 5.0), (0.0, 9.0), (4.5, 12.0)] {
                    let bar =
                        Rect::from_min_max(c + vec2(dx - 1.6, 6.0 - h), c + vec2(dx + 1.6, 6.0));
                    painter.rect_filled(bar, CornerRadius::same(1), color);
                }
            }
            Icon::Cup => {
                let s = Stroke::new(1.4, color);
                let body = Rect::from_min_max(c + vec2(-5.5, -2.0), c + vec2(2.5, 5.5));
                painter.rect_stroke(
                    body,
                    CornerRadius {
                        nw: 1,
                        ne: 1,
                        sw: 3,
                        se: 3,
                    },
                    s,
                    StrokeKind::Middle,
                );
                // The handle: the right half of a small ring.
                let handle: Vec<Pos2> = (0..=8)
                    .map(|i| {
                        let a = -TAU / 4.0 + TAU / 2.0 * i as f32 / 8.0;
                        c + vec2(2.5 + 2.2 * a.cos(), 1.2 + 2.0 * a.sin())
                    })
                    .collect();
                painter.add(Shape::line(handle, s));
                for dx in [-3.5, -0.5] {
                    painter.line_segment([c + vec2(dx, -4.0), c + vec2(dx, -6.5)], s);
                }
            }
            Icon::People => {
                let s = Stroke::new(1.4, color);
                // Shoulders: an arc of an ellipse under each head, between
                // two angles (the top half runs from half a turn to a full one).
                let shoulders = |x: f32, rx: f32, from: f32| -> Vec<Pos2> {
                    (0..=10)
                        .map(|i| {
                            let a = from + (TAU - from) * i as f32 / 10.0;
                            c + vec2(x + rx * a.cos(), 6.0 + 4.5 * a.sin())
                        })
                        .collect()
                };
                // The one behind, to the right, shows only its outer quarter.
                painter.circle_stroke(c + vec2(3.6, -2.2), 1.9, s);
                painter.add(Shape::line(shoulders(3.6, 3.2, TAU * 0.75), s));
                painter.circle_stroke(c + vec2(-2.0, -2.6), 2.3, s);
                painter.add(Shape::line(shoulders(-2.0, 4.2, TAU / 2.0), s));
            }
            Icon::Plus => {
                let s = Stroke::new(1.8, color);
                painter.line_segment([c + vec2(-5.0, 0.0), c + vec2(5.0, 0.0)], s);
                painter.line_segment([c + vec2(0.0, -5.0), c + vec2(0.0, 5.0)], s);
            }
        }
    }
}

fn star(painter: &Painter, c: Pos2, r: f32, color: Color32) {
    // A five-pointed star as a fan of triangles from the centre (each one convex).
    let point = |i: usize| {
        let a = -TAU / 4.0 + TAU * i as f32 / 10.0;
        let rr = if i.is_multiple_of(2) { r } else { r * 0.45 };
        c + vec2(a.cos(), a.sin()) * rr
    };
    for i in 0..10 {
        painter.add(Shape::convex_polygon(
            vec![c, point(i), point((i + 1) % 10)],
            color,
            Stroke::NONE,
        ));
    }
}

/// A quiet square icon button; `name` is its accessible label.
pub fn icon_button(ui: &mut Ui, icon: Icon, name: &str, color: Color32) -> Response {
    let p = palette(ui);
    let side = size::ICON_BUTTON;
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, name));
    let t = fade(ui, response.id, response.hovered());
    faded_fill(ui.painter(), rect, radius::SM, p.hover, t);
    icon.paint(ui.painter(), rect, color);
    response.on_hover_text(name)
}

/// A rounded pill button: `fill` behind, `text` in `color`. It deepens a
/// little on hover.
pub fn pill(ui: &mut Ui, text: &str, fill: Color32, color: Color32) -> Response {
    let p = palette(ui);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font(text::BODY), color);
    let width = galley.size().x + 2.0 * space::MD + 2.0;
    let (rect, response) = ui.allocate_exact_size(vec2(width, size::CONTROL), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, text));
    let t = fade(ui, response.id, response.hovered());
    let fill = fill.lerp_to_gamma(p.text, 0.08 * t);
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(radius::PILL), fill);
    painter.galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}

/// A screen's title: a coloured icon and a large semibold name, Things-style.
pub fn screen_title(ui: &mut Ui, icon: Icon, color: Color32, title: &str) {
    screen_header(ui, icon, color, title, None);
}

/// [`screen_title`] with a quiet line under it, such as today's date.
pub fn screen_header(ui: &mut Ui, icon: Icon, color: Color32, title: &str, subtitle: Option<&str>) {
    ui.horizontal(|ui| {
        let side = size::TITLE_ICON;
        let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
        icon.paint(ui.painter(), rect.expand(3.0), color);
        ui.heading(title);
    });
    if let Some(subtitle) = subtitle {
        ui.label(RichText::new(subtitle).color(palette(ui).secondary));
    }
    ui.add_space(space::LG);
}

/// A frameless text button, optionally in the accent colour.
pub fn text_button(ui: &mut Ui, text: &str, accent: bool) -> Response {
    let p = palette(ui);
    let color = if accent { p.accent } else { p.text };
    ui.add(Button::new(RichText::new(text).color(color)).frame(false))
}

/// Small grey upper-case heading for a group of items.
pub fn section_label(ui: &mut Ui, text: &str) {
    ui.add_space(space::LG);
    ui.label(
        RichText::new(text.to_uppercase())
            .font(bold(ui, text::CAPTION))
            .color(palette(ui).secondary),
    );
    ui.add_space(space::XS);
}

/// The card surface: soft fill, hairline border, rounded corners and a
/// subtle shadow. Fills the available width.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    let p = palette(ui);
    Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.hairline))
        .corner_radius(CornerRadius::same(radius::LG))
        .shadow(card_shadow(&p))
        .inner_margin(Margin::symmetric(space::LG as i8, space::MD as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
}

/// A raised card, lifted off the list by a deeper shadow: the inline editor
/// a task opens into, Things-style. It reaches [`space::SM`] past the
/// available width on each side, so its content lines up with the rows.
pub fn raised_card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    let p = palette(ui);
    Frame::new()
        .fill(p.raised)
        .stroke(Stroke::new(stroke::HAIRLINE, p.hairline))
        .corner_radius(CornerRadius::same(radius::MD))
        .shadow(raised_shadow(&p))
        .inner_margin(Margin::same(space::SM as i8))
        .outer_margin(Margin::symmetric(-(space::SM as i8), 0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
}

/// Paints the card surface into `rect`, for cards drawn with the painter.
pub fn paint_card(painter: &Painter, rect: Rect, p: &Palette) {
    let rounding = CornerRadius::same(radius::LG);
    painter.add(card_shadow(p).as_shape(rect, rounding));
    painter.rect(
        rect,
        rounding,
        p.card,
        Stroke::new(1.0, p.hairline),
        StrokeKind::Inside,
    );
}

/// A full-width list row `size::ROW` high whose background fades in while
/// the pointer is over it. `id` keeps the fade apart from other rows.
pub fn list_row<R>(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let p = palette(ui);
    let background = ui.painter().add(Shape::Noop);
    let inner = ui.allocate_ui_with_layout(
        vec2(ui.available_width(), size::ROW),
        Layout::left_to_right(Align::Center),
        add,
    );
    let rect = inner.response.rect.expand2(vec2(space::SM, 0.0));
    let t = fade(
        ui,
        ui.scope_id().with(("row", id)),
        ui.rect_contains_pointer(rect),
    );
    if t > 0.0 {
        ui.painter().set(
            background,
            Shape::rect_filled(
                rect,
                CornerRadius::same(radius::MD),
                p.hover.gamma_multiply(t),
            ),
        );
    }
    inner.inner
}

/// Lays `add` out in a fixed-width column of a row, so columns line up
/// from row to row. Long text should be truncated by the caller.
pub fn column<R>(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope_builder(
        UiBuilder::new().layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.set_width(width);
            add(ui)
        },
    )
    .inner
}

/// A one-line label cut short with "…" when it doesn't fit.
pub fn truncated(ui: &mut Ui, text: impl Into<egui::WidgetText>) -> Response {
    ui.add(Label::new(text).truncate())
}

/// One aligned chart row: a dot, a name column, a bar and a value column.
/// Its accessible name is `"{name} {value}"`.
pub fn bar_row(
    ui: &mut Ui,
    dot: Option<Color32>,
    name: &str,
    share: f32,
    bar_color: Color32,
    value: &str,
) -> Response {
    let p = palette(ui);
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, size::ROW - 4.0), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, format!("{name} {value}")));
    let painter = ui.painter_at(rect);
    let y = rect.center().y;
    let mut x = rect.left();
    if let Some(color) = dot {
        painter.circle_filled(pos2(x + size::ICON / 2.0, y), 4.5, color);
    }
    x += size::ICON + space::SM;
    let name_width = size::LABEL_COLUMN - size::ICON - space::SM;
    let galley = painter.layout_no_wrap(name.to_owned(), font(text::BODY), p.text);
    let name_painter = ui.painter_at(Rect::from_x_y_ranges(x..=x + name_width, rect.y_range()));
    name_painter.galley(pos2(x, y - galley.size().y / 2.0), galley, p.text);
    x = rect.left() + size::LABEL_COLUMN + space::SM;
    let bar_width =
        (rect.right() - x - size::VALUE_COLUMN - space::MD).clamp(0.0, size::BAR_COLUMN);
    let bar = Rect::from_min_size(pos2(x, y - size::BAR / 2.0), vec2(bar_width, size::BAR));
    paint_bar(&painter, bar, share, bar_color, p.hover);
    painter.text(
        pos2(bar.right() + space::MD, y),
        Align2::LEFT_CENTER,
        value,
        font(text::BODY),
        p.secondary,
    );
    response
}

/// A calm placeholder for an empty list: an icon in a soft circle, a line
/// of text and an optional hint.
pub fn empty_state(ui: &mut Ui, icon: Icon, title: &str, hint: Option<&str>) {
    let p = palette(ui);
    ui.add_space(space::XL);
    ui.vertical_centered(|ui| {
        let side = size::EMPTY_ICON;
        let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
        ui.painter()
            .circle_filled(rect.center(), side / 2.0, p.accent_soft);
        icon.paint(ui.painter(), rect, p.accent);
        ui.add_space(space::SM);
        ui.label(RichText::new(title).color(p.text));
        if let Some(hint) = hint {
            ui.label(RichText::new(hint).small().color(p.secondary));
        }
    });
    ui.add_space(space::XL);
}

/// One full-width sidebar row: an icon, a label and an optional count.
/// `name` is the accessible label (it may differ from what's shown).
pub fn sidebar_row(
    ui: &mut Ui,
    selected: bool,
    paint_icon: impl FnOnce(&Painter, Rect),
    text: &str,
    count: Option<usize>,
    name: &str,
) -> Response {
    let p = palette(ui);
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, size::SIDEBAR_ROW), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, name));
    let hover = fade(
        ui,
        response.id.with("hover"),
        response.hovered() && !selected,
    );
    let chosen = fade(ui, response.id.with("selected"), selected);
    let painter = ui.painter();
    let rounding = CornerRadius::same(radius::MD);
    faded_fill(painter, rect, radius::MD, p.hover, hover);
    if chosen > 0.0 {
        painter.rect_filled(rect, rounding, p.selected.gamma_multiply(chosen));
    }
    let icon_x = rect.left() + space::MD + size::ICON / 2.0;
    paint_icon(
        painter,
        Rect::from_center_size(pos2(icon_x, rect.center().y), vec2(size::ICON, size::ICON)),
    );
    let label_font = if selected {
        bold(ui, text::BODY)
    } else {
        font(text::BODY)
    };
    painter.text(
        pos2(icon_x + size::ICON / 2.0 + space::SM + 2.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        label_font,
        p.text,
    );
    if let Some(count) = count.filter(|c| *c > 0) {
        painter.text(
            pos2(rect.right() - space::MD, rect.center().y),
            Align2::RIGHT_CENTER,
            count.to_string(),
            font(text::SMALL),
            p.secondary,
        );
    }
    response
}

pub enum DeleteClick {
    Arm,
    Confirm,
    Cancel,
}

/// Two-step delete: "Delete" arms it, then "Confirm delete" or "Cancel".
pub fn delete_buttons(ui: &mut Ui, armed: bool) -> Option<DeleteClick> {
    let p = palette(ui);
    if !armed {
        return icon_button(ui, Icon::Trash, "Delete", p.secondary)
            .clicked()
            .then_some(DeleteClick::Arm);
    }
    if pill(ui, "Confirm delete", p.danger, Color32::WHITE).clicked() {
        Some(DeleteClick::Confirm)
    } else if ui.add(Button::new("Cancel").frame(false)).clicked() {
        Some(DeleteClick::Cancel)
    } else {
        None
    }
}

// ---- The header strip -------------------------------------------------------

/// One line of `text`, cut short with "…" past `max_width`.
fn elided(ui: &Ui, text: &str, font: FontId, color: Color32, max_width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = TextWrapping::truncate_at_width(max_width.max(1.0));
    ui.painter().layout_job(job)
}

/// The widest digit's advance: every digit of a running time sits in a slot
/// this wide, so the time doesn't jitter as it counts.
fn digit_slot(ui: &Ui, font: &FontId) -> f32 {
    ui.ctx().fonts_mut(|f| {
        ('0'..='9')
            .map(|d| f.glyph_width(font, d))
            .fold(0.0, f32::max)
    })
}

/// How wide [`hero_time`] lays `time` out.
pub fn hero_time_width(ui: &Ui, time: &str) -> f32 {
    let face = font(text::HERO);
    let slot = digit_slot(ui, &face);
    ui.ctx().fonts_mut(|f| {
        time.chars()
            .map(|c| {
                if c.is_ascii_digit() {
                    slot
                } else {
                    f.glyph_width(&face, c)
                }
            })
            .sum()
    })
}

/// The running timer's elapsed time, large, in the regular UI font. Digits sit in
/// equal slots (centred in each), so the width never changes as it counts.
/// Its accessible name is `time`.
pub fn hero_time(ui: &mut Ui, time: &str, color: Color32) -> Response {
    let face = font(text::HERO);
    let slot = digit_slot(ui, &face);
    let width = hero_time_width(ui, time);
    let height = ui.ctx().fonts_mut(|f| f.row_height(&face));
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, time));
    let painter = ui.painter();
    let mut x = rect.left();
    for c in time.chars() {
        let galley = painter.layout_no_wrap(c.to_string(), face.clone(), color);
        let advance = if c.is_ascii_digit() {
            slot
        } else {
            galley.size().x
        };
        let left = x + (advance - galley.size().x) / 2.0;
        painter.galley(pos2(left, rect.top()), galley, color);
        x += advance;
    }
    response
}

/// The width [`running_title`] wants for `title` over `subtitle`.
pub fn running_title_width(ui: &Ui, title: &str, subtitle: Option<&str>) -> f32 {
    let painter = ui.painter();
    let title = painter
        .layout_no_wrap(title.to_owned(), bold(ui, text::BODY), Color32::WHITE)
        .size()
        .x;
    let subtitle = subtitle.map_or(0.0, |s| {
        painter
            .layout_no_wrap(s.to_owned(), font(text::SMALL), Color32::WHITE)
            .size()
            .x
    });
    size::ICON + space::SM + title.max(subtitle)
}

/// What's running: a dot in its colour, the title in semibold and, under
/// it, an optional quiet line (the parent project, what comes next). Both
/// lines are cut short with "…" to fit `max_width`. Its accessible name is
/// `title`. With no `dot` it is the idle state: an empty ring and a quiet
/// regular-weight title.
pub fn running_title(
    ui: &mut Ui,
    dot: Option<Color32>,
    title: &str,
    subtitle: Option<&str>,
    max_width: f32,
) -> Response {
    let p = palette(ui);
    let width = running_title_width(ui, title, subtitle).min(max_width);
    let (rect, response) = ui.allocate_exact_size(vec2(width, size::HEADER), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, title));
    let text_left = rect.left() + size::ICON + space::SM;
    let text_width = rect.right() - text_left;
    let (title_font, title_color) = match dot {
        Some(_) => (bold(ui, text::BODY), p.text),
        None => (font(text::BODY), p.muted),
    };
    let title = elided(ui, title, title_font, title_color, text_width);
    let subtitle = subtitle.map(|s| elided(ui, s, font(text::SMALL), p.secondary, text_width));
    let title_height = title.size().y;
    let height = title_height + subtitle.as_ref().map_or(0.0, |s| s.size().y);
    let top = rect.center().y - height / 2.0;
    let painter = ui.painter();
    let dot_center = pos2(
        rect.left() + size::ICON / 2.0,
        top + title_height / 2.0 + 0.5,
    );
    let r = size::HEADER_DOT / 2.0;
    match dot {
        Some(color) => {
            painter.circle_filled(dot_center, r, color);
        }
        None => {
            painter.circle_stroke(
                dot_center,
                r - stroke::CHECK / 2.0,
                Stroke::new(stroke::CHECK, p.check),
            );
        }
    }
    painter.galley(pos2(text_left, top), title, title_color);
    if let Some(subtitle) = subtitle {
        painter.galley(pos2(text_left, top + title_height), subtitle, p.secondary);
    }
    response
}

/// A round Stop button: a soft accent circle with a square in it. On hover
/// the circle fills with the accent and the square turns white.
pub fn stop_button(ui: &mut Ui) -> Response {
    let p = palette(ui);
    let side = size::STOP_BUTTON;
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Stop"));
    let t = fade(ui, response.id, response.hovered());
    let painter = ui.painter();
    painter.circle_filled(
        rect.center(),
        side / 2.0,
        p.accent_soft.lerp_to_gamma(p.accent, t),
    );
    let glyph = size::STOP_GLYPH;
    painter.rect_filled(
        Rect::from_center_size(rect.center(), vec2(glyph, glyph)),
        CornerRadius::same(radius::STOP),
        p.accent.lerp_to_gamma(Color32::WHITE, t),
    );
    response.on_hover_text("Stop")
}

/// How a [`header_button`] looks at rest.
#[derive(Clone, Copy)]
pub enum HeaderLook {
    /// No fill until hovered; quiet text.
    Ghost,
    /// A tinted capsule for an active state or the main action: `fill`
    /// behind, `color` for the icon and text.
    Tinted { fill: Color32, color: Color32 },
}

/// A header control: a painted icon and a label in a capsule. `compact`
/// drops the label (it shows as a tooltip instead). The label is cut short
/// past `max_label`; `name` is the accessible name, kept whole.
pub fn header_button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    name: &str,
    look: HeaderLook,
    compact: bool,
    max_label: f32,
) -> Response {
    let p = palette(ui);
    let color = match look {
        HeaderLook::Ghost => p.muted,
        HeaderLook::Tinted { color, .. } => color,
    };
    let galley = (!compact).then(|| elided(ui, label, font(text::BODY), color, max_label));
    let icon_gap = space::SM - 2.0;
    let width = match &galley {
        Some(g) => space::SM + size::ICON + icon_gap + g.size().x + space::MD,
        None => size::HEADER_BUTTON,
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, size::HEADER_BUTTON), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, name));
    let t = fade(ui, response.id, response.hovered());
    let painter = ui.painter();
    let (icon_color, text_color) = match look {
        HeaderLook::Ghost => {
            faded_fill(painter, rect, radius::HEADER_BUTTON, p.hover, t);
            (
                p.secondary.lerp_to_gamma(p.text, t),
                p.muted.lerp_to_gamma(p.text, t),
            )
        }
        HeaderLook::Tinted { fill, color } => {
            painter.rect_filled(
                rect,
                CornerRadius::same(radius::HEADER_BUTTON),
                fill.lerp_to_gamma(color, 0.12 * t),
            );
            (color, color)
        }
    };
    let y = rect.center().y;
    let icon_center = match &galley {
        Some(_) => pos2(rect.left() + space::SM + size::ICON / 2.0, y),
        None => rect.center(),
    };
    icon.paint(
        painter,
        Rect::from_center_size(icon_center, vec2(size::ICON, size::ICON)),
        icon_color,
    );
    match galley {
        Some(galley) => {
            let x = icon_center.x + size::ICON / 2.0 + icon_gap;
            painter.galley(pos2(x, y - galley.size().y / 2.0), galley, text_color);
            response
        }
        None => response.on_hover_text(name),
    }
}

/// Today's office time: a short line (`lead` quiet, `worked` firm, `rest`
/// quiet) over a slim rounded bar filled to `share` in `color`. Its
/// accessible name is `name`.
pub fn office_meter(
    ui: &mut Ui,
    [lead, worked, rest]: [&str; 3],
    share: f32,
    color: Color32,
    name: &str,
) -> Response {
    let p = palette(ui);
    let colors = [p.secondary, p.text, p.secondary];
    let fonts = [font(text::SMALL), bold(ui, text::SMALL), font(text::SMALL)];
    let parts: Vec<Arc<Galley>> = [lead, worked, rest]
        .into_iter()
        .zip(fonts)
        .zip(colors)
        .map(|((s, f), c)| ui.painter().layout_no_wrap(s.to_owned(), f, c))
        .collect();
    let line_width: f32 = parts.iter().map(|g| g.size().x).sum();
    let line_height = parts.iter().map(|g| g.size().y).fold(0.0, f32::max);
    let width = line_width.max(size::METER_MIN);
    let (rect, response) = ui.allocate_exact_size(vec2(width, size::HEADER), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, name));
    let gap = space::XS + 1.0;
    let top = rect.center().y - (line_height + gap + size::METER_BAR) / 2.0;
    let painter = ui.painter();
    let mut x = rect.left();
    for (galley, c) in parts.into_iter().zip(colors) {
        let w = galley.size().x;
        painter.galley(pos2(x, top), galley, c);
        x += w;
    }
    let bar = Rect::from_min_size(
        pos2(rect.left(), top + line_height + gap),
        vec2(width, size::METER_BAR),
    );
    paint_bar(painter, bar, share, color, p.hover);
    response
}
