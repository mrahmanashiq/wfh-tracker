//! The Dashboard: today at a glance and this week's progress. It only reads.
//! Charts are painted with egui's painter; every mark is a hoverable widget
//! with an accessible name.

use egui::{
    Align2, Color32, CornerRadius, Rect, RichText, Role, ScrollArea, Sense, Stroke, Ui, WidgetInfo,
    pos2, vec2,
};
use jiff::civil::Date;
use jiff::{SignedDuration, ToSpan};
use wt_core::repo::RepoResult;
use wt_core::{
    Category, CategoryKind, CategoryTime, Clock, DayOffice, DistractionTotals, Id, Period,
    Settings, Target, Task, TimelineSegment, category_totals, clock_text, day_timeline,
    distraction_totals, duration_text, hours_text, is_personal, kind_time, office_by_day,
    target_text, timeline_window,
};

use crate::DashboardStore;
use crate::theme::{Palette, bold, font, palette, radius, space, text};
use crate::widgets::{
    Icon, bar_row, card, color32, day_name, empty_state, fade, message, paint_card, screen_title,
    section_label, target_progress_text,
};

const CARD_HEIGHT: f32 = 88.0;
const GAP: f32 = space::MD;
const STRIP_HEIGHT: f32 = 26.0;
const WEEK_CHART_HEIGHT: f32 = 120.0;
const BAR_ROUNDING: CornerRadius = CornerRadius::same(radius::XS);
/// One day's column in the week chart, and the bar inside it.
const DAY_COLUMN: f32 = 56.0;
const DAY_BAR_WIDTH: f32 = 22.0;

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
    week_categories: Vec<CategoryTime>,
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
            card(ui, |ui| timeline(ui, &data));
            section_label(ui, "This week");
            card(ui, |ui| {
                week_bars(ui, &data);
                ui.add_space(GAP);
                category_rows(ui, &data);
            });
            section_label(ui, "Distractions this week");
            card(ui, |ui| distraction_rows(ui, &data.week_distractions, p));
            ui.add_space(GAP);
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
    let targets: Vec<Target> = store
        .targets()?
        .into_iter()
        .filter(|t| t.period == Period::Week)
        .collect();

    Ok(Data {
        today,
        now_min: (local.hour() as u16) * 60 + local.minute() as u16,
        categories: store.list()?,
        tasks: store.list_live()?,
        office_today: wt_core::office_time(&sessions, today, &tz, now),
        learning_today: kind_time(&sessions, today, &tz, now, |k| k == CategoryKind::Learning),
        personal_today: kind_time(&sessions, today, &tz, now, is_personal),
        distractions_today: distraction_totals(&events, &rules, today, tomorrow, &tz, now).total,
        timeline: day_timeline(&sessions, today, &tz, now),
        week: office_by_day(&sessions, first, &tz, now, settings),
        week_categories: category_totals(&sessions, &targets, first, after, &tz, now),
        targets,
        week_distractions: distraction_totals(&events, &rules, first, after, &tz, now),
    })
}

// Today cards

/// The small text at a card's top right.
struct Note {
    text: String,
    /// Overtime: painted in the danger colour.
    danger: bool,
}

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
        // On a day off all office time is overtime.
        let (value, note) = if off_day {
            let note = Note {
                text: "day off".to_owned(),
                danger: overtime.is_positive(),
            };
            (office, Some(note))
        } else {
            let note = (overtime.as_mins() > 0).then(|| Note {
                text: format!("{} overtime", duration_text(overtime)),
                danger: true,
            });
            (
                format!("{office} / {}", target_text(settings.office_target_min)),
                note,
            )
        };
        let rect = card_tile(ui, width, "Office", &value, note.as_ref());
        // The bar fills to the target; on a day off it shows only overtime.
        let fill = if off_day {
            overtime.is_positive().then_some((1.0, p.danger))
        } else if overtime.is_positive() {
            Some((ratio(data.office_today, target), p.danger))
        } else if data.office_today >= target {
            Some((1.0, p.success))
        } else {
            Some((ratio(data.office_today, target), p.accent))
        };
        if let Some((share, color)) = fill {
            crate::widgets::paint_bar(ui.painter(), bar_rect(rect), share, color, p.hover);
        }
        for (title, time) in [
            ("Learning", data.learning_today),
            ("Personal", data.personal_today),
            ("Distractions", data.distractions_today),
        ] {
            card_tile(ui, width, title, &hours_text(time), None);
        }
    });
}

