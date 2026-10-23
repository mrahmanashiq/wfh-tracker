//! Small painted widgets shared by the screens. Everything that has no visible
//! text gets an accessible name, so screen readers (and the UI tests) find it.

use std::f32::consts::TAU;

use egui::{
    Button, Color32, CornerRadius, Painter, Pos2, Rect, Response, RichText, Role, Sense, Shape,
    Stroke, Ui, WidgetInfo, pos2, vec2,
};
use jiff::SignedDuration;
use jiff::civil::Weekday;
use wt_core::{Rgb, hours_text};

use crate::theme::palette;

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

/// A small filled circle in a category's colour.
pub fn color_dot(ui: &mut Ui, color: Rgb) {
    let (rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), 4.5, color32(color));
}

/// A thin bar `width` wide, filled to `share` (clamped to 0..=1).
pub fn progress_bar(ui: &mut Ui, width: f32, share: f32, color: Color32) {
    let p = palette(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 6.0), Sense::hover());
    let rounding = CornerRadius::same(3);
    ui.painter().rect_filled(rect, rounding, p.hover);
    let mut done = rect;
    done.set_width(rect.width() * share.clamp(0.0, 1.0));
    ui.painter().rect_filled(done, rounding, color);
}

/// Shows the result of the last action in the theme's danger colour.
pub fn message(ui: &mut Ui, message: &Option<String>) {
    if let Some(text) = message {
        ui.label(RichText::new(text).color(palette(ui).danger));
    }
}

/// A round checkbox, Things-style. `label` names it for accessibility; the
/// visible title is painted separately by the row.
pub fn round_check(ui: &mut Ui, done: bool, label: &str) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::CheckBox, true, done, label));
    let center = rect.center();
    let painter = ui.painter();
    if done {
        painter.circle_filled(center, 8.5, p.accent);
        check_mark(painter, center, Color32::WHITE);
    } else {
        let ring = if response.hovered() { p.accent } else { p.line };
        painter.circle_stroke(center, 8.0, Stroke::new(1.5, ring));
    }
    response
}

fn check_mark(painter: &Painter, c: Pos2, color: Color32) {
    let stroke = Stroke::new(1.8, color);
    painter.line_segment([c + vec2(-4.0, 0.2), c + vec2(-1.2, 3.0)], stroke);
    painter.line_segment([c + vec2(-1.2, 3.0), c + vec2(4.2, -3.2)], stroke);
}

/// A small pie showing `done` of `total`, like a Things project icon.
/// Its accessible name is the plain `done/total` text.
pub fn progress_pie(ui: &mut Ui, done: usize, total: usize) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, format!("{done}/{total}")));
    let (c, r) = (rect.center(), 6.5);
    let painter = ui.painter();
    painter.circle_stroke(c, r, Stroke::new(1.3, p.accent));
    let share = if total == 0 {
        0.0
    } else {
        done as f32 / total as f32
    };
    if share >= 1.0 {
        painter.circle_filled(c, r - 1.8, p.accent);
    } else if share > 0.0 {
        let steps = (32.0 * share).ceil().max(2.0) as usize;
        let inner = r - 1.8;
        let mut points = vec![c];
        points.extend((0..=steps).map(|i| {
            let a = -TAU / 4.0 + TAU * share * i as f32 / steps as f32;
            c + vec2(a.cos(), a.sin()) * inner
        }));
        painter.add(Shape::convex_polygon(points, p.accent, Stroke::NONE));
    }
    response
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
                painter.rect_stroke(page, CornerRadius::same(2), s, egui::StrokeKind::Middle);
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
                let s = Stroke::new(1.8, color);
                painter.line_segment([c + vec2(2.0 * dir, -5.0), c + vec2(-3.0 * dir, 0.0)], s);
                painter.line_segment([c + vec2(-3.0 * dir, 0.0), c + vec2(2.0 * dir, 5.0)], s);
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
    let (rect, response) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, name));
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(6), p.hover);
    }
    icon.paint(ui.painter(), rect, color);
    response.on_hover_text(name)
}

/// A rounded pill button: `fill` behind, `text` in `color`.
pub fn pill(ui: &mut Ui, text: &str, fill: Color32, color: Color32) -> Response {
    ui.add(
        Button::new(RichText::new(text).color(color))
            .fill(fill)
            .corner_radius(CornerRadius::same(14))
            .min_size(vec2(0.0, 28.0)),
    )
}

/// A screen's title: a coloured icon and a large bold name, Things-style.
pub fn screen_title(ui: &mut Ui, icon: Icon, color: Color32, title: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
        icon.paint(ui.painter(), rect.expand(3.0), color);
        ui.heading(title);
    });
    ui.add_space(16.0);
}

/// A frameless text button, optionally in the accent colour.
pub fn text_button(ui: &mut Ui, text: &str, accent: bool) -> Response {
    let p = palette(ui);
    let color = if accent { p.accent } else { p.text };
    ui.add(Button::new(RichText::new(text).color(color)).frame(false))
}

/// Small grey upper-case heading for a group of sidebar items.
pub fn section_label(ui: &mut Ui, text: &str) {
    ui.add_space(14.0);
    ui.label(
        RichText::new(text.to_uppercase())
            .small()
            .strong()
            .color(palette(ui).secondary),
    );
    ui.add_space(2.0);
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
    let (rect, response) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, name));
    let painter = ui.painter();
    if selected || response.hovered() {
        let fill = if selected { p.line } else { p.hover };
        painter.rect_filled(rect, CornerRadius::same(6), fill);
    }
    paint_icon(
        painter,
        Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), vec2(16.0, 16.0)),
    );
    painter.text(
        pos2(rect.left() + 34.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(14.0),
        p.text,
    );
    if let Some(count) = count.filter(|c| *c > 0) {
        painter.text(
            pos2(rect.right() - 10.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            count.to_string(),
            egui::FontId::proportional(13.0),
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
