# Spec: WFH Tracker

Status: **approved 2026-10-21** · Intent: [docs/intent/wfh-tracker.md](docs/intent/wfh-tracker.md)

## Objective

A personal desktop app that answers three questions every day:

1. **Did I do my 8 hours — and not 10?** Office time is tracked against an 8h target with a 9h soft ceiling.
2. **What did I actually do?** A plain-text office report ready to paste into Teams.
3. **Did I protect my own time?** Learning/personal targets are met, and Facebook/reels don't eat scheduled learning blocks.

User: one person, local machine, Windows first. Office work happens inside Remote Desktop, so all office tracking is manual (start/stop). The app is open source.

### User stories (v1)

- As the user, I create a **task** (e.g. "Checkout feature") with **sub-tasks** ("Payment form validation"), assign a **category**, and mark each done/undone. Undone items still show the next day.
- I press **Start** on a (sub-)task; starting another stops the current one. Only one timer runs at a time.
- Every **N minutes** (default 25, set per task or globally) I get a **break reminder**. Taking a break pauses work time; break time never counts toward office hours.
- I press **Meeting** (button or global hotkey) when a Teams call starts: the current office task pauses and a meeting session starts. Ending the meeting offers to resume the previous task.
- At **8h** office time I get "Office day complete — wrap up". At **9h** a soft warning. Time beyond 9h is logged as **overtime**. Nothing is ever force-stopped.
- I define **scheduled blocks** (e.g. Mon–Fri 21:00–22:00 = Learning). When a block starts I get a reminder to start a timer.
- During a learning block, a running learning session, or a running **office/meeting** session, if the **foreground window** matches a distraction rule (Facebook, reels, …) for more than a grace period, I get a **nudge** notification, and the distracted minutes are recorded.
- I set **targets** per category per week or month (e.g. Learning 5h/week) and see progress on the **dashboard**.
- At day end I click **Copy office report** and paste it into Teams. A separate **learning report** is for me only.

### Domain rules

| Rule | Value (default) |
|---|---|
| Day boundary | Local midnight; sessions crossing midnight are split per day in reports |
| Office time | Sum of `office` + `meeting` sessions for the local day; `break` excluded |
| Office target / soft ceiling | 8h / 9h (configurable) |
| Break interval / length | 25 min / 5 min (configurable globally and per task) |
| Concurrent timers | Exactly one active session at a time |
| Distraction grace / nudge cooldown | 60 s / 2 min |
| Week start | Sunday (configurable) |
| Workdays / weekend | Sun–Thu / Fri–Sat (configurable) |
| Office time on a weekend day | Logged in full as overtime (no 8h target) |

### Office report format (plain text)

```
Office report — Tue 21 Oct 2026 — 7h 45m (target 8h)

Checkout
  ✅ Payment form validation — 2h 10m
  ⏳ API integration — 3h 05m
Meetings — 3 (1h 20m)
Overtime — none
```

## Tech Stack

