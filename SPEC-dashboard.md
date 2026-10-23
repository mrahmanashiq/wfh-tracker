# Spec: `dashboard` module

Status: **approved 2026-10-23** by the orchestrator, on the user's instruction to build autonomously · Parent: [SPEC.md](SPEC.md) (Capability Map → `dashboard`) · Depends on: `reports`, `planning`, `nudges`

## Objective

One screen that answers "how is my day and my week going?" at a glance: today's office time against the target, where today's time went on a timeline, office hours per day this week, time per category against the weekly targets, and how much time distractions took. It only reads; nothing is edited here.

### User stories

1. A **Dashboard** item in the sidebar, directly below Today, opens the Dashboard screen.
2. **Today cards:** office time against the target with a progress bar (overtime in the danger colour; on a day off the card says "day off"), learning today, personal today and distractions today.
3. **Today timeline:** a horizontal strip with one coloured segment per session (the category's colour; breaks muted; a running session up to now), hour ticks and a "now" marker. Hovering a segment shows e.g. `Checkout 09:00-10:30` (the category name for a session with no task).
4. **This week** (from Settings' `week_start`): office time per day as 7 small bars with a line at the daily target and the overtime part in the danger colour; and time per category this week as horizontal bars, with `x / y` target progress where a weekly target exists.
5. **Distractions this week:** the total and the time per rule.

## Domain rules

| Rule | Value |
|---|---|
| Times shown | Whole minutes, as in the reports (`duration_text`: `50m`, `2h 05m`; totals `0h 25m`). |
| Day split | Sessions and distraction events are split at local midnight; a running session or open event counts up to now. |
| Office card | `Office 6h 12m / 8h`. On a day off (not in `workdays`): `Office 2h 00m - day off`. |
| Overtime | As `office_status`: time past the soft ceiling on a workday, all office time on a day off. Painted in the danger colour. |
| Learning / personal | Session kinds `learning`, and `personal` + `custom` (same grouping as the My time report). |
| Timeline window | From the hour the first segment starts to the hour after the last one ends, at least 8 hours (extended later, then earlier, within 00:00-24:00). With nothing tracked: 09:00-17:00. |
| Timeline segment | One per session that has time on the day, clipped to the day. Its name: `<task title or category name> HH:MM-HH:MM` (local, minutes floored; a segment ending at midnight ends `24:00`). A deleted task shows as `(deleted task)`. |
| Week | `period_range(today, Week, week_start)`: 7 local days. |
| Weekday bar name | `Sun 7h 30m` (day name and office time); a day off is `Fri 2h 00m - day off`. |
| Category this week | Every category with time this week, except breaks, most time first, then categories with a weekly target and no time yet. With a weekly target: `Learning 3h 20m / 5h` (as on the Planning screen), otherwise `Family 1h 10m`. |
| Distractions | Total and one line per rule (its pattern) with time, in rule order, a deleted rule as `(deleted rule)`. Lines under a minute are left out. |

## Architecture

- **`wt-core` `dashboard.rs`** (pure functions, no new aggregate):
  - `day_timeline(sessions, day, tz, now) -> Vec<TimelineSegment>` and `timeline_window(segments) -> (u16, u16)` (local minutes).
  - `office_by_day(sessions, week_first_day, tz, now, settings) -> Vec<DayOffice>` (7 days, with the day's target or `None` on a day off, and its overtime).
  - `category_totals(sessions, targets, from, to, tz, now) -> Vec<CategoryTime>` over local dates `[from, to)`; categories of `targets` with no time come last.
  - `distraction_totals(events, rules, from, to, tz, now) -> DistractionTotals`.
  - `kind_time(sessions, day, tz, now, keep)` for the learning and personal cards.
- **`wt-ui-egui` `dashboard_screen.rs`:** `DashboardScreen`, painted with egui's painter (no new dependencies). Every painted mark is a hoverable widget with an accessible name. Colours come from the theme palette and the category colours.
- No schema change.

## Success criteria

1. Core, literal-date tests: a session crossing midnight split into the timeline of both days; a running session up to now; the timeline window (minimum 8 h, empty day); office per day for a Sunday-start week, including a day off with overtime and the ceiling overtime on a workday; category totals within the week only, breaks excluded; distraction totals per rule with a deleted rule.
2. Dashboard screen (kittest, real in-memory store, `FixedClock`): the office card text, a timeline segment's accessible name, a weekday bar's name, a category target line and the distraction total.
3. `cargo xtask ci` passes.

## Decisions (2026-10-23)

- Charts are painted by hand with egui's painter; no chart crate.
- The dashboard shows the current week only (no month view or paging in v1). Monthly targets stay on the Planning screen.
- Overtime uses the same definition as the status bar and reports (past the soft ceiling), so the three never disagree.
- **Test seams:** the core functions, and the Dashboard screen via kittest.

## As built (2026-10-23)

- The sidebar item uses a new painted `Chart` icon. Card values are painted text in the proportional font (the semibold family is only installed by the app, not in tests).
- The timeline marks are `hover` widgets with a tooltip; the weekday bars carry the same name as their tooltip. Category and distraction rows are plain labels beside a painted bar.
- A category's bar fills against its weekly target (green once met); without a target it is scaled to the week's longest category.
- The Office card's accessible name includes its note (`Office 9h 30m / 8h - 30m overtime`). On a day off with office time the note and a full bar are in the danger colour, as all of it is overtime. A day-off weekday bar paints "day off" under its time.
