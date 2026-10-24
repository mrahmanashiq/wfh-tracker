//! Whole-app frame timings and offscreen screenshots. Both are ignored by
//! default (they are slow and print rather than assert); run them with
//!
//! ```text
//! cargo test -p wt-ui-egui --release --test frame_bench -- --ignored --nocapture
//! ```
//!
//! The store is a temporary SQLite file seeded with five weeks of realistic
//! use, so frames hit the same storage path as the real app. Screenshots go
//! to `target/ui-shots/<WFH_SHOTS_LABEL or "latest">/`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code (SPEC: Code Style)

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use jiff::civil::Weekday;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp, ToSpan};
use wt_core::{
    Autostart, AutostartError, BlockDraft, BlockRepo, CategoryRepo, Clipboard, ClipboardError,
    Clock, DistractionEventRepo, DistractionRuleRepo, FixedClock, ForegroundWatcher,
    ForegroundWindow, Notifier, Period, Placement, SessionRepo, TargetRepo, TaskRepo, TimerTarget,
    WatchContext, Weekdays,
};
use wt_storage::SqliteStore;
use wt_ui_egui::{Services, UiFonts, WfhApp};

const SCREENS: [&str; 7] = [
    "Today",
    "Dashboard",
    "Planning",
    "Focus",
    "Reports",
    "Categories",
    "Settings",
];
const SIZE: egui::Vec2 = egui::vec2(1040.0, 700.0);
/// Thursday 22 Oct 2026, 15:30 UTC: mid-afternoon on a workday.
const NOW: &str = "2026-10-22T15:30:00Z";
/// Five weeks back, a Sunday.
const FIRST_DAY: &str = "2026-09-13";

struct Quiet;
impl Autostart for Quiet {
    fn is_enabled(&self) -> Result<bool, AutostartError> {
        Ok(false)
    }
    fn set_enabled(&self, _: bool) -> Result<(), AutostartError> {
        Ok(())
    }
}
impl Notifier for Quiet {
    fn notify(&self, _: &str, _: &str) {}
}
impl ForegroundWatcher for Quiet {
    fn foreground(&self) -> Option<ForegroundWindow> {
        None
    }
}
impl Clipboard for Quiet {
    fn set_text(&self, _: &str) -> Result<(), ClipboardError> {
        Ok(())
    }
}

fn db_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wfh-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.db"));
    let _ = std::fs::remove_file(&path);
    path
}

/// Moves a fixed clock forward to `to` (never back).
fn goto(clock: &FixedClock, to: Timestamp) {
    let gap = to.duration_since(clock.now());
    if gap.is_positive() {
        clock.advance(gap);
    }
}

