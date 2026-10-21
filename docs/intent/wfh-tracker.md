# Intent: WFH Tracker

Confirmed with the owner on 2026-10-21 (interview-me).

- **Outcome:** A personal, open-source Rust desktop app that keeps office work near 8h/day and protects learning and personal time from Facebook/reels.
- **User:** The owner — a WFH software engineer who does office work through Remote Desktop Connection and runs Microsoft Teams on the local Windows PC.
- **Why now:** Office hours and reels are eating personal life; there is no visibility into where time goes.
- **v1 scope:**
  - Tasks with sub-features, done/undone, unfinished items carry over.
  - Manual start/stop timers (office work runs inside RDP, so automatic app tracking can't tell features apart).
  - Break reminder per task (default every 25 min, configurable).
  - Categories: office, meeting, learning, personal, family, and user-defined.
  - Office clock: breaks excluded; 8h reminder, 9h soft warning, overtime logged — never a hard stop.
  - Meeting button/hotkey adds a meeting to the office timeline.
  - Distraction nudge: during learning blocks, detect Facebook/reels in the local foreground window and nudge (no blocking).
  - Weekly/monthly time targets per category + scheduled blocks with reminders.
  - Desktop notifications, dashboard.
  - Plain-text daily office report with one-click copy (pasted into Teams manually); private learning report.
- **Success:** Daily report pasted in under 10 seconds; learning targets met; reel time trending down.
- **Constraints:**
  - Rust only, open source, Windows first, Mac support kept possible.
  - SQLite now, behind a storage layer; Neon (Postgres) or Mongo later for cross-device sync. Every record has a UUID and `updated_at` from day one.
  - Layered like storytold's Craft apps: core logic has no UI dependency; egui front end is swappable.
  - Must stay extensible — many features will be added later.
- **Out of scope (v1):** Hard site blocking; automatic Teams call detection (v2 — feasible because Teams is local); automatic app tracking for office work; Jira/timesheet integrations; cloud sync (designed for, not built); multiple users; mobile.
