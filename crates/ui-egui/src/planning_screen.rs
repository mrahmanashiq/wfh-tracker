use egui::{ComboBox, CornerRadius, DragValue, RichText, Sense, TextEdit, Ui, vec2};
use jiff::civil::Weekday;
use wt_core::repo::RepoResult;
use wt_core::{
    BlockDraft, Category, CategoryKind, Clock, Id, Period, Settings, Target, Weekdays, clock_text,
    target_progress,
};

use crate::PlanStore;
use crate::theme::palette;
use crate::widgets::{
    DeleteClick, Icon, color_dot, color32, delete_buttons, icon_button, message, pill,
    screen_title, section_label,
};

const DAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// Targets ("Learning: 5h this week") and scheduled blocks.
#[derive(Default)]
pub struct PlanningScreen {
    target_category: Option<Id>,
    target_period: Option<Period>,
    target_hours: u32,
    /// The target being edited, if any.
    editing_target: Option<Target>,
    /// The block form; `editing` is the block being changed, if any.
    block: Option<BlockDraft>,
    editing: Option<Id>,
    confirm_delete: Option<Id>,
    message: Option<String>,
}

enum Action {
    SetTarget,
    EditTarget(Target),
    DeleteTarget(Id),
    SaveBlock,
    EditBlock(Id, BlockDraft),
    DeleteBlock(Id),
    ArmDelete(Option<Id>),
}

