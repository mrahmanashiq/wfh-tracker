# Spec: `tasks` module

Status: **approved 2026-10-22** · Parent: [SPEC.md](SPEC.md) (Capability Map → `tasks`) · Depends on: `foundation`

## Objective

Give the user one list of the work they intend to do — office features with sub-features, learning goals, personal to-dos — that survives from day to day until it's done. The `tracking` module will later start timers on these tasks.

### User stories

1. I add a **task** with a title and a **category** (e.g. "Checkout feature" · Office).
2. I add **sub-tasks** under a task (e.g. "Payment form validation"). A sub-task always has its parent's category.
3. I tick a task or sub-task **done**, and can untick it (**undone**) if I was wrong.
4. **Carry-over:** the Tasks screen shows every open task, plus tasks I finished *today*. Tasks finished on earlier days drop off the list. Nothing is copied: an open task is simply still open tomorrow.
5. I can **rename** a task and **delete** it. Deleting a task deletes its sub-tasks.
6. I manage **categories**: add (name, kind, colour), rename/recolour, and delete. A category still used by an open or done task can't be deleted. The app says which tasks use it.

## Domain rules

| Rule | Value |
|---|---|
| Nesting depth | Two levels: task → sub-task. A sub-task can't have sub-tasks. |
| Sub-task category | Always the parent's. Changing the parent's category changes the sub-tasks too. |
| Done timestamp | `done_at` is set when ticked and cleared when unticked. |
| "Today" | The local calendar day from the `Clock`'s time zone. |
| Visible on Tasks screen | A task is visible if it's open or was done today (local). A parent is shown if it or any of its sub-tasks is visible. |
| Parent auto-done | When every sub-task is done, the parent is marked done. When a sub-task is unticked, or a new one is added, the parent reopens. Ticking or unticking a parent by hand doesn't change its sub-tasks. The parent shows progress, e.g. `2/5`. |
| Order | Creation order (UUID v7 id). No manual reordering in this module. |
| Title | Trimmed, 1–200 characters |
| Category delete | Refused while any live task references it |

## Changes to existing code

- **`Clock` gains `fn time_zone(&self) -> TimeZone`** so "today" is decided in core, not the UI. `SystemClock` returns `TimeZone::system()`. `FixedClock::new` takes a time zone, with a `FixedClock::utc` shortcut so foundation tests stay short.
- **App shell gets a left navigation:** Tasks (default) · Categories · Settings.
- **Departure from SPEC.md's data model:** `task.sort_order` and `task.break_interval_min` are **not** added now. Ordering is by id. `break_interval_min` arrives with `tracking` as a new migration (`ALTER TABLE task ADD COLUMN`). This follows the "build only what's used" rule.

## Data

Migration `002_tasks.sql`:

| Column | Type | Notes |
|---|---|---|
| id, created_at, updated_at, deleted_at | as every table | |
| parent_id | TEXT NULL → task(id) | null = top-level |
| title | TEXT NOT NULL | |
| category_id | TEXT NOT NULL → category(id) | |
| status | TEXT CHECK IN ('open','done') | |
| done_at | TEXT NULL | fixed-width RFC 3339 |

`TaskRepo` (in `wt-core`): `list_live`, `create(NewTask)`, `rename`, `set_category`, `set_done(id, bool)`, `delete` (soft, cascades to children). `CategoryRepo::delete` now returns `RepoError::InUse { tasks: Vec<String> }` when tasks still reference the category.

The "visible today" filter and the `2/5` progress are **pure functions in `wt-core`** over the task list, not SQL. That keeps the rules testable with literal dates.

## Testing seams (to confirm)

| Seam | Catches | Misses |
|---|---|---|
| `wt-core` today-view function (task list + date + tz → visible tree with progress) | Carry-over rules, midnight edge in a non-UTC zone, sub-task progress | Persistence |
| `TaskRepo` on in-memory SQLite | Create/sub-task depth limit, done_at set/cleared, cascade delete, category-in-use refusal, sync columns | UI |
| Tasks screen via `egui_kittest` | Add task, add sub-task, tick done, delete | Real window |
| Categories screen via `egui_kittest` | Add category, blocked delete shows the using tasks | Real window |

## Success criteria

1. Add "Checkout" (Office) with sub-tasks "Validation" and "API". The list shows `Checkout 0/2` with both underneath.
2. Tick "Validation": the parent shows `1/2`. Tick "API": the parent is done automatically (`2/2`). Untick "API": the parent reopens and its `done_at` is cleared.
3. A task done at 23:50 local time is visible that day and gone the next. An open task created 3 days ago is still visible today. Both are checked in a UTC+6 time zone (Asia/Dhaka, where local and UTC dates differ around midnight).
4. Adding a sub-task under a sub-task is refused.
5. Deleting "Checkout" removes its sub-tasks from the list. The rows remain with `deleted_at` set.
6. Deleting the Office category while "Checkout" exists is refused with a message naming "Checkout".
7. `cargo xtask ci` passes.

## Decisions (2026-10-22)

- The parent is auto-ticked when all its sub-tasks are done, and reopens when one is unticked or added.
- There's no done-history view in this module. The `reports` module covers what was finished.
- All four test seams are agreed. The UI tests use the real SQLite store in memory (as a dev-dependency only), not hand-written fakes.
