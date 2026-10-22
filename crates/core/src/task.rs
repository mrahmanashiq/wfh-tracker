use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;

use crate::{Id, Meta, RepoError};

/// A piece of intended work. Top-level tasks may have one level of sub-tasks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub meta: Meta,
    /// `None` for a top-level task.
    pub parent_id: Option<Id>,
    pub title: String,
    pub category_id: Id,
    /// When it was ticked; `None` while open.
    pub done_at: Option<Timestamp>,
    /// This task's own break reminder interval. Sub-tasks use their parent's.
    pub break_interval_min: Option<u32>,
}

impl Task {
    pub fn is_done(&self) -> bool {
        self.done_at.is_some()
    }

    fn visible_on(&self, day: Date, tz: &TimeZone) -> bool {
        self.done_at
            .is_none_or(|at| at.to_zoned(tz.clone()).date() == day)
    }
}

/// Where a new task goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    TopLevel {
        category_id: Id,
    },
    /// A sub-task of this top-level task (it takes the parent's category).
    Under(Id),
}

pub const MAX_TITLE_CHARS: usize = 200;

/// Trims a title and checks it is 1–200 characters.
pub fn clean_title(raw: &str) -> Result<String, RepoError> {
    let title = raw.trim();
    match title.chars().count() {
        0 => Err(RepoError::Invalid("a task needs a title")),
        n if n > MAX_TITLE_CHARS => Err(RepoError::Invalid(
            "a task title can be at most 200 characters",
        )),
        _ => Ok(title.to_owned()),
    }
}

/// A top-level task with the sub-tasks to show under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskTree {
    pub task: Task,
    /// Sub-tasks that are open or were done on the day, in creation order.
    pub subtasks: Vec<Task>,
    /// `(done, total)` over all live sub-tasks; `None` when there are none.
    pub progress: Option<(usize, usize)>,
}

/// What the Tasks screen shows on `day`: every open task, plus tasks done that
/// local day. A parent shows whenever it or one of its sub-tasks would show,
/// so unfinished work carries over without being copied.
pub fn today_view(tasks: &[Task], day: Date, tz: &TimeZone) -> Vec<TaskTree> {
    let mut tops: Vec<&Task> = tasks.iter().filter(|t| t.parent_id.is_none()).collect();
    tops.sort_by_key(|t| t.meta.id);

    tops.into_iter()
        .filter_map(|top| {
            let mut kids: Vec<&Task> = tasks
                .iter()
                .filter(|t| t.parent_id == Some(top.meta.id))
                .collect();
            kids.sort_by_key(|t| t.meta.id);

            let subtasks: Vec<Task> = kids
                .iter()
                .filter(|k| k.visible_on(day, tz))
                .map(|k| (*k).clone())
                .collect();
            if subtasks.is_empty() && !top.visible_on(day, tz) {
                return None;
            }
            let progress = (!kids.is_empty())
                .then(|| (kids.iter().filter(|k| k.is_done()).count(), kids.len()));
            Some(TaskTree {
                task: top.clone(),
                subtasks,
                progress,
            })
        })
        .collect()
}
