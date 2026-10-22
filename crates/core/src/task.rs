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

/// Levels of nesting: task → sub-task → sub-sub-task.
pub const MAX_DEPTH: usize = 3;

/// Trims a title and checks it is 1-200 characters.
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

/// A task with the tasks to show under it (up to [`MAX_DEPTH`] levels).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskTree {
    pub task: Task,
    /// Tasks directly below that are visible on the day, in creation order.
    pub children: Vec<TaskTree>,
    /// `(done, total)` over all live direct sub-tasks; `None` when there are none.
    pub progress: Option<(usize, usize)>,
}

/// What the Tasks screen shows on `day`: every open task, plus tasks done that
/// local day. A task shows whenever it or a task below it would show, so
/// unfinished work carries over without being copied.
pub fn today_view(tasks: &[Task], day: Date, tz: &TimeZone) -> Vec<TaskTree> {
    let mut tops: Vec<&Task> = tasks.iter().filter(|t| t.parent_id.is_none()).collect();
    tops.sort_by_key(|t| t.meta.id);
    tops.into_iter()
        .filter_map(|top| tree(top, tasks, day, tz))
        .collect()
}

fn tree(task: &Task, all: &[Task], day: Date, tz: &TimeZone) -> Option<TaskTree> {
    let mut kids: Vec<&Task> = all
        .iter()
        .filter(|t| t.parent_id == Some(task.meta.id))
        .collect();
    kids.sort_by_key(|t| t.meta.id);
    let children: Vec<TaskTree> = kids.iter().filter_map(|k| tree(k, all, day, tz)).collect();
    if children.is_empty() && !task.visible_on(day, tz) {
        return None;
    }
    let progress =
        (!kids.is_empty()).then(|| (kids.iter().filter(|k| k.is_done()).count(), kids.len()));
    Some(TaskTree {
        task: task.clone(),
        children,
        progress,
    })
}
