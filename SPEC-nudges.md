# Spec: `nudges` module

Status: **approved 2026-10-23** by the orchestrator, on the user's instruction to build autonomously · Parent: [SPEC.md](SPEC.md) (Capability Map → `nudges`) · Depends on: `tracking`, `planning`

## Objective

Protect the time the user set aside. While a learning block is on, or a learning, office or meeting timer runs, the app watches which window is in front. If it's Facebook, reels or similar for longer than a grace period, a desktop notification nudges the user back, and the distracted time is recorded. Window titles never leave memory: only the matched rule and the times are stored.

### User stories

1. During a **learning block**, or while a **learning** timer runs, I open Facebook. After 60 s I get a notification: "Facebook during your Learning block" / "Back to it?". If I stay, I'm nudged again at most every 2 min.
2. The same happens during **office time** (an office or meeting timer is running): "Facebook during office time".
3. On a new **Focus** screen I see my **distraction rules** (Facebook, Reels, YouTube Shorts, Instagram to start with). I can add a rule (a text and whether it matches the window **title** or the **process** name), turn rules on and off, and delete them.
4. The private **My time** report shows how long I was distracted, per rule and context, e.g. `Distractions - 0h 25m` / `  Facebook - 20m (office)`. The office report never shows it.

## Domain rules

| Rule | Value |
|---|---|
| Watch context | `learning` while a block of a Learning-kind category is on, or a Learning session runs. Otherwise `office` while an Office or Meeting session runs. Otherwise not watching. Learning wins when both apply. Never while a Break session runs, not even inside a learning block. |
| Sampling | The foreground window (title + process name) is read every 5 s, also while the window is hidden in the tray (via `logic()`), separately from the 15 s reminder tick. The app's own window gives no sample. |
| Rule match | Enabled rules only. Case-insensitive substring of the window title (`match_on = title`) or of the process name (`match_on = process`). With several matches, the oldest rule wins. |
| Grace | `distraction_grace_secs` (60 s) of continuous matching of the same rule, in the same context, before anything happens. |
| Nudge | When the grace has passed, then again each time `nudge_cooldown_secs` (120 s) have passed since the last nudge while still matching. A new match within the cooldown of the last nudge waits for the cooldown. |
| Continuity | A gap of more than 30 s between samples (the PC slept, the app froze) breaks a match: a recorded event ends at the last sample before the gap, and matching starts over. |
| Event | Recorded once the grace has passed, with `started_at` = when the match began. Ends at the last sample that still matched, when the window stops matching (or matches another rule, or its rule is disabled), the context ends or changes, or the app quits. An event left open by a crash ends at the last heartbeat on the next start. |
| Rule pattern | Trimmed, 1-60 characters. |
| Default rules | Seeded once into a database that never had rules: Facebook, Reels, Shorts (YouTube Shorts), Instagram, all matching the title. |
| Report | My time report only. A `Distractions` section after Personal: the total, then one line per rule and context in rule order (learning before office). A rule deleted since shows as `(deleted rule)`. Time under a minute is left out, as in the rest of the reports. |
| Privacy | Window titles and process names are matched in memory only. They are never stored or logged. |

## Data

Migration `005_nudges.sql`, with the usual sync columns on both tables:
- `distraction_rule(pattern, match_on ('title'|'process'), enabled)`
- `distraction_event(rule_id → distraction_rule, context ('learning'|'office'), started_at, ended_at NULL)`

Repositories: `DistractionRuleRepo` (list / add / enable / delete / seed) and `DistractionEventRepo` (start / end open / list since / record a watcher command). Validation and seeding live in core, as provided methods.

## Architecture

- **`wt-core`:**
  - `distraction_rule.rs`: `DistractionRule`, `RuleDraft`, `MatchOn`.
  - `distraction_event.rs`: `DistractionEvent`, `WatchContext`.
  - `nudge.rs`: the `ForegroundWatcher` port and `ForegroundWindow` sample, `watch_context(...)`, `learning_block_on(...)`, `nudge_title(...)`, and `NudgeWatcher`, a pure state machine fed samples and timestamps that returns `NudgeCommand`s (Nudge, StartEvent, EndEvent).
  - `ReportInput` gains the day's distraction events and the rules.
- **`wt-storage`:** the migration, `distraction_rule.rs` (`DistractionRuleRepo`) and `distraction_event.rs` (`DistractionEventRepo`).
- **`wt-platform`:** `OsForegroundWatcher` (active-win-pos-rs), a thin adapter with no automated test. It returns no sample when the foreground window's process id is `std::process::id()`.
- **`wt-ui-egui`:** a `Nudges` poller (watcher → repo + notifier), a `FocusScreen` with a sidebar item, and the Distractions section on the Reports screen via the core report.

## Success criteria

1. Core (fake samples and timestamps): no nudge before 60 s; exactly one at 60 s; none again before 120 s after it; another at 120 s; a gap of more than 30 s ends the event at the last sample; no watching outside a context; disabled rules never match; context selection (learning block, learning session, office, meeting, break, nothing).
2. Repository on in-memory SQLite: pattern validation (empty, over 60 characters), soft delete, enable/disable, defaults seeded once (not again after the user deletes them all), events started and ended, stale open events ended at a given time.
3. My time report text with literal expected strings, including per-context lines, a deleted rule, sub-minute time left out, and an untouched office report.
4. Focus screen (kittest): the default rules are listed; add a rule; disable one; delete one with confirm.
5. SPEC.md success criterion 5: with a fake foreground watcher returning "Reels | Facebook" for more than 60 s during a learning block, exactly one nudge, none again within 2 min, and a stored `distraction_event` with the right duration. The window title is not in the database file.
6. `cargo xtask ci` passes.

## Deviations from SPEC.md

- **Success criterion 10:** distraction minutes are shown in the private **My time** report, not as a line in the office report. The office report is for Teams and stays unchanged, so there is no "include in the copied text" switch.

## Decisions (2026-10-23)

- Nudges watch learning **and** office time (SPEC.md decision).
- The rules UI is its own **Focus** screen, not part of Settings. The grace and cooldown stay on the Settings screen, where they already are.
- **Test seams:** the `NudgeWatcher` state machine, `DistractionRuleRepo` and `DistractionEventRepo` on SQLite, the report text, the Focus screen via kittest, and the `Nudges` poller with a fake foreground watcher. Real foreground detection and real toasts are checked by hand.

## As built (review fixes, 2026-10-23)

- **Event end time:** an event ends at the last sample that matched, not at the first one that didn't, so the stored duration leaves out the trailing 5 s poll. The same holds when the context ends, the rule is disabled, another rule matches, or the app quits (`NudgeWatcher::stop` takes no time any more).
- **Breaks:** `watch_context` returns no context while a Break session runs, even inside a scheduled learning block, so there are no nudges or events and an open event ends at its last matching sample.
- **The app itself:** `OsForegroundWatcher` gives no sample when the foreground window belongs to this process (its process id equals `std::process::id()`), so the app's own window never matches, e.g. the Focus screen showing "Facebook".
- **One aggregate per module:** rules and events are separate modules in core (`distraction_rule.rs`, `distraction_event.rs`) and storage, with one repository trait each (`DistractionRuleRepo`, `DistractionEventRepo`). Behaviour is unchanged. Rules are listed by `created_at`, then id (UUID v7, so creation order), which makes "the oldest rule wins" deterministic for rules seeded in the same instant.
- **Nudge title:** which learning block is on, and the title built from it, live in core (`learning_block_on`, `nudge_title`); `watch_context` uses the same `learning_block_on`.
