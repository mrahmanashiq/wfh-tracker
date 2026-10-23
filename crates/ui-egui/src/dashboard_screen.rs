//! The Dashboard: today at a glance and this week's progress. It only reads.
//! Charts are painted with egui's painter; every mark is a hoverable widget
//! with an accessible name.

use egui::{
    Align2, Color32, CornerRadius, FontId, Rect, Response, RichText, Role, ScrollArea, Sense,
    Stroke, Ui, WidgetInfo, pos2, vec2,
};
use jiff::civil::Date;
use jiff::{SignedDuration, ToSpan};
use wt_core::repo::RepoResult;
use wt_core::{
    Category, CategoryKind, Clock, DayOffice, DistractionTotals, Id, Period, Settings, Target,
    Task, TimelineSegment, category_totals, clock_text, day_timeline, distraction_totals,
    duration_text, kind_time, office_by_day, timeline_window,
};

use crate::DashboardStore;
use crate::theme::{Palette, palette};
use crate::widgets::{Icon, color_dot, color32, message, screen_title, section_label};

const DAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const CARD_HEIGHT: f32 = 84.0;
const GAP: f32 = 12.0;
const STRIP_HEIGHT: f32 = 26.0;
const WEEK_CHART_HEIGHT: f32 = 120.0;
const BAR_ROUNDING: CornerRadius = CornerRadius::same(3);

/// Today's cards and timeline, then this week's office bars, category
/// progress and distractions.
#[derive(Default)]
pub struct DashboardScreen;

/// Everything the screen shows, loaded once per frame.
struct Data {
    today: Date,
    now_min: u16,
    categories: Vec<Category>,
    tasks: Vec<Task>,
    targets: Vec<Target>,
    office_today: SignedDuration,
    learning_today: SignedDuration,
    personal_today: SignedDuration,
    distractions_today: SignedDuration,
    timeline: Vec<TimelineSegment>,
    week: Vec<DayOffice>,
    week_categories: Vec<(Id, SignedDuration)>,
    week_distractions: DistractionTotals,
}

impl DashboardScreen {
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        store: &dyn DashboardStore,
        settings: &Settings,
        clock: &dyn Clock,
    ) {
        let p = palette(ui);
        screen_title(ui, Icon::Chart, p.accent, "Dashboard");
        let data = match load(store, settings, clock) {
            Ok(data) => data,
            Err(e) => {
                message(ui, &Some(format!("Could not load the dashboard: {e}")));
                return;
            }
        };
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            section_label(ui, "Today");
            cards(ui, &data, settings);
            ui.add_space(GAP);
            timeline(ui, &data);
            section_label(ui, "This week");
            week_bars(ui, &data);
            ui.add_space(GAP);
            category_rows(ui, &data);
            section_label(ui, "Distractions this week");
            distraction_rows(ui, &data.week_distractions, p);
        });
    }
}

fn load(store: &dyn DashboardStore, settings: &Settings, clock: &dyn Clock) -> RepoResult<Data> {
    let (tz, now, today) = (clock.time_zone(), clock.now(), clock.today());
    let (first, after) = wt_core::period_range(today, Period::Week, settings.week_start);
    let since = first.to_zoned(tz.clone()).map_or(now, |z| z.timestamp());
    let sessions = store.sessions_since(since)?;
    let events = store.distractions_since(since)?;
    let rules = store.rules()?;
    let tomorrow = today.checked_add(1.day()).unwrap_or(today);
    let local = now.to_zoned(tz.clone());

    let mut week_categories: Vec<(Id, SignedDuration)> =
        category_totals(&sessions, first, after, &tz, now)
            .into_iter()
            .map(|t| (t.category_id, t.time))
            .collect();
    let targets: Vec<Target> = store
        .targets()?
        .into_iter()
        .filter(|t| t.period == Period::Week)
        .collect();
    for target in &targets {
        if !week_categories
            .iter()
            .any(|(id, _)| *id == target.category_id)
        {
            week_categories.push((target.category_id, SignedDuration::ZERO));
        }
    }

    Ok(Data {
        today,
        now_min: (local.hour() as u16) * 60 + local.minute() as u16,
        categories: store.list()?,
        tasks: store.list_live()?,
        targets,
        office_today: wt_core::office_time(&sessions, today, &tz, now),
        learning_today: kind_time(&sessions, today, &tz, now, |k| k == CategoryKind::Learning),
        personal_today: kind_time(&sessions, today, &tz, now, |k| {
            matches!(k, CategoryKind::Personal | CategoryKind::Custom)
        }),
        distractions_today: distraction_totals(&events, &rules, today, tomorrow, &tz, now).total,
        timeline: day_timeline(&sessions, today, &tz, now),
        week: office_by_day(&sessions, first, &tz, now, settings),
        week_categories,
        week_distractions: distraction_totals(&events, &rules, first, after, &tz, now),
    })
}

