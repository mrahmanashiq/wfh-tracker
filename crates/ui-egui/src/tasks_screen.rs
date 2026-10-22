use std::collections::HashMap;

use egui::{
    Align, Button, ComboBox, CornerRadius, DragValue, FontId, Layout, RichText, Sense, Shape,
    Stroke, TextEdit, Ui, vec2,
};
use jiff::SignedDuration;
use wt_core::repo::RepoResult;
use wt_core::task::MAX_DEPTH;
use wt_core::{
    Category, CategoryRepo, Clock, Id, Placement, SessionRepo, Settings, Task, TaskRepo, TaskTree,
    TimerTarget, duration_text, today_view, tracked_time,
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
    /// Time tracked on it today (a parent includes everything below it).
    time: SignedDuration,
    /// 0 for a top-level task, 1 for a sub-task, 2 for a sub-sub-task.
    depth: usize,
}

/// What every row of one frame shares.
struct RowContext<'a> {
    by_id: &'a HashMap<Id, &'a Category>,
    cats: &'a [Category],
    /// Today's time per task, including everything below it.
    family_time: &'a HashMap<Id, SignedDuration>,
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
        let family_time: HashMap<Id, SignedDuration> = all
            .iter()
            .map(|t| (t.meta.id, family_time(&all, t.meta.id, &own_time)))
            .collect();
        let context = RowContext {
            by_id: &by_id,
            cats: &cats,
            family_time: &family_time,
        };
        let mut hovered = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for tree in &view {
                    self.tree_rows(ui, tree, 0, &context, &mut hovered, &mut action);
                    ui.add_space(6.0);
                }
            });
        self.hovered = hovered;

        if let Some(action) = action {
            self.apply(action, store);
        }
    }

    /// A task's row, then (indented, Things-style without a guide line) the
    /// rows below it and the "add sub-task" field when it's open.
    fn tree_rows(
        &mut self,
        ui: &mut Ui,
        tree: &TaskTree,
        depth: usize,
        context: &RowContext<'_>,
        hovered: &mut Option<Id>,
        action: &mut Option<Action>,
    ) {
        let id = tree.task.meta.id;
        let row = Row {
            task: &tree.task,
            progress: tree.progress,
            category: (depth == 0)
                .then(|| context.by_id.get(&tree.task.category_id).copied())
                .flatten(),
            time: context.family_time.get(&id).copied().unwrap_or_default(),
            depth,
        };
        self.task_row(ui, &row, context.cats, hovered, action);
        let adding_here = self.adding_under.as_ref().is_some_and(|a| a.0 == id);
        if tree.children.is_empty() && !adding_here {
            return;
        }
        ui.horizontal(|ui| {
            ui.add_space(28.0);
            ui.vertical(|ui| {
                for child in &tree.children {
                    self.tree_rows(ui, child, depth + 1, context, hovered, action);
                }
                self.subtask_field(ui, id, action);
            });
        });
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
        let show_actions = running || self.hovered == Some(id);

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
                        self.row_icons(ui, row.task, row.depth, action);
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
                        ui.label(
                            RichText::new(duration_text(row.time))
                                .small()
                                .color(p.secondary),
                        );
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

    /// The row's icons, shown on hover: delete, edit, add a sub-task.
    fn row_icons(&self, ui: &mut Ui, task: &Task, depth: usize, action: &mut Option<Action>) {
        let p = palette(ui);
        let id = task.meta.id;
        let top_level = task.parent_id.is_none();
        if let Some(DeleteClick::Arm) = delete_buttons(ui, false) {
            *action = Some(Action::ArmDelete(Some(id)));
        }
        if icon_button(ui, Icon::Pencil, "Edit", p.secondary).clicked() {
            *action = Some(Action::OpenRename(EditDraft {
                id,
                title: task.title.clone(),
                category: top_level.then_some(task.category_id),
                break_interval: task.break_interval_min,
                top_level,
            }));
        }
        let can_nest = depth + 1 < MAX_DEPTH;
        if can_nest && icon_button(ui, Icon::Plus, "Add sub-task", p.secondary).clicked() {
            *action = Some(Action::OpenSubtask(id));
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

/// Today's time on a task plus everything below it.
fn family_time(all: &[Task], id: Id, own: &dyn Fn(Id) -> SignedDuration) -> SignedDuration {
    all.iter()
        .filter(|t| t.parent_id == Some(id))
        .map(|t| family_time(all, t.meta.id, own))
        .fold(own(id), SignedDuration::saturating_add)
}
