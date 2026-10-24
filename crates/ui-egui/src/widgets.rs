//! Small painted widgets shared by the screens. Everything that has no visible
//! text gets an accessible name, so screen readers (and the UI tests) find it.
//! Sizes, gaps and colours come from [`crate::theme`].

use std::f32::consts::TAU;

use egui::{
    Align, Align2, Button, Color32, CornerRadius, Frame, Id, InnerResponse, Label, Layout, Margin,
    Painter, Pos2, Rect, Response, RichText, Role, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder,
    WidgetInfo, pos2, vec2,
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
    Stop,
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
            Icon::Stop => {
                let r = Rect::from_center_size(c, vec2(9.0, 9.0));
                painter.rect_filled(r, CornerRadius::same(2), color);
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
