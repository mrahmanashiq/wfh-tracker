use std::collections::HashMap;

use egui::{
    Align, Button, ComboBox, CornerRadius, DragValue, FontId, Layout, Popup, RichText, Sense,
    Shape, Stroke, TextEdit, Ui, vec2,
};
use jiff::SignedDuration;
use wt_core::repo::RepoResult;
use wt_core::{
    Category, CategoryRepo, Clock, Id, Placement, SessionRepo, Settings, Task, TaskRepo,
    TimerTarget, today_view, tracked_time,
};

use crate::theme::palette;
use crate::widgets::{
    DeleteClick, Icon, color32, delete_buttons, icon_button, message, progress_pie, round_check,
};

/// Today: open tasks carried over plus anything finished today.
#[derive(Default)]
pub struct TasksScreen {
    new_title: String,
    new_category: Option<Id>,
    /// Top-level task whose "add sub-task" field is open, with its text.
    adding_under: Option<(Id, String)>,
    renaming: Option<EditDraft>,
    confirm_delete: Option<Id>,
    message: Option<String>,
    /// The task whose timer is running; refreshed every frame.
    running: Option<Id>,
    /// The row under the pointer last frame; only it shows its actions.
    hovered: Option<Id>,
    /// The row whose "More" menu is open; it keeps its actions while the
    /// pointer is over the menu, outside the row.
    menu_open: Option<Id>,
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

/// What one row draws.
struct Row<'a> {
    task: &'a Task,
    progress: Option<(usize, usize)>,
    /// Shown as a tag on top-level rows only.
    category: Option<&'a Category>,
    /// Time tracked on it today (a parent includes its sub-tasks).
    time: SignedDuration,
}