/// A painted card: a small title, a large value and an optional note. Its
/// accessible name is the painted text: `Office 9h 30m / 8h - 30m overtime`.
fn card_tile(ui: &mut Ui, width: f32, title: &str, value: &str, note: Option<&Note>) -> Rect {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(width, CARD_HEIGHT), Sense::hover());
    let name = match note {
        Some(note) => format!("{title} {value} - {}", note.text),
        None => format!("{title} {value}"),
    };
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &name));
    let painter = ui.painter();
    paint_card(painter, rect, &p);
    let left = rect.left() + space::LG;
    let top = rect.top() + space::MD + 2.0;
    painter.text(
        pos2(left, top),
        Align2::LEFT_TOP,
        title,
        font(text::SMALL),
        p.secondary,
    );
    painter.text(
        pos2(left, top + space::LG + 2.0),
        Align2::LEFT_TOP,
        value,
        bold(ui, text::VALUE),
        p.text,
    );
    if let Some(note) = note {
        painter.text(
            pos2(rect.right() - space::LG, top),
            Align2::RIGHT_TOP,
            &note.text,
            font(text::SMALL),
            if note.danger { p.danger } else { p.secondary },
        );
    }
    rect
}

/// The office card's progress bar, along its bottom edge.
fn bar_rect(card: Rect) -> Rect {
    Rect::from_min_max(
        pos2(card.left() + space::LG, card.bottom() - space::LG - 2.0),
        pos2(card.right() - space::LG, card.bottom() - space::MD + 2.0),
    )
}

// Today timeline

