use std::collections::{HashMap, HashSet};

use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align, Color32, CornerRadius, Layout, Rect, RichText, Role, Sense, Shape, Stroke, TextFormat,
    Ui, UiBuilder, WidgetInfo, vec2,
};
use jiff::SignedDuration;
use wt_core::repo::RepoResult;
use wt_core::task::MAX_DEPTH;
use wt_core::{
    Category, CategoryRepo, Clock, Id, Placement, SessionRepo, Settings, Task, TaskRepo, TaskTree,
    TimerTarget, display_order, duration_text, shows_tag, today_view, tracked_time,
};

use crate::theme::{FADE, bold, font, palette, radius, size, space, stroke, text};
use crate::widgets::{
    CheckShape, DeleteClick, Icon, color32, delete_buttons, empty_state, field, icon_button,
    message, pill, placeholder_check, progress_pie, raised_card, screen_header, tag, tag_picker,
    task_check, text_button, timer_capsule,
};

/// How far checklist items sit in from their to-do: one checkbox column, so
/// their ring lines up under the to-do's title.
const INDENT: f32 = size::CHECK + space::SM;
/// The gap above each project group.
const GROUP_GAP: f32 = space::XL;
/// Slots for the hover actions: Start, Add sub-task, Edit and Delete. Their
/// room is always kept, so the time column doesn't shift as they fade in.
const ACTION_SLOTS: f32 = 4.0;
/// Room the new-task field leaves for the category tag and "Add task".
const NEW_TASK_TRAILING: f32 = size::PICKER + size::TIME_COLUMN;
/// The break intervals a task can pick, in minutes. A task's own value
/// that isn't one of these is offered too, so editing never loses it.
const BREAK_PRESETS: [u32; 8] = [15, 20, 25, 30, 45, 50, 60, 90];

/// Today: open tasks carried over plus anything finished today, drawn like
/// the Things 3 Today list.
#[derive(Default)]
pub struct TasksScreen {
    new_title: String,
    new_category: Option<Id>,
    /// Task whose "add sub-task" field is open, with its text.
    adding_under: Option<(Id, String)>,
    renaming: Option<EditDraft>,
    confirm_delete: Option<Id>,
    message: Option<String>,
    /// The task whose timer is running; refreshed every frame.
    running: HashSet<Id>,
    /// The row under the pointer last frame; only it fades its actions in.
    hovered: Option<Id>,
    /// The row under the pointer this frame, found while drawing.
    pointer_on: Option<Id>,
    /// The row last clicked, drawn with Things' soft blue highlight.
    selected: Option<Id>,
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
    Select(Id),
    OpenSubtask(Id),
    OpenRename(EditDraft),
    Rename,
    Start(Id),
    Stop(Id),
    Delete(Id),
    ArmDelete(Option<Id>),
    Close,
}

/// How a row reads, by its place in the tree.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A top-level task with sub-tasks: a group heading with a pie.
    Project,
    /// A top-level task without sub-tasks, or a sub-task: a square box.
    ToDo,
    /// A third-level task: a small ring and smaller, muted text.
    Checklist,
}

impl Kind {
    fn of(tree: &TaskTree, depth: usize) -> Self {
        match depth {
            0 if tree.progress.is_some() => Kind::Project,
            0 | 1 => Kind::ToDo,
            _ => Kind::Checklist,
        }
    }

    /// The box this row shows when it is edited or has no pie.
    fn shape(self) -> CheckShape {
        match self {
            Kind::Project | Kind::ToDo => CheckShape::Square,
            Kind::Checklist => CheckShape::Small,
        }
    }

    /// The box its sub-tasks get.
    fn child_shape(self) -> CheckShape {
        match self {
            Kind::Project => CheckShape::Square,
            Kind::ToDo | Kind::Checklist => CheckShape::Small,
        }
    }
}

/// What one row draws.
struct Row<'a> {
    task: &'a Task,
    kind: Kind,
    progress: Option<(usize, usize)>,
    /// The tag, where it adds information (see [`shows_tag`]).
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
    /// The break interval from Settings, used when a task has none of its own.
    default_break: u32,
}

