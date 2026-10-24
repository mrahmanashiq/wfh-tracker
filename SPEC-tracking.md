# Spec: `tracking` module

Status: **approved 2026-10-22** · Parent: [SPEC.md](SPEC.md) (Capability Map → `tracking`) · Depends on: `tasks`

## Objective

Record where time actually goes, and keep the office day near 8 hours. One timer runs at a time. Breaks don't count toward office hours. Meetings do. Reminders arrive at 8h and 9h, and nothing is ever stopped automatically.

### User stories

1. I press **Start** on a task or sub-task. Its timer runs, and starting another task stops the current one.
2. I press **Stop** to end the running timer.
7. **Quick start:** buttons for non-office categories (e.g. Family, Social media, Learning) start a timer with no task attached, so personal time is tracked too.
8. If the app was closed, or the PC slept, while a timer ran, the timer is stopped at the last moment the app was alive, and a banner tells me so.
3. A **status bar**, always visible, shows the running task and its elapsed time, plus today's office clock: `6h 12m / 8h`.
4. Every **N minutes of continuous work** (default 25, or the task's own value) I get a **break reminder**. **Take break** starts a break timer and pauses work. When the break length (default 5 min) has passed, I get "Break over" and a **Resume ‹task›** button.
5. I press **Meeting** (a button in the status bar, or the global hotkey **Ctrl+Alt+M**) when a Teams call starts. The current timer stops and a meeting timer starts. Pressing it again ends the meeting and **automatically restarts the timer of the task the meeting interrupted** (changed 2026-10-22 at the user's request; it used to offer a Resume button).
6. At **8h** of office time I get "Office day complete — wrap up". At **9h**, a soft warning. Time beyond 9h shows as **overtime**. On a weekend day (Fri/Sat) there's no 8h reminder, and all office time is overtime.

## Domain rules

| Rule | Value |
|---|---|
| Active sessions | At most one. Starting a session ends the running one at the same instant, in one transaction. |
| Session category / kind | Copied from the task's category when the session starts. Meeting sessions use the first category of kind Meeting. Break sessions use the first category of kind Break. |
| Office time | Sum of sessions of kind `office` + `meeting` within the local day. A session crossing midnight is split between the two days. |
| Office status | `UnderTarget` < target ≤ `TargetReached` < ceiling ≤ `SoftCeiling`, with `overtime = office − ceiling` once past it. Weekend: no target, `overtime = office`. |
| Threshold notices | Each fires once, on the tick where office time crosses the threshold. On app start, thresholds already passed don't fire. |
| Work stretch | Back-to-back non-break sessions where each gap is shorter than the break length. A break session, or a gap at least as long as the break length, starts a new stretch. |
| Break reminder | Fires when the current stretch reaches the interval, then again every 5 min until a break starts or the timer stops. |
| Break interval | `task.break_interval_min` if set, else the global setting. |
| Ticks | The app re-checks reminders every 15 s, also while hidden in the tray (via `logic()`). |
| Heartbeat | Every tick stores `now` as the heartbeat. If a timer is running and the last heartbeat is more than 3 min old (the app was closed, or the PC slept), the timer is ended at the last heartbeat, and the stop is reported. |
| Quick start | Starts a session with a category and no task. Same one-active rule. |

## Architecture

- **`wt-core`**
  - `Session`, `SessionKind`.
  - `office_minutes(sessions, day, tz)` and `office_status(minutes, settings, weekday)`.
  - `Reminders`: a pure state machine. `tick(sessions, now, settings, …) -> Vec<Notice>`, with a `Notice` enum (TargetReached, SoftCeiling, BreakDue, BreakOver).
  - A `Notifier` port.
  - `SessionRepo`, holding the start/stop/meeting rules as provided methods, like `TaskRepo`.
- **`wt-storage`:** migration `003_tracking.sql`, which creates `session` and adds `task.break_interval_min`.
- **`wt-platform`:**
  - `DesktopNotifier` (notify-rust).
  - `MeetingHotkey` (global-hotkey). Like the tray, it is registered on the event-loop thread and forwards presses through `ShellHandle`.
- **`wt-ui-egui`:**
  - The status bar.
  - Start/Stop buttons on task rows.
  - Take break / Resume buttons.
  - Reminders are ticked from `logic()`.

Session table: `id, task_id NULL → task, category_id → category, kind, started_at, ended_at NULL` plus the sync columns.

## Success criteria

1. Start "Validation", then start "API": exactly two sessions exist, the first ended at the instant the second started, and only "API" is running.
2. With a simulated clock, 8h00m of office time emits `TargetReached` once, 9h00m emits `SoftCeiling` once, and 9h30m reports `Overtime { minutes: 30 }`. The running timer is never stopped.
3. Breaks are excluded from office time. A 23:00–01:00 session (local time) gives 60 min to each day.
4. Meeting toggle during an office task: the task's session ends and a meeting session runs. Ending the meeting starts the task's timer again at the same instant. A meeting started while idle resumes nothing. A task deleted during the meeting isn't resumed. The hotkey does the same as the button, and the rule holds across an app restart during the meeting.
5. 25 min of continuous work emits `BreakDue`. Ignoring it emits it again 5 min later. Taking a break, then 5 min passing, emits `BreakOver`.
6. On Friday, 3h of office work gives no 8h notice and `Overtime { minutes: 180 }`.
7. `cargo xtask ci` passes. A manual check confirms that a real Windows toast appears and the hotkey works while the window is hidden.

## As built (2026-10-22)

- A session's kind reuses `CategoryKind`, so there's no separate `SessionKind`. Office time is `office_time(sessions, day, tz, now) -> SignedDuration`. It takes `now` because a running session counts up to now.
- A task's own break interval is edited in its Rename form (top-level tasks only). Sub-tasks use their parent's interval. It is a tag-style drop-down beside the category: "Default break (Nm)" (no value of its own, so the Settings interval applies) or a preset of 15, 20, 25, 30, 45, 50, 60 or 90 minutes; a task's own value that is not a preset is listed too, so it is never lost.
- A unique index in SQLite also enforces "at most one running session".
- Toasts carry text only. The Resume button lives in the status bar.

## Decisions (2026-10-22)

- **Forgotten timer:** it's auto-stopped at the last heartbeat, which covers both an app closed with a timer running and a PC that slept. A banner explains the stop.
- **Quick start by category** is in this module.
- **Defaults:** the meeting hotkey is Ctrl+Alt+M, and the break reminder repeats every 5 min.
- **Test seams:** office clock functions, the `Reminders` state machine, `SessionRepo` on SQLite, and the status bar plus Start/Stop via kittest. Real toasts and the hotkey are checked by hand.

Success criterion 8: a timer left running across a 2 h gap with no heartbeat ends at the last heartbeat, not at "now".