fn timeline(ui: &mut Ui, data: &Data) {
    let p = palette(ui);
    let (from, to) = timeline_window(&data.timeline);
    let span = f32::from(to - from);
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), STRIP_HEIGHT + space::LG + 2.0),
        Sense::hover(),
    );
    let strip = Rect::from_min_size(rect.min, vec2(rect.width(), STRIP_HEIGHT));
    let x_at =
        |min: u16| strip.left() + strip.width() * f32::from(min.clamp(from, to) - from) / span;
    ui.painter()
        .rect_filled(strip, CornerRadius::same(radius::SM), p.hover);

    // Hour ticks under the strip, labelled every hour or every other hour.
    let step = if to - from > 12 * 60 { 120 } else { 60 };
    for hour in (from..=to).step_by(60) {
        let x = x_at(hour);
        ui.painter().line_segment(
            [pos2(x, strip.bottom()), pos2(x, strip.bottom() + space::XS)],
            Stroke::new(1.0, p.line),
        );
        if (hour - from) % step == 0 {
            let align = match hour {
                h if h == from => Align2::LEFT_TOP,
                h if h == to => Align2::RIGHT_TOP,
                _ => Align2::CENTER_TOP,
            };
            ui.painter().text(
                pos2(x, strip.bottom() + space::XS + 1.0),
                align,
                clock_text(hour),
                font(text::CAPTION),
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
        let t = fade(ui, response.id, response.hovered());
        let color = color.gamma_multiply(1.0 - 0.2 * t);
        ui.painter()
            .rect_filled(mark, CornerRadius::same(radius::XS + 1), color);
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
    let scale_max = data
        .week
        .iter()
        .flat_map(|d| [d.office, d.target.unwrap_or(SignedDuration::ZERO)])
        .max()
        .unwrap_or(SignedDuration::ZERO)
        .max(SignedDuration::from_hours(1));
    let height_of = |d: SignedDuration| WEEK_CHART_HEIGHT * ratio(d, scale_max);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = space::XS;
        for day in &data.week {
            let weekday = day_name(day.date.weekday());
            let office = hours_text(day.office);
            let name = match day.target {
                Some(_) => format!("{weekday} {office}"),
                None => format!("{weekday} {office} - day off"),
            };
            let (rect, response) =
                ui.allocate_exact_size(vec2(DAY_COLUMN, WEEK_CHART_HEIGHT + 48.0), Sense::hover());
            response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &name));
            let response = response.on_hover_text(&name);
            let hover = fade(ui, response.id, response.hovered());
            let painter = ui.painter();
            let base = rect.top() + WEEK_CHART_HEIGHT;
            let cx = rect.center().x;
            let x = (cx - DAY_BAR_WIDTH / 2.0)..=(cx + DAY_BAR_WIDTH / 2.0);
            painter.line_segment(
                [
                    pos2(rect.left() + space::XS, base),
                    pos2(rect.right() - space::XS, base),
                ],
                Stroke::new(1.0, p.hairline),
            );
            let total = height_of(day.office);
            let over = height_of(day.overtime).min(total);
            let regular = Rect::from_x_y_ranges(x.clone(), (base - total + over)..=base);
            if regular.height() > 0.0 {
                let fill = p.accent.gamma_multiply(1.0 - 0.2 * hover);
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
                    [pos2(cx - space::LG, y), pos2(cx + space::LG, y)],
                    Stroke::new(1.5, p.line),
                );
            }
            let is_today = day.date == data.today;
            painter.text(
                pos2(cx, base + space::XS),
                Align2::CENTER_TOP,
                weekday,
                if is_today {
                    bold(ui, text::SMALL)
                } else {
                    font(text::SMALL)
                },
                if is_today { p.accent } else { p.secondary },
            );
            painter.text(
                pos2(cx, base + 19.0),
                Align2::CENTER_TOP,
                &office,
                font(text::CAPTION),
                p.secondary,
            );
            if day.target.is_none() {
                painter.text(
                    pos2(cx, base + 33.0),
                    Align2::CENTER_TOP,
                    "day off",
                    font(text::CAPTION),
                    p.secondary,
                );
            }
        }
    });
}

/// Rounded top only when nothing sits on top of the bar.
fn bottom_open_rounding(covered: bool) -> CornerRadius {
    if covered {
        CornerRadius::ZERO
    } else {
        CornerRadius {
            nw: radius::XS,
            ne: radius::XS,
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
        .map(|t| t.time)
        .max()
        .unwrap_or(SignedDuration::ZERO);
    for &CategoryTime {
        category_id: id,
        time,
    } in &data.week_categories
    {
        let target = data.targets.iter().find(|t| t.category_id == id);
        let dot = data
            .categories
            .iter()
            .find(|c| c.meta.id == id)
            .map(|c| color32(c.color));
        let (share, color, value) = match target {
            Some(t) => {
                let goal = SignedDuration::from_mins(i64::from(t.minutes));
                let met = time >= goal;
                (
                    ratio(time, goal),
                    if met {
                        p.success
                    } else {
                        category_color(data, id, p)
                    },
                    target_progress_text(time, t.minutes),
                )
            }
            None => (
                ratio(time, longest),
                category_color(data, id, p),
                hours_text(time),
            ),
        };
        bar_row(ui, dot, category_name(data, id), share, color, &value);
    }
    if data.week_categories.is_empty() {
        empty_state(
            ui,
            Icon::Chart,
            "Nothing tracked this week yet.",
            Some("Start a timer and your week fills in here."),
        );
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
        bar_row(
            ui,
            None,
            &rule.name,
            ratio(rule.time, longest),
            p.secondary,
            &duration_text(rule.time),
        );
    }
    if totals.rules.is_empty() {
        ui.label(RichText::new("No distractions this week.").color(p.secondary));
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