impl TasksScreen {
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        store: &dyn SessionRepo,
        settings: &Settings,
        clock: &dyn Clock,
    ) {
        let tasks: &dyn TaskRepo = store;
        let categories: &dyn CategoryRepo = store;
        let now = clock.now();
        let sessions = store
            .sessions_since(
                now.checked_sub(SignedDuration::from_hours(48))
                    .unwrap_or(now),
            )
            .unwrap_or_default();
        self.running = sessions
            .iter()
            .filter(|s| s.is_running())
            .filter_map(|s| s.task_id)
            .collect();

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
        let date = today.strftime("%A, %-d %B").to_string();
        screen_header(ui, Icon::Star, p.star, "Today", Some(&date));

        let mut action = None;
        self.new_task_line(ui, &cats, &mut action);
        message(ui, &self.message);
        ui.add_space(space::SM);

        let tz = clock.time_zone();
        let view = display_order(today_view(&all, today, &tz));
        if view.is_empty() {
            ui.add_space(space::XL);
            empty_state(
                ui,
                Icon::Star,
                "Nothing on your list today.",
                Some("Type a to-do above and press Enter."),
            );
        }
        let own_time = |id: Id| tracked_time(&sessions, TimerTarget::Task(id), today, &tz, now);
        let family_time: HashMap<Id, SignedDuration> = all
            .iter()
            .map(|t| (t.meta.id, family_time(&all, t.meta.id, &own_time)))
            .collect();
        let context = RowContext {
            by_id: &by_id,
            cats: &cats,
            family_time: &family_time,
            default_break: settings.break_interval_min,
        };
        self.pointer_on = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = space::XXS;
                for (i, tree) in view.iter().enumerate() {
                    if i > 0 && tree.progress.is_some() {
                        ui.add_space(GROUP_GAP);
                    }
                    self.tree_rows(ui, tree, None, 0, &context, &mut action);
                }
            });
        self.hovered = self.pointer_on;

        if let Some(action) = action {
            self.apply(action, store);
        }
    }

    /// A task's row, then the rows below it and the "add sub-task" field
    /// when it's open. A project's to-dos sit flush with its heading, as in
    /// Things; a to-do's checklist sits in by one checkbox column.
    fn tree_rows(
        &mut self,
        ui: &mut Ui,
        tree: &TaskTree,
        parent: Option<&Task>,
        depth: usize,
        context: &RowContext<'_>,
        action: &mut Option<Action>,
    ) {
        let id = tree.task.meta.id;
        let kind = Kind::of(tree, depth);
        let row = Row {
            task: &tree.task,
            kind,
            progress: tree.progress,
            category: shows_tag(&tree.task, parent)
                .then(|| context.by_id.get(&tree.task.category_id).copied())
                .flatten(),
            time: context.family_time.get(&id).copied().unwrap_or_default(),
            depth,
        };
        self.task_row(ui, &row, context, action);
        let adding_here = self.adding_under.as_ref().is_some_and(|a| a.0 == id);
        if tree.children.is_empty() && !adding_here {
            return;
        }
        let mut below = |ui: &mut Ui| {
            for child in &tree.children {
                self.tree_rows(ui, child, Some(&tree.task), depth + 1, context, action);
            }
            self.subtask_field(ui, id, kind.child_shape(), action);
        };
        if kind == Kind::Project {
            below(ui);
        } else {
            ui.horizontal(|ui| {
                ui.add_space(INDENT);
                ui.vertical(below);
            });
        }
    }

    /// Things' "New To-Do" placeholder row: a faint box, grey placeholder
    /// text and a quiet category tag.
    fn new_task_line(&mut self, ui: &mut Ui, cats: &[Category], action: &mut Option<Action>) {
        let row = vec2(ui.available_width(), size::TASK_ROW);
        ui.allocate_ui_with_layout(row, Layout::left_to_right(Align::Center), |ui| {
            placeholder_check(ui, CheckShape::Square);
            let field = ui
                .add(
                    field(&mut self.new_title)
                        .hint_text("New To-Do")
                        .frame(egui::Frame::new())
                        .margin(vec2(0.0, 0.0))
                        .min_size(vec2(0.0, size::TASK_ROW))
                        .font(font(text::TASK))
                        .desired_width(
                            (ui.available_width() - NEW_TASK_TRAILING).max(size::PICKER),
                        ),
                )
                .accessible_name("New task title");
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let add = !self.new_title.is_empty() && text_button(ui, "Add task", true).clicked();
                if add || enter {
                    *action = Some(Action::Add);
                }
                category_picker(ui, "new_task_category", cats, &mut self.new_category);
            });
        });
    }

    fn task_row(
        &mut self,
        ui: &mut Ui,
        row: &Row<'_>,
        context: &RowContext<'_>,
        action: &mut Option<Action>,
    ) {
        let id = row.task.meta.id;
        if let Some(draft) = self.renaming.as_mut().filter(|d| d.id == id) {
            edit_card(ui, draft, row.kind.shape(), context, action);
            return;
        }
        let p = palette(ui);
        let running = self.running.contains(&id);

        // The row's own click (selection) is registered first, so the
        // checkbox and the icons drawn over it win their clicks.
        let rect = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), size::TASK_ROW));
        let background = ui.painter().add(Shape::Noop);
        if ui
            .interact(rect, ui.scope_id().with(("task-row", id)), Sense::click())
            .clicked()
        {
            *action = Some(Action::Select(id));
        }
        let hover = ui.ctx().animate_bool_with_time(
            ui.scope_id().with(("task-hover", id)),
            self.hovered == Some(id),
            FADE,
        );

        ui.scope_builder(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                ui.set_min_size(rect.size());
                let done = row.task.is_done();
                let toggled = match (row.kind, row.progress) {
                    (Kind::Project, Some((finished, total))) => {
                        progress_pie(ui, finished, total, done).clicked()
                    }
                    _ => task_check(ui, done, &row.task.title, row.kind.shape()).clicked(),
                };
                if toggled {
                    *action = Some(Action::Toggle(id, !done));
                }

                let trailing = trailing_width(ui, row.time, running);
                let tag_room = row.category.map_or(0.0, |_| size::PICKER);
                let room = (ui.available_width() - trailing - tag_room).max(size::PICKER);
                title(ui, row, done, room);
                if row.kind != Kind::Project
                    && let Some((finished, total)) = row.progress
                {
                    ui.label(
                        RichText::new(format!("{finished}/{total}"))
                            .font(font(text::TAG))
                            .color(p.secondary),
                    );
                }
                if let Some(cat) = row.category {
                    tag(ui, &cat.name, color32(cat.color));
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
                    row_actions(ui, row, running, hover, action);
                    if running {
                        if timer_capsule(ui, &elapsed(row.time)).clicked() {
                            *action = Some(Action::Stop(id));
                        }
                    } else if row.time.is_positive() {
                        ui.label(
                            RichText::new(duration_text(row.time))
                                .font(font(text::SMALL))
                                .color(p.secondary),
                        );
                    }
                });
            },
        );

        if self.selected == Some(id) {
            ui.painter().set(
                background,
                Shape::rect_filled(
                    rect.expand2(vec2(space::SM, 0.0)),
                    CornerRadius::same(radius::MD),
                    p.selection,
                ),
            );
        }
        if row.kind == Kind::Project {
            ui.painter().hline(
                rect.x_range(),
                rect.bottom(),
                Stroke::new(stroke::HAIRLINE, p.hairline),
            );
        }
        if ui.rect_contains_pointer(rect) {
            self.pointer_on = Some(id);
        }
    }

    /// The "add sub-task" field, as a placeholder row with a faint box of
    /// the shape the new task will get.
    fn subtask_field(
        &mut self,
        ui: &mut Ui,
        parent: Id,
        shape: CheckShape,
        action: &mut Option<Action>,
    ) {
        let Some((_, text)) = self.adding_under.as_mut().filter(|a| a.0 == parent) else {
            return;
        };
        let row = vec2(ui.available_width(), size::TASK_ROW);
        ui.allocate_ui_with_layout(row, Layout::left_to_right(Align::Center), |ui| {
            placeholder_check(ui, shape);
            let field = ui
                .add(
                    field(text)
                        .hint_text("New sub-task")
                        .frame(egui::Frame::new())
                        .margin(vec2(0.0, 0.0))
                        .font(font(text_size(shape))),
                )
                .accessible_name("Sub-task title");
            field.request_focus();
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if text_button(ui, "Add", true).clicked() || enter {
                *action = Some(Action::AddSubtask);
            }
            if text_button(ui, "Cancel", false).clicked() {
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
            Action::Select(id) => {
                self.selected = Some(id);
                return;
            }
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
            Action::Stop(id) => store.stop_target(TimerTarget::Task(id)).map(|_| ()),
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

/// The row's quiet actions, right to left: Delete, Edit, Add sub-task and
/// Start. They fade in with `hover` (0..=1) and keep their room when hidden
/// or missing, so columns stay put.
fn row_actions(ui: &mut Ui, row: &Row<'_>, running: bool, hover: f32, action: &mut Option<Action>) {
    if hover <= 0.0 {
        ui.add_space(actions_width(ui));
        return;
    }
    let color = palette(ui).secondary.gamma_multiply(hover);
    let task = row.task;
    let id = task.meta.id;
    let top_level = task.parent_id.is_none();
    if icon_button(ui, Icon::Trash, "Delete", color).clicked() {
        *action = Some(Action::ArmDelete(Some(id)));
    }
    if icon_button(ui, Icon::Pencil, "Edit", color).clicked() {
        *action = Some(Action::OpenRename(EditDraft {
            id,
            title: task.title.clone(),
            category: top_level.then_some(task.category_id),
            break_interval: task.break_interval_min,
            top_level,
        }));
    }
    let can_nest = row.depth + 1 < MAX_DEPTH;
    slot(ui, can_nest, |ui| {
        if icon_button(ui, Icon::Plus, "Add sub-task", color).clicked() {
            *action = Some(Action::OpenSubtask(id));
        }
    });
    slot(ui, !running, |ui| {
        if icon_button(ui, Icon::Play, "Start", color).clicked() {
            *action = Some(Action::Start(id));
        }
    });
}

/// One action slot: `add` when `shown`, else the same room left empty.
fn slot(ui: &mut Ui, shown: bool, add: impl FnOnce(&mut Ui)) {
    if shown {
        add(ui);
    } else {
        ui.add_space(size::ICON_BUTTON);
    }
}

/// Width of the hover actions' block.
fn actions_width(ui: &Ui) -> f32 {
    let gap = ui.spacing().item_spacing.x;
    ACTION_SLOTS * (size::ICON_BUTTON + gap) - gap
}

/// Room the right side of a row takes: the actions plus the time.
fn trailing_width(ui: &Ui, time: SignedDuration, running: bool) -> f32 {
    let time = if running || time.is_positive() {
        size::TIME_COLUMN
    } else {
        0.0
    };
    actions_width(ui) + time + ui.spacing().item_spacing.x
}

/// Title size for a row whose box is `shape`.
fn text_size(shape: CheckShape) -> f32 {
    match shape {
        CheckShape::Square => text::TASK,
        CheckShape::Small => text::CHECKLIST,
    }
}

/// The task title, painted rather than added as a label so a to-do's
/// checkbox stays the one accessible element named after it. A project's
/// heading has a pie instead, so there the title carries the name. Cut
/// short with "…" past `max_width`.
fn title(ui: &mut Ui, row: &Row<'_>, done: bool, max_width: f32) {
    let p = palette(ui);
    let (font, color) = match row.kind {
        Kind::Project => (bold(ui, text::TASK), p.text),
        Kind::ToDo => (font(text::TASK), p.text),
        Kind::Checklist => (font(text::CHECKLIST), p.muted),
    };
    let color = if done { p.secondary } else { color };
    let mut job =
        LayoutJob::single_section(row.task.title.clone(), TextFormat::simple(font, color));
    job.wrap = TextWrapping::truncate_at_width(max_width);
    let galley = ui.painter().layout_job(job);
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    if row.kind == Kind::Project {
        response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &row.task.title));
    }
    ui.painter().galley(rect.min, galley, color);
}

/// The task opened for editing: a raised card inside the list holding the
/// title and, for a top-level task, its category and break interval.
fn edit_card(
    ui: &mut Ui,
    draft: &mut EditDraft,
    shape: CheckShape,
    context: &RowContext<'_>,
    action: &mut Option<Action>,
) {
    let p = palette(ui);
    ui.add_space(space::XS);
    raised_card(ui, |ui| {
        let line = vec2(ui.available_width(), size::TASK_ROW);
        ui.allocate_ui_with_layout(line, Layout::left_to_right(Align::Center), |ui| {
            placeholder_check(ui, shape);
            ui.add(
                field(&mut draft.title)
                    .frame(egui::Frame::new())
                    .margin(vec2(0.0, 0.0))
                    .min_size(vec2(0.0, size::TASK_ROW))
                    .font(font(text_size(shape)))
                    .desired_width(f32::INFINITY),
            )
            .accessible_name("Task title");
        });
        ui.add_space(space::XS);
        ui.horizontal(|ui| {
            ui.add_space(INDENT);
            if draft.top_level {
                category_picker(
                    ui,
                    ("rename_category", draft.id),
                    context.cats,
                    &mut draft.category,
                );
                break_picker(
                    ui,
                    ("rename_break", draft.id),
                    context.default_break,
                    &mut draft.break_interval,
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if pill(ui, "Save", p.accent, Color32::WHITE).clicked() {
                    *action = Some(Action::Rename);
                }
                if text_button(ui, "Cancel", false).clicked() {
                    *action = Some(Action::Close);
                }
            });
        });
    });
    ui.add_space(space::SM);
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

/// A category drop-down that looks like a Things tag: a small muted
/// capsule that fits its text.
fn category_picker(
    ui: &mut Ui,
    id_salt: impl egui::AsIdSalt,
    cats: &[Category],
    selected: &mut Option<Id>,
) {
    let current = cats.iter().find(|c| Some(c.meta.id) == *selected);
    let name = current.map_or("-", |c| c.name.as_str());
    tag_picker(ui, id_salt, name, "Category", |ui| {
        for c in cats {
            ui.selectable_value(selected, Some(c.meta.id), &c.name);
        }
    });
}

/// A task's own break interval as a tag drop-down beside its category:
/// the Settings default (no value of its own) or one of [`BREAK_PRESETS`],
/// plus the task's current value when it is none of those.
fn break_picker(
    ui: &mut Ui,
    id_salt: impl egui::AsIdSalt,
    default: u32,
    selected: &mut Option<u32>,
) {
    let shown = match *selected {
        Some(minutes) => format!("Break {minutes}m"),
        None => "Default break".to_owned(),
    };
    let mut choices: Vec<u32> = BREAK_PRESETS.to_vec();
    if let Some(own) = *selected
        && !choices.contains(&own)
    {
        choices.push(own);
        choices.sort_unstable();
    }
    tag_picker(ui, id_salt, &shown, "Break interval", |ui| {
        ui.selectable_value(selected, None, format!("Default break ({default}m)"));
        for minutes in choices {
            ui.selectable_value(selected, Some(minutes), format!("Break every {minutes}m"));
        }
    });
}

/// Today's time on a task plus everything below it.
fn family_time(all: &[Task], id: Id, own: &dyn Fn(Id) -> SignedDuration) -> SignedDuration {
    all.iter()
        .filter(|t| t.parent_id == Some(id))
        .map(|t| family_time(all, t.meta.id, own))
        .fold(own(id), SignedDuration::saturating_add)
}
