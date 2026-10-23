use std::time::{Duration, Instant};

use egui::{Color32, CornerRadius, Frame, Margin, RichText, ScrollArea, Ui};
use jiff::ToSpan;
use jiff::civil::Date;
use wt_core::repo::RepoResult;
use wt_core::{Clipboard, Clock, ReportInput, Settings, my_time_report, office_report};

use crate::ReportStore;
use crate::theme::palette;
use crate::widgets::{Icon, icon_button, message, pill, screen_title};

/// How long "Copied" shows after a copy.
const COPIED_FOR: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Report {
    Office,
    MyTime,
}

/// The day's office report (for Teams) and the private "My time" report.
#[derive(Default)]
pub struct ReportsScreen {
    /// 0 = today, -1 = yesterday, …
    day_offset: i32,
    copied: Option<(Report, Instant)>,
    message: Option<String>,
}

impl ReportsScreen {
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        store: &dyn ReportStore,
        settings: &Settings,
        clock: &dyn Clock,
        clipboard: &dyn Clipboard,
    ) {
        let p = palette(ui);
        screen_title(ui, Icon::Doc, p.accent, "Reports");

        let today = clock.today();
        let day = today
            .checked_add(i64::from(self.day_offset).days())
            .unwrap_or(today);
        ui.horizontal(|ui| {
            if icon_button(ui, Icon::Back, "Previous day", p.secondary).clicked() {
                self.day_offset -= 1;
            }
            ui.label(RichText::new(day_label(day, today)).strong());
            if self.day_offset < 0
                && icon_button(ui, Icon::Forward, "Next day", p.secondary).clicked()
            {
                self.day_offset += 1;
            }
        });
        ui.add_space(12.0);

        let (office, mine) = match reports(store, settings, clock, day) {
            Ok(reports) => reports,
            Err(e) => {
                message(ui, &Some(format!("Could not build the reports: {e}")));
                return;
            }
        };
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            self.card(
                ui,
                Report::Office,
                "Office report",
                "Copy office report",
                &office,
                clipboard,
            );
            ui.add_space(16.0);
            self.card(
                ui,
                Report::MyTime,
                "My time · private",
                "Copy my time",
                &mine,
                clipboard,
            );
        });
        message(ui, &self.message);
    }

    fn card(
        &mut self,
        ui: &mut Ui,
        which: Report,
        title: &str,
        copy_label: &str,
        text: &str,
        clipboard: &dyn Clipboard,
    ) {
        let p = palette(ui);
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).strong());
            let just_copied = self
                .copied
                .is_some_and(|(w, at)| w == which && at.elapsed() < COPIED_FOR);
            if just_copied {
                ui.label(RichText::new("Copied").color(p.accent));
                ui.ctx().request_repaint_after(COPIED_FOR);
            } else if pill(ui, copy_label, p.accent, Color32::WHITE).clicked() {
                match clipboard.set_text(text) {
                    Ok(()) => {
                        self.copied = Some((which, Instant::now()));
                        self.message = None;
                    }
                    Err(e) => self.message = Some(e.to_string()),
                }
            }
        });
        ui.add_space(6.0);
        Frame::new()
            .fill(p.sidebar)
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::same(14))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(text).monospace());
            });
    }
}

fn reports(
    store: &dyn ReportStore,
    settings: &Settings,
    clock: &dyn Clock,
    day: Date,
) -> RepoResult<(String, String)> {
    let tz = clock.time_zone();
    let now = clock.now();
    // A date outside the time zone's range is not a real report day: show none.
    let day_start = day
        .to_zoned(tz.clone())
        .map(|z| z.timestamp())
        .unwrap_or(now);
    let sessions = store.sessions_since(day_start)?;
    let tasks = store.list_live()?;
    let categories = store.list()?;
    let rules = store.rules()?;
    let distractions = store.distractions_since(day_start)?;
    let input = ReportInput {
        day,
        tz: &tz,
        now,
        sessions: &sessions,
        tasks: &tasks,
        categories: &categories,
        settings,
        rules: &rules,
        distractions: &distractions,
    };
    Ok((office_report(&input), my_time_report(&input)))
}

/// `Today · Thu 22 Oct 2026`, `Yesterday · …`, or just the date further back.
fn day_label(day: Date, today: Date) -> String {
    let date = day.strftime("%a %-d %b %Y").to_string();
    match (today - day).get_days() {
        0 => format!("Today · {date}"),
        1 => format!("Yesterday · {date}"),
        _ => date,
    }
}