// Today cards

fn cards(ui: &mut Ui, data: &Data, settings: &Settings) {
    let p = palette(ui);
    let width = ((ui.available_width() - 3.0 * GAP) / 4.0).max(120.0);
    let off_day = !settings.workdays.contains(&data.today.weekday());
    let target = SignedDuration::from_mins(i64::from(settings.office_target_min));
    let overtime = data
        .week
        .iter()
        .find(|d| d.date == data.today)
        .map_or(SignedDuration::ZERO, |d| d.overtime);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        let office = hours_text(data.office_today);
        let (value, note) = if off_day {
            (office.clone(), "day off".to_owned())
        } else {
            (
                format!("{office} / {}", target_text(target)),
                if overtime.as_mins() > 0 {
                    format!("{} overtime", duration_text(overtime))
                } else {
                    String::new()
                },
            )
        };
        let name = if off_day {
            format!("Office {office} - day off")
        } else {
            format!("Office {value}")
        };
        let response = card(ui, width, "Office", &value, &note, &name);
        if !off_day {
            // The bar fills to the target; overtime past the ceiling is danger.
            let bar = bar_rect(response.rect);
            let share = ratio(data.office_today, target);
            ui.painter().rect_filled(bar, BAR_ROUNDING, p.hover);
            let done = Rect::from_min_size(bar.min, vec2(bar.width() * share, bar.height()));
            let color = if overtime.is_positive() {
                p.danger
            } else if data.office_today >= target {
                p.success
            } else {
                p.accent
            };
            ui.painter().rect_filled(done, BAR_ROUNDING, color);
        }
        for (title, time) in [
            ("Learning", data.learning_today),
            ("Personal", data.personal_today),
            ("Distractions", data.distractions_today),
        ] {
            let value = hours_text(time);
            card(ui, width, title, &value, "", &format!("{title} {value}"));
        }
    });
}

/// A painted card: a small title, a large value and an optional note.
fn card(ui: &mut Ui, width: f32, title: &str, value: &str, note: &str, name: &str) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(width, CARD_HEIGHT), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, name));
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(10), p.sidebar);
    let left = rect.left() + 14.0;
    painter.text(
        pos2(left, rect.top() + 12.0),
        Align2::LEFT_TOP,
        title,
        FontId::proportional(12.0),
        p.secondary,
    );
    painter.text(
        pos2(left, rect.top() + 30.0),
        Align2::LEFT_TOP,
        value,
        FontId::proportional(20.0),
        p.text,
    );
    if !note.is_empty() {
        let color = if note == "day off" {
            p.secondary
        } else {
            p.danger
        };
        painter.text(
            pos2(rect.right() - 14.0, rect.top() + 12.0),
            Align2::RIGHT_TOP,
            note,
            FontId::proportional(12.0),
            color,
        );
    }
    response
}

fn bar_rect(card: Rect) -> Rect {
    Rect::from_min_max(
        pos2(card.left() + 14.0, card.bottom() - 16.0),
        pos2(card.right() - 14.0, card.bottom() - 10.0),
    )
}

// Today timeline