| Concern | Choice | Version (crates.io, 2026-10-21) |
|---|---|---|
| Language | Rust, edition 2024, stable toolchain | — |
| UI | `eframe` / `egui` (same family as storytold's Craft apps) | 0.37 |
| UI tests | `egui_kittest` | 0.37 |
| Storage | `rusqlite` (bundled SQLite) + `rusqlite_migration` | 0.40 / 2.6 |
| IDs | `uuid` v7 (time-ordered, sync-friendly) | 1.28 |
| Time & time zones | `jiff` | 0.2 |
| Notifications | `notify-rust` (Windows toast, macOS notification center) | 4.18 |
| Tray icon | `tray-icon` (app keeps running when window is closed) | 0.26 |
| Global hotkey | `global-hotkey` (Meeting toggle) | 0.9 |
| Foreground window | `active-win-pos-rs` (title + process name) | 0.11 |
| Clipboard | `arboard` | 3.7 |
| Errors / logging | `thiserror` / `tracing` | 2.0 / 0.1 |
| Autostart at login | `auto-launch` (per-user `HKCU\...\Run` key, no admin rights) | 0.6 |
| Data dir | `directories` (`%APPDATA%\wfh-tracker\data`, `~/Library/Application Support/wfh-tracker`) | 6.0 |

Versions are re-checked when each crate is first added. Adding any dependency not in this table is an **Ask first** item.

## Capability Map

This is the module breakdown. Approve it together with this spec. Each module after `foundation` then gets its own `SPEC-<id>.md`, written in build order.

| Module id | Responsibility | Depends on |
|---|---|---|
| `foundation` | Workspace layout, `Clock` trait, ID/timestamp conventions, `Repository` traits, SQLite + migrations, app shell (window + tray + autostart), `cargo xtask ci` | — |
| `tasks` | Tasks, sub-tasks, categories, done/undone, carry-over | foundation |
| `tracking` | Timer sessions, one-active rule, break reminders, meeting toggle + hotkey, office clock (8h / 9h / overtime) | tasks |
| `reports` | Daily office report and learning report as plain text, copy to clipboard | tracking |
| `planning` | Weekly/monthly targets per category, scheduled blocks, block-start reminders | tracking |
| `nudges` | Foreground-window watcher (learning + office contexts), distraction rules, nudge notifications, distraction minutes | tracking, planning |
| `dashboard` | Today view, week/month progress vs targets, office timeline, distraction summary | reports, planning, nudges |

Build order: `foundation → tasks → tracking → reports → planning → nudges → dashboard`

Each module ships its own UI screen, so the app is usable after every module. `dashboard` is the summary view built last.

**v2 slots (not built, the architecture leaves room):** `teams-detect` (automatic meeting from local Teams call), `sync` (Neon/Mongo `Repository` implementations), `mcp` (agents query "what did I do today?"), `blocking` (hard site blocking).

## Architecture

```text
wt-core        domain types + rules (sessions, office clock, targets, reports). No I/O, no UI.
               Defines traits: Clock, Repository (per aggregate), Notifier, ForegroundWatcher.
wt-storage     SQLite implementation of the Repository traits + migrations.
wt-platform    OS adapters: notifications, tray, global hotkey, foreground window, clipboard.
wt-ui-egui     egui front end. Talks to wt-core services only.
apps/wfh-tracker   binary: wires concrete adapters into core services.
xtask          `cargo xtask ci` — fmt, clippy, tests, layering check.
```

Dependency direction: `wt-core ← wt-storage, wt-platform, wt-ui-egui ← apps/wfh-tracker`. `wt-core` depends on none of them. `xtask ci` fails if `wt-core` gains a dependency on egui, rusqlite, or an OS crate. This rule is what makes swapping the database (Neon) or the UI possible later without rewrites.

### Data model (sync-ready from day one)

Every table has: `id TEXT PRIMARY KEY` (UUID v7), `created_at`, `updated_at` (UTC, RFC 3339), `deleted_at` (soft delete, so a future sync can propagate deletions).

| Table | Key fields |
|---|---|
| `category` | name, kind (`office` · `meeting` · `learning` · `personal` · `break` · `custom`), color |
| `task` | parent_id (null = top-level), title, category_id, status (`open` · `done`), break_interval_min (nullable), sort_order |
| `session` | task_id (nullable for meeting/break), category_id, kind, started_at, ended_at (null = running) |
| `target` | category_id, period (`week` · `month`), minutes |
| `block` | category_id, weekdays bitmask, start_local, end_local, label |
| `distraction_rule` | pattern, match_on (`title` · `process`), enabled |
| `distraction_event` | rule_id, context (`learning` · `office`), started_at, ended_at |
| `setting` | key, value |

Privacy: window titles are only matched in memory. Only the matched **rule id** and its duration are stored, never raw titles.

## Commands

```bash
cargo run -p wfh-tracker                                  # run the app
cargo test --workspace                                    # all tests
cargo clippy --workspace --all-targets -- -D warnings     # lint
cargo fmt --all --check                                   # format check
cargo xtask ci                                            # everything above + layering check
cargo build --release -p wfh-tracker                      # release build
```

## Project Structure

```text
wfh-tracker/
├── Cargo.toml              workspace
├── SPEC.md                 this file (+ SPEC-<module>.md per module)
├── crates/
│   ├── core/               wt-core
│   ├── storage/            wt-storage (src/migrations/*.sql)
│   ├── platform/           wt-platform (src/windows.rs, src/macos.rs behind cfg)
│   └── ui-egui/            wt-ui-egui
├── apps/wfh-tracker/       binary crate
├── xtask/                  CI / layering check
├── docs/intent/            confirmed intents
└── tasks/                  plan.md, todo.md (from planning phase)
```

Unit tests live next to code in `#[cfg(test)] mod tests`. Cross-crate integration tests live in each crate's `tests/` directory.

## Code Style

`rustfmt` defaults. Clippy is clean with `-D warnings`. No `unwrap()`/`expect()` outside tests and `main`. Errors use `thiserror` per crate. Time is always injected through `Clock`, never `Timestamp::now()` inside core logic.

```rust
/// Office time for one local day: office + meeting sessions, breaks excluded.
pub fn office_minutes(sessions: &[Session], day: Date, tz: &TimeZone) -> i64 {
    sessions
        .iter()
        .filter(|s| matches!(s.kind, SessionKind::Office | SessionKind::Meeting))
        .map(|s| s.minutes_within(day, tz))
        .sum()
}

pub enum OfficeStatus { UnderTarget, TargetReached, SoftCeiling, Overtime { minutes: i64 } }
```

Naming: crates `wt-*`, types `PascalCase`, functions `snake_case`, one aggregate per module file (`task.rs`, `session.rs`).

## Testing Strategy

| Level | What | How |
|---|---|---|
| Unit (wt-core) | Office clock thresholds, midnight split, break scheduling, target progress, report text | Fake `Clock`; table-driven tests; report text compared against expected snapshot strings |
| Storage (wt-storage) | Migrations, CRUD, soft delete, `updated_at` bumps | In-memory SQLite (`:memory:`) |
| UI (wt-ui-egui) | Start/stop, meeting toggle, report copy button | `egui_kittest` |
| Platform | Notifications, hotkey, foreground watcher | Traits with fakes in tests; manual checklist per OS release |

Every rule in the *Domain rules* table has at least one unit test. Bugs get a failing test first.

## Boundaries

- **Always:** run `cargo xtask ci` before commits; keep `wt-core` free of I/O and UI; inject time via `Clock`; give every new table `id/created_at/updated_at/deleted_at`; work on feature branches.
- **Ask first:** adding a dependency not in the Tech Stack table; changing the DB schema after a migration has shipped; changing the domain-rule defaults; anything that needs admin rights (hosts file, services).
- **Never:** store raw window titles or browser URLs; hard-block apps or sites in v1; send data off the machine; commit secrets; skip or delete failing tests to go green.

## Success Criteria

1. Start → stop on a sub-task creates exactly one session row; starting another task closes the running session in the same transaction.
2. With a fake clock: 8h00m office time emits `TargetReached` once, 9h00m emits `SoftCeiling` once, 9h30m reports `Overtime { minutes: 30 }`. The timer is never stopped automatically.
3. Break sessions are excluded from office time. A session crossing midnight is split correctly between the two days.
4. Pressing the Meeting hotkey during an office task pauses it, records a meeting session, and offers to resume the task when the meeting ends.
5. With a fake foreground watcher returning "Reels | Facebook" for more than 60 s during a learning block: one nudge, then none for 2 min, and a `distraction_event` with the right duration is stored. No title text is stored.
6. "Copy office report" puts text matching the agreed format on the clipboard. Pasted into Teams, it keeps its line breaks.
7. Target progress for "Learning 5h/week" is correct across week boundaries (Sunday start). On Fri/Sat there's no 8h reminder, and any office time counts as overtime.
8. Closing the window keeps the app in the tray, and reminders still fire. With autostart on, the app starts minimized to the tray at Windows login.
9. `cargo xtask ci` passes on Windows. The macOS build compiles (no Mac-specific features required in v1).
10. A distraction during an office session nudges you and is recorded with context `office`. The office report shows the distraction minutes as a separate line, for you only. It isn't included in the copied text unless you turn that on.

## Decisions (2026-10-21)

- Rust 1.99.1 is installed. License: **MIT**. Name: **wfh-tracker**.
- Autostart at login: **yes** (per-user, no admin rights).
- Default distraction rules: Facebook, Reels, YouTube Shorts, Instagram.
- Nudges are active during learning **and** office time.
- The week starts on **Sunday**. Workdays are Sun–Thu; the weekend is Fri–Sat.

## Open Questions

None right now. Weekend office time is assumed to count fully as overtime (see Domain rules). Correct that if it's wrong.