/// Five weeks of workdays (tasks, meetings, breaks), evening learning,
/// weekend personal time, distractions, targets and blocks. Ends with a
/// task timer running at [`NOW`].
fn seed(store: &SqliteStore, clock: &FixedClock) {
    store.seed_defaults().unwrap();
    store.seed_default_rules().unwrap();
    let cats = store.list().unwrap();
    let id_of = |name: &str| cats.iter().find(|c| c.name == name).unwrap().meta.id;
    let (office, learning, family) = (id_of("Office"), id_of("Learning"), id_of("Family"));

    let projects = [
        (
            "Checkout redesign",
            [
                "Payment form validation",
                "Stripe API integration",
                "Order summary emails",
            ],
        ),
        (
            "Rate limiter",
            ["Token bucket", "Redis backend", "Load test"],
        ),
        (
            "Search revamp",
            ["Index schema", "Typo tolerance", "Facets"],
        ),
        ("Billing exports", ["CSV writer", "Scheduled jobs", "Docs"]),
    ];
    let mut work = Vec::new();
    for (title, subs) in projects {
        let parent = store
            .add(
                title,
                Placement::TopLevel {
                    category_id: office,
                },
            )
            .unwrap();
        for sub in subs {
            work.push(
                store
                    .add(sub, Placement::Under(parent.meta.id))
                    .unwrap()
                    .meta
                    .id,
            );
        }
    }
    for title in ["Review PR #482", "Write sprint notes", "Fix flaky CI job"] {
        work.push(
            store
                .add(
                    title,
                    Placement::TopLevel {
                        category_id: office,
                    },
                )
                .unwrap()
                .meta
                .id,
        );
    }
    let book = store
        .add(
            "Rust async book",
            Placement::TopLevel {
                category_id: learning,
            },
        )
        .unwrap();
    store
        .add(
            "Call parents",
            Placement::TopLevel {
                category_id: family,
            },
        )
        .unwrap();

    store.set_target(learning, Period::Week, 5).unwrap();
    store.set_target(family, Period::Week, 3).unwrap();
    store.set_target(office, Period::Month, 160).unwrap();
    for (start, end, category, label) in [
        (21 * 60, 22 * 60, learning, "Evening study"),
        (7 * 60, 7 * 60 + 30, family, ""),
    ] {
        store
            .add_block(&BlockDraft {
                category_id: category,
                weekdays: Weekdays::from_days(&[
                    Weekday::Sunday,
                    Weekday::Monday,
                    Weekday::Tuesday,
                    Weekday::Wednesday,
                    Weekday::Thursday,
                ]),
                start_min: start,
                end_min: end,
                label: label.to_owned(),
            })
            .unwrap();
    }

    let rules = store.rules().unwrap();
    let now: Timestamp = NOW.parse().unwrap();
    let mut day: jiff::civil::Date = FIRST_DAY.parse().unwrap();
    let mut n = 0usize;
    let at = |day: jiff::civil::Date, min: i64| {
        day.to_zoned(TimeZone::UTC).unwrap().timestamp() + SignedDuration::from_mins(min)
    };
    loop {
        let workday = !matches!(day.weekday(), Weekday::Friday | Weekday::Saturday);
        // Days differ a little: a later start, one block more or less.
        let mut min = 8 * 60 + 30 + i64::from(day.day() % 4) * 15;
        let blocks = 5 + usize::from(day.day().unsigned_abs() % 3);
        if workday {
            // Work blocks with breaks and a stand-up meeting.
            for block in 0..blocks {
                let t = at(day, min);
                if t >= now {
                    break;
                }
                goto(clock, t);
                store
                    .start(TimerTarget::Task(work[n % work.len()]))
                    .unwrap();
                n += 1;
                min += 45 + (n % 4) as i64 * 5;
                if block == 1 {
                    goto(clock, at(day, min));
                    store.toggle_meeting().unwrap();
                    min += 30;
                    goto(clock, at(day, min));
                    store.toggle_meeting().unwrap();
                    min += 20;
                }
                if at(day, min) >= now {
                    break;
                }
                goto(clock, at(day, min));
                store.take_break().unwrap();
                min += 5;
            }
            if at(day, min) < now {
                goto(clock, at(day, min));
                store.stop().unwrap();
            }
            if n.is_multiple_of(3) {
                let t = at(day, 11 * 60);
                if t < now {
                    let rule = rules[n % rules.len()].meta.id;
                    store
                        .insert_distraction(rule, WatchContext::Office, t)
                        .unwrap();
                    store
                        .end_distractions(t + SignedDuration::from_mins(7))
                        .unwrap();
                }
            }
        }
        // Evening learning or family time.
        let evening = at(day, 21 * 60);
        if evening < now {
            goto(clock, evening);
            if n.is_multiple_of(2) {
                store.start(TimerTarget::Task(book.meta.id)).unwrap();
            } else {
                store.start(TimerTarget::Category(family)).unwrap();
            }
            goto(clock, evening + SignedDuration::from_mins(50));
            store.stop().unwrap();
        }
        if at(day, 24 * 60) > now {
            break;
        }
        day = day.checked_add(1.day()).unwrap();
    }
    goto(clock, now);
    store.start(TimerTarget::Task(work[1])).unwrap();
    for id in work.iter().step_by(4) {
        store.set_done(*id, true).unwrap();
    }
    // A checklist (third level) under the running to-do, one item done.
    let checklist: Vec<_> = ["Webhook signatures", "Retry on 5xx", "Idempotency keys"]
        .into_iter()
        .map(|title| store.add(title, Placement::Under(work[1])).unwrap().meta.id)
        .collect();
    store.set_done(checklist[0], true).unwrap();
    store.beat().unwrap();
}

fn system_fonts() -> Option<UiFonts> {
    let read = |name: &str| std::fs::read(format!("C:/Windows/Fonts/{name}")).ok();
    Some(UiFonts {
        regular: read("segoeui.ttf")?,
        semibold: read("seguisb.ttf")?,
        emoji: read("seguiemj.ttf"),
    })
}

/// The whole app on a fresh store: five weeks of data when `seeded`, else
/// only the default categories (to show the empty states).
fn app(name: &str, seeded: bool) -> Harness<'static, WfhApp<SqliteStore>> {
    let clock = Arc::new(FixedClock::new(
        FIRST_DAY
            .parse::<jiff::civil::Date>()
            .unwrap()
            .to_zoned(TimeZone::UTC)
            .unwrap()
            .timestamp(),
        TimeZone::UTC,
    ));
    let store = SqliteStore::open(&db_path(name), clock.clone()).unwrap();
    if seeded {
        seed(&store, &clock);
    } else {
        store.seed_defaults().unwrap();
        goto(&clock, NOW.parse().unwrap());
    }
    Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let services = Services {
            clock: clock as Arc<dyn Clock>,
            autostart: Box::new(Quiet),
            notifier: Box::new(Quiet),
            foreground: Box::new(Quiet),
            fonts: system_fonts(),
            updater: None,
            clipboard: Box::new(Quiet),
        };
        WfhApp::new(&cc.egui_ctx, store, services, false).0
    })
}