fn timeline(ui: &mut Ui, data: &Data) {
    let p = palette(ui);
    let (from, to) = timeline_window(&data.timeline);
    let span = f32::from(to - from);
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), STRIP_HEIGHT + 18.0),
        Sense::hover(),
    );
    let strip = Rect::from_min_size(rect.min, vec2(rect.width(), STRIP_HEIGHT));
    let x_at =
        |min: u16| strip.left() + strip.width() * f32::from(min.clamp(from, to) - from) / span;
    ui.painter()
        .rect_filled(strip, CornerRadius::same(6), p.sidebar);

    // Hour ticks under the strip, labelled every hour or every other hour.
    let step = if to - from > 12 * 60 { 120 } else { 60 };
    for hour in (from..=to).step_by(60) {
        let x = x_at(hour);
        ui.painter().line_segment(
            [pos2(x, strip.bottom()), pos2(x, strip.bottom() + 4.0)],
            Stroke::new(1.0, p.line),
        );
        if (hour - from) % step == 0 {
            let align = match hour {
                h if h == from => Align2::LEFT_TOP,
                h if h == to => Align2::RIGHT_TOP,
                _ => Align2::CENTER_TOP,
            };
            ui.painter().text(
                pos2(x, strip.bottom() + 5.0),
                align,
                clock_text(hour),
                FontId::proportional(11.0),
                p.secondary,
            );
        }
    }

    for (i, segment) in data.timeline.iter().enumerate() {
        let (x0, x1) = (x_at(segment.from_min), x_at(segment.to_min));
        // A 1px surface gap on each side keeps neighbours apart; at least 2px wide.
        let (x0, x1) = if x1 - x0 > 4.0 {
            (x0 + 1.0, x1 - 1.0)
        } else {
            (x0, x0 + 2.0)
        };
        let mark = Rect::from_min_max(pos2(x0, strip.top() + 3.0), pos2(x1, strip.bottom() - 3.0));
        let name = segment_name(data, segment);
        let response = ui
            .interact(mark, ui.scope_id().with(("segment", i)), Sense::hover())
            .on_hover_text(&name);
        response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &name));
        let color = if segment.kind == CategoryKind::Break {
            p.line
        } else {
            category_color(data, segment.category_id, p)
        };
        let color = if response.hovered() {
            color.gamma_multiply(0.8)
        } else {
            color
        };
        ui.painter().rect_filled(mark, CornerRadius::same(4), color);
    }

    if (from..=to).contains(&data.now_min) {
        let x = x_at(data.now_min);
        ui.painter().line_segment(
            [pos2(x, strip.top() - 2.0), pos2(x, strip.bottom() + 2.0)],
            Stroke::new(2.0, p.text),
        );
        ui.painter()
            .circle_filled(pos2(x, strip.top() - 2.0), 3.0, p.text);
    }
}

/// `Checkout 09:00-10:30`: the task, or the category for a task-less session.
fn segment_name(data: &Data, segment: &TimelineSegment) -> String {
    let what = match segment.task_id {
        Some(id) => data
            .tasks
            .iter()
            .find(|t| t.meta.id == id)
            .map_or("(deleted task)", |t| t.title.as_str()),
        None => category_name(data, segment.category_id),
    };
    format!(
        "{what} {}-{}",
        clock_text(segment.from_min),
        clock_text(segment.to_min)
    )
}

// This week

fn week_bars(ui: &mut Ui, data: &Data) {
    let p = palette(ui);
    let column = 56.0;
    let bar_width = 22.0;
    let scale_max = data
        .week
        .iter()
        .flat_map(|d| [d.office, d.target.unwrap_or(SignedDuration::ZERO)])
        .max()
        .unwrap_or(SignedDuration::ZERO)
        .max(SignedDuration::from_hours(1));
    let height_of = |d: SignedDuration| WEEK_CHART_HEIGHT * ratio(d, scale_max);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for day in &data.week {
            let weekday = DAY_NAMES[day.date.weekday().to_sunday_zero_offset() as usize];
            let office = hours_text(day.office);
            let name = match day.target {
                Some(_) => format!("{weekday} {office}"),
                None => format!("{weekday} {office} - day off"),
            };
            let (rect, response) =
                ui.allocate_exact_size(vec2(column, WEEK_CHART_HEIGHT + 34.0), Sense::hover());
            response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &name));
            let response = response.on_hover_text(&name);
            let painter = ui.painter();
            let base = rect.top() + WEEK_CHART_HEIGHT;
            let cx = rect.center().x;
            let x = (cx - bar_width / 2.0)..=(cx + bar_width / 2.0);
            painter.line_segment(
                [
                    pos2(rect.left() + 4.0, base),
                    pos2(rect.right() - 4.0, base),
                ],
                Stroke::new(1.0, p.line),
            );
            let total = height_of(day.office);
            let over = height_of(day.overtime).min(total);
            let regular = Rect::from_x_y_ranges(x.clone(), (base - total + over)..=base);
            if regular.height() > 0.0 {
                let fill = if response.hovered() {
                    p.accent.gamma_multiply(0.8)
                } else {
                    p.accent
                };
                painter.rect_filled(regular, bottom_open_rounding(over > 0.0), fill);
            }
            if over > 0.0 {
                // The 2px surface gap separates overtime from the regular part.
                let top =
                    Rect::from_x_y_ranges(x.clone(), (base - total)..=(base - total + over - 2.0));
                painter.rect_filled(top, BAR_ROUNDING, p.danger);
            }
            if let Some(target) = day.target {
                let y = base - height_of(target);
                painter.line_segment(
                    [pos2(cx - 16.0, y), pos2(cx + 16.0, y)],
                    Stroke::new(1.5, p.secondary),
                );
            }
            let label_color = if day.date == data.today {
                p.accent
            } else {
                p.secondary
            };
            painter.text(
                pos2(cx, base + 4.0),
                Align2::CENTER_TOP,
                weekday,
                FontId::proportional(12.0),
                label_color,
            );
            painter.text(
                pos2(cx, base + 19.0),
                Align2::CENTER_TOP,
                &office,
                FontId::proportional(11.0),
                p.secondary,
            );
        }
    });
}

