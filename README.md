# WFH Tracker

A small desktop app, written in Rust, that keeps a work-from-home day near
8 hours and protects the time that belongs to you.

- **Today list** with tasks and sub-tasks, carried over until they're done.
- **One timer at a time** on any task, plus quick-start timers for personal time.
- **Office clock**: a reminder at 8 h, a soft warning at 9 h, and overtime shown, never a hard stop.
- **Breaks** every 25 minutes (configurable) and a **Meeting** button / `Ctrl+Alt+M`.
- Lives in the tray, starts at login, and **updates itself** from GitHub Releases.

Windows first; built to stay portable.

## Install

Download `WFH Tracker_<version>_x64-setup.exe` from the
[latest release](https://github.com/mrahmanashiq/wfh-tracker/releases/latest) and run it.
It installs for your user only (no admin rights). New versions are offered in the app.

## Develop

```bash
cargo run -p wfh-tracker          # run the app
cargo xtask ci                    # format, lint, tests, layering check
cargo run -p wt-storage --example seed_demo -- <folder>   # sample data
WFH_TRACKER_DATA_DIR=<folder> cargo run -p wfh-tracker    # run against it
```

The design and decisions are in [SPEC.md](SPEC.md) and the `SPEC-*.md` module specs.

## Release

```bash
cargo xtask release            # signed installer + latest.json in dist/
cargo xtask release --publish  # also creates the GitHub release
```

Releases are signed with a private key kept outside this repository
(`WFH_SIGNING_KEY`, default `~/.wfh-tracker-signing/updater.key`). The app only
installs updates whose signature matches `packaging/updater.key.pub`.

## License

MIT