impl TasksScreen {
    pub fn ui(&mut self, ui: &mut Ui, store: &dyn SessionRepo, clock: &dyn Clock) {
        let tasks: &dyn TaskRepo = store;
        let categories: &dyn CategoryRepo = store;
        let now = clock.now();
        let running = store.running().ok().flatten();
        self.running = running.as_ref().and_then(|s| s.task_id);
        let sessions = store
            .sessions_since(
                now.checked_sub(SignedDuration::from_hours(48))
                    .unwrap_or(now),
            )
            .unwrap_or_default();

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

        let p = palette(ui);
        let today = clock.today();
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
            Icon::Star.paint(ui.painter(), rect.expand(3.0), p.star);
            ui.heading("Today");
        });
        ui.label(RichText::new(today.strftime("%A, %-d %B").to_string()).color(p.secondary));
        ui.add_space(16.0);

        let mut action = None;
        self.new_task_line(ui, &cats, &mut action);
        message(ui, &self.message);
        ui.add_space(8.0);

        let view = today_view(&all, today, &clock.time_zone());
        if view.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Nothing on your list today.").color(p.secondary));
            });
        }
        let tz = clock.time_zone();
        let own_time = |id: Id| tracked_time(&sessions, TimerTarget::Task(id), today, &tz, now);
        let mut hovered = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for tree in &view {
                    let family_time = all
                        .iter()
                        .filter(|t| t.parent_id == Some(tree.task.meta.id))
                        .map(|t| own_time(t.meta.id))
                        .fold(own_time(tree.task.meta.id), SignedDuration::saturating_add);
                    let top = Row {
                        task: &tree.task,
                        progress: tree.progress,
                        category: by_id.get(&tree.task.category_id).copied(),
                        time: family_time,
                    };
                    self.task_row(ui, &top, &cats, &mut hovered, &mut action);
                    let has_children = !tree.subtasks.is_empty()
                        || self
                            .adding_under
                            .as_ref()
                            .is_some_and(|a| a.0 == tree.task.meta.id);
                    if has_children {
                        // Sub-tasks sit under the title, without a guide line (Things-style).
                        ui.horizontal(|ui| {
                            ui.add_space(28.0);
                            ui.vertical(|ui| {
                                for sub in &tree.subtasks {
                                    let row = Row {
                                        task: sub,
                                        progress: None,
                                        category: None,
                                        time: own_time(sub.meta.id),
                                    };
                                    self.task_row(ui, &row, &cats, &mut hovered, &mut action);
                                }
                                self.subtask_field(ui, tree.task.meta.id, &mut action);
                            });
                        });
                    }
                    ui.add_space(6.0);
                }
            });
        self.hovered = hovered;

        if let Some(action) = action {
            self.apply(action, store);
        }
    }

    /// Things' "New To-Do" line: a ring, a borderless field and a category picker.
    fn new_task_line(&mut self, ui: &mut Ui, cats: &[Category], action: &mut Option<Action>) {
        let p = palette(ui);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
            ui.painter()
                .circle_stroke(rect.center(), 8.0, Stroke::new(1.5, p.line));
            let field = ui
                .add(
                    TextEdit::singleline(&mut self.new_title)
                        .hint_text("New To-Do")
                        .frame(egui::Frame::new())
                        .margin(vec2(0.0, 3.0))
                        .font(FontId::proportional(15.0))
                        .desired_width((ui.available_width() - 230.0).max(120.0)),
                )
                .accessible_name("New task title");
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            category_picker(ui, "new_task_category", cats, &mut self.new_category);
            let add = !self.new_title.is_empty()
                && ui
                    .add(Button::new(RichText::new("Add task").color(p.accent)).frame(false))
                    .clicked();
            if add || enter {
                *action = Some(Action::Add);
            }
        });
        ui.add_space(4.0);
        let y = ui.cursor().top();
        ui.painter()
            .hline(ui.max_rect().x_range(), y, Stroke::new(1.0, p.hover));
    }

    fn task_row(
        &mut self,
        ui: &mut Ui,
        row: &Row<'_>,
        cats: &[Category],
        hovered: &mut Option<Id>,
        action: &mut Option<Action>,
    ) {
        let p = palette(ui);
        let id = row.task.meta.id;
        let running = self.running == Some(id);
        let show_actions = running || self.hovered == Some(id) || self.menu_open == Some(id);

        let background = ui.painter().add(Shape::Noop);
        let inner = ui.allocate_ui_with_layout(
            vec2(ui.available_width(), 32.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                if let Some(draft) = self.renaming.as_mut().filter(|d| d.id == id) {
                    edit_fields(ui, draft, cats, action);
                    return;
                }

                let done = row.task.is_done();
                if round_check(ui, done, &row.task.title).clicked() {
                    *action = Some(Action::Toggle(id, !done));
                }
                title(ui, &row.task.title, done, running);
                if let Some((finished, total)) = row.progress {
                    progress_pie(ui, finished, total);
                }
                if let Some(cat) = row.category {
                    ui.label(RichText::new(&cat.name).small().color(color32(cat.color)));
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if self.confirm_delete == Some(id) {
                        match delete_buttons(ui, true) {
                            Some(DeleteClick::Confirm) => *action = Some(Action::Delete(id)),
                            Some(DeleteClick::Cancel) => *action = Some(Action::ArmDelete(None)),
                            _ => {}
                        }
                        return;
                    }
                    if show_actions {
                        self.more_menu(ui, row.task, action);
                    }
                    if running {
                        if icon_button(ui, Icon::Stop, "Stop", p.accent).clicked() {
                            *action = Some(Action::Stop);
                        }
                        ui.label(RichText::new(elapsed(row.time)).color(p.accent));
                        return;
                    }
                    if show_actions && icon_button(ui, Icon::Play, "Start", p.secondary).clicked() {
                        *action = Some(Action::Start(id));
                    }
                    if row.time.is_positive() {
                        ui.label(RichText::new(compact(row.time)).small().color(p.secondary));
                    }
                });
            },
        );
        let rect = inner.response.rect;
        let fill = if running {
            Some(p.accent_soft)
        } else if self.hovered == Some(id) {
            Some(p.hover)
        } else {
            None
        };
        if let Some(fill) = fill {
            ui.painter().set(
                background,
                Shape::rect_filled(rect.expand2(vec2(6.0, 0.0)), CornerRadius::same(8), fill),
            );
        }
        if ui.rect_contains_pointer(rect) {
            *hovered = Some(id);
        }
    }

    /// The "⋯" menu: rename, add a sub-task, delete.
    fn more_menu(&mut self, ui: &mut Ui, task: &Task, action: &mut Option<Action>) {
        let p = palette(ui);
        let id = task.meta.id;
        let top_level = task.parent_id.is_none();
        let button = icon_button(ui, Icon::More, "More", p.secondary);
        let popup_id = Popup::default_response_id(&button);
        Popup::menu(&button).show(|ui| {
            ui.set_min_width(150.0);
            if ui.button("Rename").clicked() {
                *action = Some(Action::OpenRename(EditDraft {
                    id,
                    title: task.title.clone(),
                    category: top_level.then_some(task.category_id),
                    break_interval: task.break_interval_min,
                    top_level,
                }));
                ui.close();
            }
            if top_level && ui.button("Add sub-task").clicked() {
                *action = Some(Action::OpenSubtask(id));
                ui.close();
            }
            if ui.button(RichText::new("Delete").color(p.danger)).clicked() {
                *action = Some(Action::ArmDelete(Some(id)));
                ui.close();
            }
        });
        if Popup::is_id_open(ui.ctx(), popup_id) {
            self.menu_open = Some(id);
        } else if self.menu_open == Some(id) {
            self.menu_open = None;
        }
    }

    fn subtask_field(&mut self, ui: &mut Ui, parent: Id, action: &mut Option<Action>) {
        let Some((_, text)) = self.adding_under.as_mut().filter(|a| a.0 == parent) else {
            return;
        };
        let p = palette(ui);
        ui.horizontal(|ui| {
            let field = ui
                .add(
                    TextEdit::singleline(text)
                        .hint_text("New sub-task")
                        .frame(egui::Frame::new()),
                )
                .accessible_name("Sub-task title");
            field.request_focus();
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui
                .add(Button::new(RichText::new("Add").color(p.accent)).frame(false))
                .clicked()
                || enter
            {
                *action = Some(Action::AddSubtask);
            }
            if ui.add(Button::new("Cancel").frame(false)).clicked() {
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

/// The task title, painted rather than added as a label so the checkbox stays
/// the one accessible element named after the task.
fn title(ui: &mut Ui, text: &str, done: bool, running: bool) {
    let p = palette(ui);
    let color = if done { p.secondary } else { p.text };
    let font = FontId::proportional(15.0);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    let painter = ui.painter();
    if running {
        // Faux bold for the running task: draw twice, half a pixel apart.
        painter.galley(rect.min + vec2(0.5, 0.0), galley.clone(), color);
    }
    painter.galley(rect.min, galley, color);
    if done {
        painter.hline(
            rect.x_range(),
            rect.center().y,
            Stroke::new(1.0, p.secondary),
        );
    }
}

fn edit_fields(ui: &mut Ui, draft: &mut EditDraft, cats: &[Category], action: &mut Option<Action>) {
    let p = palette(ui);
    ui.add(TextEdit::singleline(&mut draft.title).desired_width(200.0))
        .accessible_name("Task title");
    if draft.top_level {
        category_picker(ui, ("rename_category", draft.id), cats, &mut draft.category);
        let mut own = draft.break_interval.is_some();
        if ui.checkbox(&mut own, "Own break every").changed() {
            draft.break_interval = own.then_some(Settings::default().break_interval_min);
        }
        if let Some(minutes) = draft.break_interval.as_mut() {
            ui.add(DragValue::new(minutes).range(1..=180).suffix(" min"))
                .accessible_name("Break every (min)");
        }
    }
    if ui
        .add(Button::new(RichText::new("Save").color(p.accent)).frame(false))
        .clicked()
    {
        *action = Some(Action::Rename);
    }
    if ui.add(Button::new("Cancel").frame(false)).clicked() {
        *action = Some(Action::Close);
    }
}

/// `25m`, or `1h 05m` from an hour on.
fn compact(d: SignedDuration) -> String {
    let m = d.as_mins().max(0);
    match m / 60 {
        0 => format!("{m}m"),
        h => format!("{h}h {:02}m", m % 60),
    }
}

/// `42:13`, or `1:02:05` past an hour.
fn elapsed(d: SignedDuration) -> String {
    let s = d.as_secs().max(0);
    match s / 3600 {
        0 => format!("{}:{:02}", s / 60, s % 60),
        h => format!("{h}:{:02}:{:02}", s % 3600 / 60, s % 60),
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

/// A quiet category drop-down showing the selected category's colour.
fn category_picker(
    ui: &mut Ui,
    id_salt: impl egui::AsIdSalt,
    cats: &[Category],
    selected: &mut Option<Id>,
) {
    let current = cats.iter().find(|c| Some(c.meta.id) == *selected);
    let text = current.map_or_else(
        || RichText::new("—"),
        |c| RichText::new(&c.name).color(color32(c.color)),
    );
    ComboBox::from_id_salt(id_salt)
        .selected_text(text)
        .width(130.0)
        .show_ui(ui, |ui| {
            for c in cats {
                ui.selectable_value(selected, Some(c.meta.id), &c.name);
            }
        })
        .response
        .accessible_name("Category");
}