/// Rounded top only when nothing sits on top of the bar.
fn bottom_open_rounding(covered: bool) -> CornerRadius {
    if covered {
        CornerRadius::ZERO
    } else {
        CornerRadius {
            nw: 3,
            ne: 3,
            sw: 0,
            se: 0,
        }
    }
}

fn category_rows(ui: &mut Ui, data: &Data) {
    let p = palette(ui);
    let longest = data
        .week_categories
        .iter()
        .map(|(_, t)| *t)
        .max()
        .unwrap_or(SignedDuration::ZERO);
    for (id, time) in &data.week_categories {
        let target = data.targets.iter().find(|t| t.category_id == *id);
        let name = category_name(data, *id);
        ui.horizontal(|ui| {
            if let Some(c) = data.categories.iter().find(|c| c.meta.id == *id) {
                color_dot(ui, c.color);
            }
            let (share, color, text) = match target {
                Some(t) => {
                    let goal = SignedDuration::from_mins(i64::from(t.minutes));
                    let met = *time >= goal;
                    (
                        ratio(*time, goal),
                        if met {
                            p.success
                        } else {
                            category_color(data, *id, p)
                        },
                        format!("{name} {} / {}", hours_text(*time), target_text(goal)),
                    )
                }
                None => (
                    ratio(*time, longest),
                    category_color(data, *id, p),
                    format!("{name} {}", hours_text(*time)),
                ),
            };
            let (rect, _) = ui.allocate_exact_size(vec2(220.0, 8.0), Sense::hover());
            ui.painter().rect_filled(rect, BAR_ROUNDING, p.hover);
            let done = Rect::from_min_size(rect.min, vec2(rect.width() * share, rect.height()));
            ui.painter().rect_filled(done, BAR_ROUNDING, color);
            ui.label(text);
        });
    }
    if data.week_categories.is_empty() {
        ui.label(RichText::new("Nothing tracked this week yet.").color(p.secondary));
    }
}

fn distraction_rows(ui: &mut Ui, totals: &DistractionTotals, p: Palette) {
    ui.label(RichText::new(format!("Total {}", hours_text(totals.total))).strong());
    let longest = totals
        .rules
        .iter()
        .map(|r| r.time)
        .max()
        .unwrap_or(SignedDuration::ZERO);
    for rule in &totals.rules {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(220.0, 8.0), Sense::hover());
            ui.painter().rect_filled(rect, BAR_ROUNDING, p.hover);
            let done = Rect::from_min_size(
                rect.min,
                vec2(rect.width() * ratio(rule.time, longest), 8.0),
            );
            ui.painter().rect_filled(done, BAR_ROUNDING, p.secondary);
            ui.label(format!("{} {}", rule.name, duration_text(rule.time)));
        });
    }
}

// Helpers

fn category_name(data: &Data, id: Id) -> &str {
    data.categories
        .iter()
        .find(|c| c.meta.id == id)
        .map_or("(deleted category)", |c| c.name.as_str())
}

fn category_color(data: &Data, id: Id, p: Palette) -> Color32 {
    data.categories
        .iter()
        .find(|c| c.meta.id == id)
        .map_or(p.secondary, |c| color32(c.color))
}

/// `part / whole`, clamped to 0..=1.
fn ratio(part: SignedDuration, whole: SignedDuration) -> f32 {
    let whole = whole.as_secs().max(1) as f32;
    (part.as_secs() as f32 / whole).clamp(0.0, 1.0)
}

/// `7h 45m`: always hours and minutes, for totals.
fn hours_text(d: SignedDuration) -> String {
    let m = d.as_mins().max(0);
    format!("{}h {:02}m", m / 60, m % 60)
}

/// `8h`, or `7h 30m` when the target isn't whole hours.
fn target_text(d: SignedDuration) -> String {
    let m = d.as_mins().max(0);
    match m % 60 {
        0 => format!("{}h", m / 60),
        r => format!("{}h {r:02}m", m / 60),
    }
}
