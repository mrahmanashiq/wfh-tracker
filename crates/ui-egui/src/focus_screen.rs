use egui::{ComboBox, RichText, TextEdit, Ui};
use wt_core::repo::RepoResult;
use wt_core::{DistractionRuleRepo, Id, MatchOn, RuleDraft};

use crate::theme::palette;
use crate::widgets::{
    DeleteClick, Icon, delete_buttons, message, pill, screen_title, section_label,
};

/// Distraction rules: what counts as Facebook or reels during protected time.
pub struct FocusScreen {
    draft: RuleDraft,
    confirm_delete: Option<Id>,
    message: Option<String>,
}

impl Default for FocusScreen {
    fn default() -> Self {
        Self {
            draft: RuleDraft::new("", MatchOn::Title),
            confirm_delete: None,
            message: None,
        }
    }
}

enum Action {
    Add,
    SetEnabled(Id, bool),
    Delete(Id),
    ArmDelete(Option<Id>),
}

impl FocusScreen {
    /// `poll_error` is the last failure of the background distraction check.
    pub fn ui(&mut self, ui: &mut Ui, store: &dyn DistractionRuleRepo, poll_error: Option<&str>) {
        let p = palette(ui);
        screen_title(ui, Icon::Focus, p.accent, "Focus");
        ui.label(
            RichText::new(
                "During a learning block or a learning, office or meeting timer, a window \
                 matching a rule for a while gets you a nudge. Only the rule and the time \
                 are saved, never the window's title.",
            )
            .color(p.secondary),
        );
        if let Some(e) = poll_error {
            message(ui, &Some(format!("Could not check for distractions: {e}")));
        }
        let rules = match store.rules() {
            Ok(rules) => rules,
            Err(e) => {
                message(ui, &Some(format!("Could not load your rules: {e}")));
                return;
            }
        };

        let mut action = None;
        section_label(ui, "Distraction rules");
        for rule in &rules {
            ui.horizontal(|ui| {
                let mut on = rule.enabled;
                if ui.checkbox(&mut on, &rule.pattern).changed() {
                    action = Some(Action::SetEnabled(rule.meta.id, on));
                }
                ui.label(RichText::new(where_text(rule.match_on)).color(p.secondary));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let id = rule.meta.id;
                    match delete_buttons(ui, self.confirm_delete == Some(id)) {
                        Some(DeleteClick::Arm) => action = Some(Action::ArmDelete(Some(id))),
                        Some(DeleteClick::Confirm) => action = Some(Action::Delete(id)),
                        Some(DeleteClick::Cancel) => action = Some(Action::ArmDelete(None)),
                        None => {}
                    }
                });
            });
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add(
                TextEdit::singleline(&mut self.draft.pattern)
                    .hint_text("e.g. Twitter")
                    .desired_width(180.0),
            )
            .accessible_name("Rule text");
            let match_on = &mut self.draft.match_on;
            ComboBox::from_id_salt("rule_match_on")
                .selected_text(choice_text(*match_on))
                .show_ui(ui, |ui| {
                    for choice in MatchOn::ALL {
                        ui.selectable_value(match_on, choice, choice_text(choice));
                    }
                })
                .response
                .accessible_name("Match on");
            if pill(ui, "Add rule", p.accent, egui::Color32::WHITE).clicked() {
                action = Some(Action::Add);
            }
        });
        message(ui, &self.message);

        if let Some(action) = action {
            self.apply(action, store);
        }
    }

    fn apply(&mut self, action: Action, store: &dyn DistractionRuleRepo) {
        let result: RepoResult<()> = match action {
            Action::Add => store.add_rule(&self.draft).map(|_| {
                self.draft.pattern.clear();
            }),
            Action::SetEnabled(id, on) => store.set_rule_enabled(id, on),
            Action::Delete(id) => {
                self.confirm_delete = None;
                store.delete_rule(id)
            }
            Action::ArmDelete(id) => {
                self.confirm_delete = id;
                Ok(())
            }
        };
        self.message = result.err().map(|e| e.to_string());
    }
}

fn where_text(match_on: MatchOn) -> &'static str {
    match match_on {
        MatchOn::Title => "in the window title",
        MatchOn::Process => "in the process name",
    }
}

fn choice_text(match_on: MatchOn) -> &'static str {
    match match_on {
        MatchOn::Title => "Window title",
        MatchOn::Process => "Process name",
    }
}
