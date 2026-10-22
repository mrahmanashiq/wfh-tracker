# Spec: `reports` module

Status: **approved 2026-10-22** · Parent: [SPEC.md](SPEC.md) (Capability Map → `reports`) · Depends on: `tracking`

## Objective

At the end of the day, one click puts an accurate plain-text office report on the clipboard, ready to paste into Teams. A second report, **My time**, is private to you. It shows where your own time went: learning and personal time.

### User stories

1. A **Reports** screen in the sidebar shows the reports for **today**. **‹ / ›** steps to earlier days.
2. **Copy office report** puts the office report on the clipboard. The button then says **Copied** for a moment.
3. The **My time** report shows the day's learning (tasks learned or finished) and personal time by category (Family, Social media, …). It has its own **Copy** button and is never mixed into the office report.

## Office report format

```
Office report — Thu 22 Oct 2026 — 7h 45m (target 8h)

Checkout redesign — 5h 15m
  ✅ Payment form validation — 2h 10m
  ⏳ Stripe API integration — 3h 05m
✅ Review PR #482 — 50m
Other office work — 20m
Meetings — 3 (1h 20m)
Overtime — none
```

| Line | Rule |
|---|---|
| Header | Local date, total office time (office + meetings), target. On a day off: `(day off)` instead of the target. |
| Tasks | Office-kind tasks **worked on or finished that day**, top-level tasks in creation order. A parent shows its total including sub-tasks, then its sub-tasks indented. A task without sub-tasks is a single line. |
| ✅ / ⏳ | ✅ done that day (or earlier), ⏳ still open |
| Times | `2h 10m`, `50m`, from that day's sessions only (split at local midnight) |
| Other office work | Office time with no task (quick start). Omitted when zero. |
| Meetings | Count and total. `Meetings — none` when zero. |
| Overtime | `none`, `30m`, or the whole office time on a day off |
| Deleted tasks | Their time still counts, listed as `(deleted task)` |

## My time report format (private)

```
My time — Thu 22 Oct 2026

Learning — 1h 20m
  ✅ Rust async book, chapter 4 — 50m
  Learning (no task) — 30m

Personal — 2h 10m
  Family — 1h 30m
  Social media — 40m
```

- Learning: learning-kind tasks worked on or finished that day, then quick-start learning time as `<category> (no task)`.
- Personal: personal- and custom-kind time grouped by category, in category order.
- An empty section is omitted. When both are empty, the body reads `Nothing tracked for yourself this day.`

## Architecture

- **`wt-core` `reports.rs`:** pure functions `office_report(&ReportInput) -> String` and `my_time_report(&ReportInput) -> String`. `ReportInput` holds day, tz, now, sessions, tasks and settings.
- **`wt-core`:** a `Clipboard` port. `wt-platform` implements it with `arboard` (already in the Tech Stack table).
- **`wt-ui-egui`:** a `ReportsScreen` with the day picker, two read-only previews and the Copy buttons. A Reports item goes in the sidebar.
- **Fonts:** Segoe UI Emoji is added as a fallback, so ✅ ⏳ render in the preview.

## Success criteria

1. Literal expected-text tests for both reports. They cover: a parent with sub-tasks, a single task, quick-start office time, meetings, overtime, a day off, a deleted task, a session crossing midnight, and learning plus personal grouping.
2. Copy puts exactly the report text on the clipboard (through a fake clipboard in the UI test).
3. ‹ shows yesterday's report, built from yesterday's sessions only.
4. `cargo xtask ci` passes.

## Decisions (2026-10-22)

- The private report is **My time**: learning plus personal time.
- The office report lists tasks **worked on or finished** that day only.
- **Test seams:** report text in core (literal expected strings), and the Reports screen via kittest with a fake clipboard.