impl PlanningScreen {
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        store: &dyn PlanStore,
        settings: &Settings,
        clock: &dyn Clock,
    ) {
        let p = palette(ui);
        screen_title(ui, Icon::Target, p.accent, "Planning");
        let loaded =
            (|| -> RepoResult<_> { Ok((store.list()?, store.targets()?, store.blocks()?)) })();
        let (cats, targets, blocks) = match loaded {
            Ok(loaded) => loaded,
            Err(e) => {
                message(ui, &Some(format!("Could not load your plans: {e}")));
                return;
            }
        };
        let default_category = cats
            .iter()
            .find(|c| c.kind == CategoryKind::Learning)
            .or(cats.first())
            .map(|c| c.meta.id);
        let name_of = |id: Id| {
            cats.iter()
                .find(|c| c.meta.id == id)
                .map_or("(deleted category)", |c| c.name.as_str())
        };
        let mut action = None;

        // Targets
        section_label(ui, "Targets");
        let now = clock.now();
        let today = clock.today();
        let sessions = store
            .sessions_since(
                now.checked_sub(jiff::SignedDuration::from_hours(24 * 32))
                    .unwrap_or(now),
            )
            .unwrap_or_default();
        for target in &targets {
            let done = target_progress(
                &sessions,
                target,
                today,
                &clock.time_zone(),
                now,
                settings.week_start,
            );
            ui.horizontal(|ui| {
                if let Some(cat) = cats.iter().find(|c| c.meta.id == target.category_id) {
                    color_dot(ui, cat.color);
                }
                let period = match target.period {
                    Period::Week => "this week",
                    Period::Month => "this month",
                };
                ui.label(format!("{} · {period}", name_of(target.category_id)));
                let goal = jiff::SignedDuration::from_mins(i64::from(target.minutes));
                let met = done >= goal;
                progress_bar(
                    ui,
                    done.as_mins() as f32 / goal.as_mins().max(1) as f32,
                    if met { p.success } else { p.accent },
                );
                ui.label(
                    RichText::new(format!(
                        "{} / {}h",
                        hours_minutes(done),
                        target.minutes / 60
                    ))
                    .color(if met { p.success } else { p.secondary }),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let id = target.meta.id;
                    self.row_actions(
                        ui,
                        id,
                        &mut action,
                        || Action::EditTarget(target.clone()),
                        Action::DeleteTarget(id),
                    );
                });
            });
        }
        ui.horizontal(|ui| {
            let category = self
                .target_category
                .get_or_insert_with(|| default_category.unwrap_or_default());
            category_picker(ui, "target_category", &cats, category);
            let period = self.target_period.get_or_insert(Period::Week);
            ComboBox::from_id_salt("target_period")
                .selected_text(period_label(*period))
                .show_ui(ui, |ui| {
                    for choice in [Period::Week, Period::Month] {
                        ui.selectable_value(period, choice, period_label(choice));
                    }
                })
                .response
                .accessible_name("Period");
            if self.target_hours == 0 {
                self.target_hours = 5;
            }
            ui.add(
                DragValue::new(&mut self.target_hours)
                    .range(1..=period.max_hours())
                    .suffix(" h"),
            )
            .accessible_name("Target hours");
            let label = if self.editing_target.is_some() {
                "Save target"
            } else {
                "Add target"
            };
            if pill(ui, label, p.accent, egui::Color32::WHITE).clicked() {
                action = Some(Action::SetTarget);
            }
        });

        // Blocks
        section_label(ui, "Scheduled blocks");
        for block in &blocks {
            ui.horizontal(|ui| {
                if let Some(cat) = cats.iter().find(|c| c.meta.id == block.category_id) {
                    color_dot(ui, cat.color);
                }
                let title = if block.label.is_empty() {
                    name_of(block.category_id)
                } else {
                    &block.label
                };
                ui.label(title);
                let days: Vec<&str> = block.weekdays.days().map(day_name).collect();
                ui.label(
                    RichText::new(format!("{} · {}", days.join(" "), block.times_text()))
                        .color(p.secondary),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let draft = BlockDraft {
                        category_id: block.category_id,
                        weekdays: block.weekdays,
                        start_min: block.start_min,
                        end_min: block.end_min,
                        label: block.label.clone(),
                    };
                    let id = block.meta.id;
                    self.row_actions(
                        ui,
                        id,
                        &mut action,
                        || Action::EditBlock(id, draft),
                        Action::DeleteBlock(id),
                    );
                });
            });
        }
        let draft = self.block.get_or_insert_with(|| BlockDraft {
            category_id: default_category.unwrap_or_default(),
            weekdays: Weekdays::from_days(&settings.workdays),
            start_min: 21 * 60,
            end_min: 22 * 60,
            label: String::new(),
        });
        ui.horizontal_wrapped(|ui| {
            category_picker(ui, "block_category", &cats, &mut draft.category_id);
            for day in Weekdays::ALL_IN_ORDER {
                let on = draft.weekdays.contains(day);
                if ui.selectable_label(on, day_name(day)).clicked() {
                    draft.weekdays = draft.weekdays.toggled(day);
                }
            }
            time_field(ui, &mut draft.start_min, "Start");
            ui.label("–");
            time_field(ui, &mut draft.end_min, "End");
            ui.add(
                TextEdit::singleline(&mut draft.label)
                    .hint_text("Label (optional)")
                    .desired_width(140.0),
            )
            .accessible_name("Block label");
            let label = if self.editing.is_some() {
                "Save block"
            } else {
                "Add block"
            };
            if pill(ui, label, p.accent, egui::Color32::WHITE).clicked() {
                action = Some(Action::SaveBlock);
            }
        });
        message(ui, &self.message);

        if let Some(action) = action {
            self.apply(action, store);
        }
    }

    /// Edit and delete icons, with the two-step delete.
    fn row_actions(
        &self,
        ui: &mut Ui,
        id: Id,
        action: &mut Option<Action>,
        edit: impl FnOnce() -> Action,
        delete: Action,
    ) {
        let p = palette(ui);
        match delete_buttons(ui, self.confirm_delete == Some(id)) {
            Some(DeleteClick::Arm) => *action = Some(Action::ArmDelete(Some(id))),
            Some(DeleteClick::Confirm) => *action = Some(delete),
            Some(DeleteClick::Cancel) => *action = Some(Action::ArmDelete(None)),
            None => {}
        }
        if self.confirm_delete != Some(id)
            && icon_button(ui, Icon::Pencil, "Edit", p.secondary).clicked()
        {
            *action = Some(edit());
        }
    }

    fn apply(&mut self, action: Action, plans: &dyn PlanStore) {
        let result: RepoResult<()> = match action {
            Action::SetTarget => match (self.target_category, self.target_period) {
                (Some(category), Some(period)) => self.save_target(plans, category, period),
                _ => Ok(()),
            },
            Action::EditTarget(target) => {
                self.target_category = Some(target.category_id);
                self.target_period = Some(target.period);
                self.target_hours = target.minutes / 60;
                self.editing_target = Some(target);
                Ok(())
            }
            Action::DeleteTarget(id) => {
                self.confirm_delete = None;
                plans.delete_target(id)
            }
            Action::SaveBlock => match (self.block.clone(), self.editing) {
                (Some(draft), Some(id)) => {
                    plans.update_block(id, &draft).map(|_| self.reset_block())
                }
                (Some(draft), None) => plans.add_block(&draft).map(|_| self.reset_block()),
                (None, _) => Ok(()),
            },
            Action::EditBlock(id, draft) => {
                self.editing = Some(id);
                self.block = Some(draft);
                Ok(())
            }
            Action::DeleteBlock(id) => {
                self.confirm_delete = None;
                plans.delete_block(id)
            }
            Action::ArmDelete(id) => {
                self.confirm_delete = id;
                Ok(())
            }
        };
        self.message = result.err().map(|e| e.to_string());
    }

    /// Saves the target form. When editing moves a target to another category
    /// or period, the old one is removed so no duplicate is left behind.
    fn save_target(
        &mut self,
        plans: &dyn PlanStore,
        category: Id,
        period: Period,
    ) -> RepoResult<()> {
        plans.set_target(category, period, self.target_hours)?;
        if let Some(old) = self.editing_target.take()
            && (old.category_id, old.period) != (category, period)
        {
            plans.delete_target(old.meta.id)?;
        }
        Ok(())
    }

    fn reset_block(&mut self) {
        self.block = None;
        self.editing = None;
    }
}

