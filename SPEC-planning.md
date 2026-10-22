# Spec: `planning` module

Status: **approved 2026-10-22** · Parent: [SPEC.md](SPEC.md) (Capability Map → `planning`) · Depends on: `tracking`

## Objective

Let the user decide ahead of time how much time things deserve, and when. **Targets** say "Learning: 5h this week". **Scheduled blocks** say "Sun–Thu 21:00-22:00 is Learning". The app shows progress against targets and reminds the user when a block starts. Later, the `nudges` module uses blocks to know when to watch for Facebook and reels.

### User stories

1. On a new **Planning** screen I add a **target**: a category, a period (**week** or **month**), and a number of hours. I can edit and delete targets.
2. Each target shows **progress for the current period**, e.g. `Learning · this week · 3h 20m / 5h`, with a progress bar. A met target turns green.
3. I add a **scheduled block**: a category, the weekdays it applies to, a start and an end time (e.g. 21:00-22:00), and an optional label. I can edit and delete blocks.
4. When a block starts, I get a notification: "Learning block 21:00-22:00 — start your timer". There's no notification if a timer for that category is already running.
5. While a block is on, the header shows it, e.g. `Learning block · until 22:00`.

## Domain rules

| Rule | Value |
|---|---|
| Week | Starts on Settings' `week_start` (Sunday); 7 local days |
| Month | Calendar month, local time |
| Target progress | Time in sessions of **that category** within the current period, counting a running session up to now |
| Target size | 1–200 h per week, 1–800 h per month; at most one target per category and period |
| Block times | Local times as minutes after midnight; `end > start` (no overnight blocks in this module) |
| Block weekdays | At least one |
| Block reminder | Once per block per day, on the first tick at or after its start, while the block is still on. Skipped if a session of the block's category is running. On app start, a block already in progress still gets its reminder. |
| Overlapping blocks | Allowed. Each block reminds on its own, and the header shows the earliest-starting one. |

## Data

Migration `004_planning.sql`, with the usual sync columns on both tables:
- `target(category_id, period ('week'|'month'), minutes)`
- `block(category_id, weekdays INTEGER bitmask, start_min, end_min, label)`

Repositories: `TargetRepo` (list / set / delete) and `BlockRepo` (list / create / update / delete). The validation rules live in core, as provided methods, the same way as `TaskRepo`.

## Architecture

- **`wt-core`:**
  - `Target`, `Period`, `Block`, `Weekdays`.
  - `period_range(day, period, week_start)` and `target_progress(...)`.
  - `active_blocks(blocks, now, tz)`.
  - `Reminders` gains `Notice::BlockStarted { block }`.
- **`wt-storage`:** the migration and both repositories.
- **`wt-ui-egui`:** a `PlanningScreen` (Targets section, Blocks section), a sidebar item, and the current-block chip in the header.

## Success criteria

1. Literal tests: week ranges with a Sunday start (and a Monday start), month ranges, and target progress that excludes other categories and other periods and counts a running session.
2. Block reminders: one per day at the start; none when the matching timer is running; none outside the block's weekdays; a block already in progress at app start reminds once.
3. Repositories: validation (empty weekdays, end ≤ start, out-of-range hours) and soft delete. Setting a target for a category and period that already has one replaces it, so duplicates can't exist. A category with targets or blocks can't be deleted.
4. Planning screen (kittest): add a target and see `0h 00m / 5h`; add a block and see it listed; delete both.
5. `cargo xtask ci` passes.

## As built (2026-10-22)

- `active_block(blocks, now, tz)` returns the one block to show (the earliest-starting block that is on).
- Block labels are limited to 60 characters, and blocks end by midnight.
- Editing a target replaces it. Moving it to another category or period removes the old one.
- A block whose times change reminds again that day, at its new start.

## Decisions (2026-10-22)

- A target counts time in **one category**.
- A block reminds **at its start** only.
- **Test seams:** core rules, the repositories on SQLite, and the Planning screen via kittest.
