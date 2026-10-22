use std::collections::HashMap;

use egui::{ComboBox, DragValue, RichText, TextEdit, Ui};
use wt_core::repo::RepoResult;
use wt_core::{
    Category, CategoryRepo, Clock, Id, Placement, SessionRepo, Task, TaskRepo, TimerTarget,
    today_view,
};

use crate::widgets::{DeleteClick, color_dot, delete_buttons, message};

/// Today's tasks: open ones carried over plus anything finished today.
#[derive(Default)]
pub struct TasksScreen {
    new_title: String,
    new_category: Option<Id>,
    /// Top-level task whose "add sub-task" field is open, with its text.
    adding_under: Option<(Id, String)>,
    renaming: Option<EditDraft>,
    confirm_delete: Option<Id>,
    message: Option<String>,
    /// The task whose timer is running, refreshed every frame.
    running_task: Option<Id>,
}

/// A task being edited. Category and break interval apply to top-level tasks only.
struct EditDraft {
    id: Id,
    title: String,
    category: Option<Id>,
    break_interval: Option<u32>,
    top_level: bool,
}

/// What the user asked for this frame; applied after drawing.
enum Action {
    Add,
    AddSubtask,
    Toggle(Id, bool),
    OpenSubtask(Id),
    OpenRename(EditDraft),
    Rename,
    Start(Id),
    Stop,
    Delete(Id),
    ArmDelete(Option<Id>),
    Close,
}

impl TasksScreen {
    pub fn ui(&mut self, ui: &mut Ui, store: &dyn SessionRepo, clock: &dyn Clock) {
        let tasks: &dyn TaskRepo = store;
        let categories: &dyn CategoryRepo = store;
        self.running_task = store.running().ok().flatten().and_then(|s| s.task_id);
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
            self.apply(action, store);
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

            if let Some(draft) = self.renaming.as_mut().filter(|d| d.id == id) {
                ui.add(TextEdit::singleline(&mut draft.title).desired_width(200.0))
                    .accessible_name("Task title");
                if draft.top_level {
                    category_picker(ui, ("rename_category", id), cats, &mut draft.category);
                    let mut own = draft.break_interval.is_some();
                    if ui.checkbox(&mut own, "Own break every").changed() {
                        draft.break_interval =
                            own.then_some(wt_core::Settings::default().break_interval_min);
                    }
                    if let Some(minutes) = draft.break_interval.as_mut() {
                        ui.add(DragValue::new(minutes).range(1..=180).suffix(" min"))
                            .accessible_name("Break every (min)");
                    }
                }
                if ui.button("Save").clicked() {
                    *action = Some(Action::Rename);
                }
                if ui.button("Cancel").clicked() {
                    *action = Some(Action::Close);
                }
                return;
            }

            let running = self.running_task == Some(id);
            let mut done = task.is_done();
            let title = RichText::new(&task.title);
            let title = if running { title.strong() } else { title };
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
                    *action = Some(Action::OpenRename(EditDraft {
                        id,
                        title: task.title.clone(),
                        category: top_level.then_some(task.category_id),
                        break_interval: task.break_interval_min,
                        top_level,
                    }));
                }
                if top_level && ui.button("Add sub-task").clicked() {
                    *action = Some(Action::OpenSubtask(id));
                }
                if running {
                    if ui.button("Stop").clicked() {
                        *action = Some(Action::Stop);
                    }
                } else if ui.button("Start").clicked() {
                    *action = Some(Action::Start(id));
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

    fn apply(&mut self, action: Action, store: &dyn SessionRepo) {
        let tasks: &dyn TaskRepo = store;
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
            Action::OpenRename(draft) => {
                self.renaming = Some(draft);
                Ok(())
            }
            Action::Rename => match self.renaming.take() {
                Some(draft) => save_edit(tasks, &draft),
                None => Ok(()),
            },
            Action::Start(id) => store.start(TimerTarget::Task(id)).map(|_| ()),
            Action::Stop => store.stop().map(|_| ()),
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

/// Writes only what changed, so a plain rename doesn't touch category or interval.
fn save_edit(tasks: &dyn TaskRepo, draft: &EditDraft) -> RepoResult<()> {
    tasks.rename(draft.id, &draft.title)?;
    if !draft.top_level {
        return Ok(());
    }
    let current = tasks.get(draft.id)?;
    if let Some(category_id) = draft.category
        && category_id != current.category_id
    {
        tasks.set_category(draft.id, category_id)?;
    }
    if draft.break_interval != current.break_interval_min {
        tasks.set_break_interval(draft.id, draft.break_interval)?;
    }
    Ok(())
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