fn open(h: &mut Harness<'_, WfhApp<SqliteStore>>, screen: &str) {
    h.get_by_role_and_label(Role::Button, screen).click();
    h.run_steps(4);
}

/// Average frame time per screen while the pointer moves, as egui repaints
/// on every mouse move.
#[test]
#[ignore = "benchmark: run with --ignored --nocapture"]
fn frame_times() {
    const FRAMES: usize = 300;
    let mut h = app("bench", true);
    h.run_steps(4);
    println!("frame_bench: {FRAMES} frames per screen, pointer moving every frame");
    let mut total = 0.0;
    for screen in SCREENS {
        open(&mut h, screen);
        let started = Instant::now();
        for i in 0..FRAMES {
            let x = 240.0 + (i % 60) as f32 * 12.0;
            let y = 80.0 + (i % 40) as f32 * 14.0;
            h.hover_at(egui::pos2(x, y));
            h.step();
        }
        let per_frame = started.elapsed().as_secs_f64() * 1e6 / FRAMES as f64;
        total += per_frame;
        println!("frame_bench: {screen:<11} {per_frame:>9.0} us/frame");
    }
    println!(
        "frame_bench: mean        {:>9.0} us/frame",
        total / SCREENS.len() as f64
    );
}

/// PNGs of every screen in light and dark, rendered offscreen with wgpu.
#[test]
#[ignore = "screenshots: run with --ignored"]
fn screenshots() {
    let label = std::env::var("WFH_SHOTS_LABEL").unwrap_or_else(|_| "latest".to_owned());
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/ui-shots")
        .join(label);
    std::fs::create_dir_all(&dir).unwrap();
    for (prefix, seeded) in [("", true), ("empty-", false)] {
        let mut h = app(&format!("shots-{seeded}"), seeded);
        h.set_render_cursor(false);
        h.run_steps(4);
        for (theme, suffix) in [(egui::Theme::Light, "light"), (egui::Theme::Dark, "dark")] {
            h.ctx.set_theme(theme);
            for screen in SCREENS {
                open(&mut h, screen);
                // Park the pointer on the sidebar's empty bottom so nothing shows hover.
                h.hover_at(egui::pos2(20.0, SIZE.y - 10.0));
                h.run_steps(30);
                let name = format!("{prefix}{}-{suffix}.png", screen.to_lowercase());
                save(&mut h, &dir.join(name));
            }
        }
        if seeded {
            today_states(&mut h, &dir);
        }
    }
}

/// The Today list's interactive states in both themes: a hovered row, a
/// selected row and a task open for editing. A sidebar hover rides along.
fn today_states(h: &mut Harness<'_, WfhApp<SqliteStore>>, dir: &std::path::Path) {
    for (theme, suffix) in [(egui::Theme::Light, "light"), (egui::Theme::Dark, "dark")] {
        h.ctx.set_theme(theme);
        open(h, "Today");
        let row = |h: &Harness<'_, WfhApp<SqliteStore>>, title: &str| h.get_by_label(title).rect();

        let hovered = row(h, "Idempotency keys");
        h.hover_at(hovered.center() + egui::vec2(200.0, 0.0));
        h.run_steps(30);
        save(h, &dir.join(format!("today-hover-{suffix}.png")));

        // A click on the row itself (not its checkbox) selects it.
        let selected = row(h, "Fix flaky CI job").center() + egui::vec2(200.0, 0.0);
        click_at(h, selected);
        h.hover_at(egui::pos2(20.0, SIZE.y - 10.0));
        h.run_steps(30);
        save(h, &dir.join(format!("today-selected-{suffix}.png")));

        h.hover_at(row(h, "Checkout redesign").center() + egui::vec2(300.0, 0.0));
        h.run_steps(30);
        h.get_by_label("Edit").click();
        h.hover_at(egui::pos2(20.0, SIZE.y - 10.0));
        h.run_steps(30);
        save(h, &dir.join(format!("today-edit-{suffix}.png")));
        h.get_by_label("Cancel").click();
        h.run_steps(4);

        h.hover_at(egui::pos2(100.0, 186.0));
        h.run_steps(30);
        save(h, &dir.join(format!("sidebar-hover-{suffix}.png")));
    }
}

/// A primary click at `pos`, as offscreen input events.
fn click_at(h: &mut Harness<'_, WfhApp<SqliteStore>>, pos: egui::Pos2) {
    h.hover_at(pos);
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        });
        h.step();
    }
}

fn save(h: &mut Harness<'_, WfhApp<SqliteStore>>, path: &std::path::Path) {
    h.render().unwrap().save(path).unwrap();
    println!("screenshot: {}", path.display());
}
