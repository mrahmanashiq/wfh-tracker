use std::collections::HashMap;

use egui::{ComboBox, RichText, TextEdit, Ui};
use wt_core::repo::RepoResult;
use wt_core::{Category, CategoryRepo, Clock, Id, Placement, Task, TaskRepo, today_view};

use crate::widgets::{DeleteClick, color_dot, delete_buttons, message};

/// Today's tasks: open ones carried over plus anything finished today.
#[derive(Default)]
pub struct TasksScreen {
    new_title: String,
    new_category: Option<Id>,
    /// Top-level task whose "add sub-task" field is open, with its text.
    adding_under: Option<(Id, String)>,
    /// Task being renamed, with the edited title and (top-level only) category.
    renaming: Option<(Id, String, Option<Id>)>,
    confirm_delete: Option<Id>,
    message: Option<String>,
}

/// What the user asked for this frame; applied after drawing.
enum Action {
    Add,
    AddSubtask,
    Toggle(Id, bool),
    OpenSubtask(Id),
    OpenRename(Id, String, Option<Id>),
    Rename,
    Delete(Id),
    ArmDelete(Option<Id>),
    Close,
}

impl TasksScreen {
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        tasks: &dyn TaskRepo,
        categories: &dyn CategoryRepo,
        clock: &dyn Clock,
    ) {
        ui.heading("Tasks");
        ui.add_space(8.0);

        let loaded = categories.list().and_then(|c| Ok((c, tasks.list_live()?)));
        let (cats, all) = match loaded {
            Ok(loaded) => loaded,
            Err(e) => {
                message(ui, &Some(format!("Could not load tasks: {e}")));
                return;
            }
        };
        let by_id: HashMap<Id, &Category> = cats.iter().map(|c| (c.meta.id, c)).collect();
        if self.new_category.is_none_or(|id| !by_id.contains_key(&id)) {
            self.new_category = cats.first().map(|c| c.meta.id);
        }

        let mut action = None;
        ui.horizontal(|ui| {
            let field = ui.add(
                TextEdit::singleline(&mut self.new_title)
                    .hint_text("What needs doing?")
                    .desired_width(260.0),
            );
            let field = field.accessible_name("New task title");
            category_picker(ui, "new_task_category", &cats, &mut self.new_category);
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Add task").clicked() || enter {
                action = Some(Action::Add);
            }
        });
        message(ui, &self.message);
        ui.separator();

        let view = today_view(&all, clock.today(), &clock.time_zone());
        if view.is_empty() {
            ui.weak("Nothing on your list. Add a task above.");
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for tree in view {
                self.task_row(ui, &tree.task, tree.progress, &by_id, &cats, &mut action);
                ui.indent(tree.task.meta.id, |ui| {
                    for sub in &tree.subtasks {
                        self.task_row(ui, sub, None, &by_id, &cats, &mut action);
                    }
                    self.subtask_field(ui, tree.task.meta.id, &mut action);
                });
            }
        });

        if let Some(action) = action {
            self.apply(action, tasks);
        }
    }

    fn task_row(
        &mut self,
        ui: &mut Ui,
        task: &Task,
        progress: Option<(usize, usize)>,
        by_id: &HashMap<Id, &Category>,
        cats: &[Category],
        action: &mut Option<Action>,
    ) {
        let id = task.meta.id;
        let top_level = task.parent_id.is_none();
        ui.horizontal(|ui| {
            if let Some(cat) = by_id.get(&task.category_id) {
                color_dot(ui, cat.color);
            }

            if let Some((_, title, category)) = self.renaming.as_mut().filter(|r| r.0 == id) {
                ui.add(TextEdit::singleline(title).desired_width(220.0))
                    .accessible_name("Task title");
                if top_level {
                    category_picker(ui, ("rename_category", id), cats, category);
                }
                if ui.button("Save").clicked() {
                    *action = Some(Action::Rename);
                }
                if ui.button("Cancel").clicked() {
                    *action = Some(Action::Close);
                }
                return;
            }

            let mut done = task.is_done();
            let title = RichText::new(&task.title);
            let title = if done {
                title.strikethrough().weak()
            } else {
                title
            };
            if ui.checkbox(&mut done, title).changed() {
                *action = Some(Action::Toggle(id, done));
            }
            if let Some((finished, total)) = progress {
                ui.weak(format!("{finished}/{total}"));
            }
            if top_level && let Some(cat) = by_id.get(&task.category_id) {
                ui.weak(&cat.name);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                match delete_buttons(ui, self.confirm_delete == Some(id)) {
                    Some(DeleteClick::Arm) => *action = Some(Action::ArmDelete(Some(id))),
                    Some(DeleteClick::Confirm) => *action = Some(Action::Delete(id)),
                    Some(DeleteClick::Cancel) => *action = Some(Action::ArmDelete(None)),
                    None => {}
                }
                if ui.button("Rename").clicked() {
                    let category = top_level.then_some(task.category_id);
                    *action = Some(Action::OpenRename(id, task.title.clone(), category));
                }
                if top_level && ui.button("Add sub-task").clicked() {
                    *action = Some(Action::OpenSubtask(id));
                }
            });
        });
    }

    fn subtask_field(&mut self, ui: &mut Ui, parent: Id, action: &mut Option<Action>) {
        let Some((_, text)) = self.adding_under.as_mut().filter(|a| a.0 == parent) else {
            return;
        };
        ui.horizontal(|ui| {
            let field = ui
                .add(TextEdit::singleline(text).hint_text("Sub-task"))
                .accessible_name("Sub-task title");
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Add").clicked() || enter {
                *action = Some(Action::AddSubtask);
            }
            if ui.button("Cancel").clicked() {
                *action = Some(Action::Close);
            }
        });
    }

    fn apply(&mut self, action: Action, tasks: &dyn TaskRepo) {
        let result: RepoResult<()> = match action {
            Action::Add => match self.new_category {
                Some(category_id) => tasks
                    .add(&self.new_title, Placement::TopLevel { category_id })
                    .map(|_| self.new_title.clear()),
                None => {
                    self.message = Some("Add a category first".to_owned());
                    return;
                }
            },
            Action::AddSubtask => match self.adding_under.take() {
                Some((parent, title)) => tasks.add(&title, Placement::Under(parent)).map(|_| ()),
                None => Ok(()),
            },
            Action::Toggle(id, done) => tasks.set_done(id, done),
            Action::OpenSubtask(id) => {
                self.adding_under = Some((id, String::new()));
                Ok(())
            }
            Action::OpenRename(id, title, category) => {
                self.renaming = Some((id, title, category));
                Ok(())
            }
            Action::Rename => match self.renaming.take() {
                Some((id, title, category)) => {
                    tasks.rename(id, &title).and_then(|()| match category {
                        Some(category_id) if tasks.get(id)?.category_id != category_id => {
                            tasks.set_category(id, category_id)
                        }
                        _ => Ok(()),
                    })
                }
                None => Ok(()),
            },
            Action::Delete(id) => {
                self.confirm_delete = None;
                tasks.delete(id)
            }
            Action::ArmDelete(id) => {
                self.confirm_delete = id;
                Ok(())
            }
            Action::Close => {
                self.adding_under = None;
                self.renaming = None;
                Ok(())
            }
        };
        self.message = result.err().map(|e| e.to_string());
    }
}

/// A category drop-down; `selected` falls back to nothing if the list is empty.
fn category_picker(
    ui: &mut Ui,
    id_salt: impl egui::AsIdSalt,
    cats: &[Category],
    selected: &mut Option<Id>,
) {
    let current = cats
        .iter()
        .find(|c| Some(c.meta.id) == *selected)
        .map_or("—", |c| c.name.as_str());
    ComboBox::from_id_salt(id_salt)
        .selected_text(current)
        .show_ui(ui, |ui| {
            for c in cats {
                ui.selectable_value(selected, Some(c.meta.id), &c.name);
            }
        })
        .response
        .accessible_name("Category");
}
