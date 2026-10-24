use egui::{ComboBox, DragValue, Grid, Ui};
use jiff::civil::Weekday;
use wt_core::{Appearance, Autostart, Settings, SettingsRepo};

const DAYS: [(Weekday, &str); 7] = [
    (Weekday::Sunday, "Sun"),
    (Weekday::Monday, "Mon"),
    (Weekday::Tuesday, "Tue"),
    (Weekday::Wednesday, "Wed"),
    (Weekday::Thursday, "Thu"),
    (Weekday::Friday, "Fri"),
    (Weekday::Saturday, "Sat"),
];

/// Edits a draft of [`Settings`]; nothing is stored until Save.
pub struct SettingsScreen {
    draft: Settings,
    status: Option<Result<(), String>>,
}

impl SettingsScreen {
    pub fn load(repo: &dyn SettingsRepo) -> Self {
        match repo.load() {
            Ok(draft) => Self {
                draft,
                status: None,
            },
            Err(e) => Self {
                draft: Settings::default(),
                status: Some(Err(format!("Could not load settings: {e}"))),
            },
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, repo: &dyn SettingsRepo, autostart: &dyn Autostart) {
        let p = crate::theme::palette(ui);
        crate::widgets::screen_title(ui, crate::widgets::Icon::Gear, p.secondary, "Settings");

        let d = &mut self.draft;
        crate::widgets::section_label(ui, "Day and breaks");
        crate::widgets::card(ui, |ui| {
            Grid::new("settings")
                .num_columns(2)
                .min_col_width(crate::theme::size::LABEL_COLUMN)
                .spacing([crate::theme::space::XL, crate::theme::space::MD])
                .show(ui, |ui| {
                    number_row(
                        ui,
                        "Office target (min)",
                        &mut d.office_target_min,
                        60..=16 * 60,
                    );
                    number_row(
                        ui,
                        "Soft ceiling (min)",
                        &mut d.office_ceiling_min,
                        60..=18 * 60,
                    );
                    number_row(ui, "Break every (min)", &mut d.break_interval_min, 1..=180);
                    number_row(ui, "Break length (min)", &mut d.break_length_min, 1..=60);
                    number_row(
                        ui,
                        "Nudge after (sec)",
                        &mut d.distraction_grace_secs,
                        5..=600,
                    );
                    number_row(
                        ui,
                        "Nudge cooldown (sec)",
                        &mut d.nudge_cooldown_secs,
                        10..=3600,
                    );

                    let label = ui.label("Week starts on");
                    ComboBox::from_id_salt("week_start")
                        .selected_text(day_name(d.week_start))
                        .show_ui(ui, |ui| {
                            for (day, name) in DAYS {
                                ui.selectable_value(&mut d.week_start, day, name);
                            }
                        })
                        .response
                        .labelled_by(label.id);
                    ui.end_row();

                    ui.label("Workdays");
                    ui.horizontal(|ui| {
                        for (day, name) in DAYS {
                            let mut on = d.workdays.contains(&day);
                            if ui.checkbox(&mut on, name).changed() {
                                d.workdays = DAYS
                                    .iter()
                                    .map(|(day, _)| *day)
                                    .filter(|x| {
                                        if *x == day {
                                            on
                                        } else {
                                            d.workdays.contains(x)
                                        }
                                    })
                                    .collect();
                            }
                        }
                    });
                    ui.end_row();

                    ui.label("");
                    ui.checkbox(&mut d.autostart, "Start at login");
                    ui.end_row();

                    ui.label("Appearance");
                    ui.horizontal(|ui| {
                        for choice in Appearance::ALL {
                            ui.selectable_value(&mut d.appearance, choice, appearance_name(choice));
                        }
                    });
                    ui.end_row();
                });
        });

        ui.add_space(crate::theme::space::LG);
        if crate::widgets::pill(ui, "Save", p.accent, egui::Color32::WHITE).clicked() {
            self.status = Some(self.save(repo, autostart));
        }
        match &self.status {
            Some(Ok(())) => {
                ui.label("Saved.");
            }
            Some(Err(e)) => {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            None => {}
        }
    }

    fn save(&self, repo: &dyn SettingsRepo, autostart: &dyn Autostart) -> Result<(), String> {
        repo.save(&self.draft).map_err(|e| e.to_string())?;
        autostart
            .set_enabled(self.draft.autostart)
            .map_err(|e| format!("Settings saved, but start at login could not be changed: {e}"))
    }
}

fn number_row(ui: &mut Ui, name: &str, value: &mut u32, range: std::ops::RangeInclusive<u32>) {
    let label = ui.label(name);
    ui.add(DragValue::new(value).range(range))
        .labelled_by(label.id);
    ui.end_row();
}

fn day_name(day: Weekday) -> &'static str {
    DAYS.iter()
        .find(|(d, _)| *d == day)
        .map_or("Sun", |(_, n)| n)
}

fn appearance_name(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::Light => "Light",
        Appearance::Dark => "Dark",
        Appearance::System => "System",
    }
}