fn period_label(period: Period) -> &'static str {
    match period {
        Period::Week => "per week",
        Period::Month => "per month",
    }
}

fn day_name(day: Weekday) -> &'static str {
    DAY_NAMES[day.to_sunday_zero_offset() as usize]
}

/// `3h 20m`
fn hours_minutes(d: jiff::SignedDuration) -> String {
    let m = d.as_mins().max(0);
    format!("{}h {:02}m", m / 60, m % 60)
}

/// A time of day as `HH:MM`, dragged in 15-minute steps or typed.
fn time_field(ui: &mut Ui, minutes: &mut u16, name: &str) {
    ui.add(
        DragValue::new(minutes)
            .range(0..=24 * 60)
            .speed(0.25)
            .custom_formatter(|v, _| clock_text(v as u16))
            .custom_parser(|text| {
                let (h, m) = text.trim().split_once(':')?;
                Some(f64::from(
                    h.parse::<u16>().ok()? * 60 + m.parse::<u16>().ok()?,
                ))
            }),
    )
    .accessible_name(name);
}

fn progress_bar(ui: &mut Ui, share: f32, color: egui::Color32) {
    let p = palette(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(120.0, 6.0), Sense::hover());
    let rounding = CornerRadius::same(3);
    ui.painter().rect_filled(rect, rounding, p.hover);
    let mut done = rect;
    done.set_width(rect.width() * share.clamp(0.0, 1.0));
    ui.painter().rect_filled(done, rounding, color);
}

fn category_picker(ui: &mut Ui, id_salt: &str, cats: &[Category], selected: &mut Id) {
    let current = cats.iter().find(|c| c.meta.id == *selected);
    let text = current.map_or_else(
        || RichText::new("—"),
        |c| RichText::new(&c.name).color(color32(c.color)),
    );
    ComboBox::from_id_salt(id_salt)
        .selected_text(text)
        .width(120.0)
        .show_ui(ui, |ui| {
            for c in cats {
                ui.selectable_value(selected, c.meta.id, &c.name);
            }
        })
        .response
        .accessible_name("Category");
}
