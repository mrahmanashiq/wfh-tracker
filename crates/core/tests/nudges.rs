#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use jiff::civil::Weekday;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use wt_core::{
    Block, Category, CategoryKind, DistractionRule, ForegroundWindow, Id, MatchOn, Meta,
    NudgeCommand, NudgeWatcher, RepoError, Rgb, RuleDraft, Sample, Session, Settings, WatchContext,
    Weekdays, watch_context,
};

fn ts(s: &str) -> Timestamp {
    s.parse().unwrap()
}

fn meta() -> Meta {
    let t = ts("2026-10-01T00:00:00Z");
    Meta {
        id: Id::new(),
        created_at: t,
        updated_at: t,
        deleted_at: None,
    }
}

fn rule(pattern: &str, match_on: MatchOn) -> DistractionRule {
    DistractionRule {
        meta: meta(),
        pattern: pattern.to_owned(),
        match_on,
        enabled: true,
    }
}

fn window(title: &str, process: &str) -> ForegroundWindow {
    ForegroundWindow {
        title: title.to_owned(),
        process: process.to_owned(),
    }
}

const START: &str = "2026-10-22T21:00:00Z";

/// Feeds the watcher one sample every `step` seconds for `secs` seconds
/// (starting at `from` seconds after 21:00), returning every command.
struct Run {
    watcher: NudgeWatcher,
    rules: Vec<DistractionRule>,
    settings: Settings,
}

impl Run {
    fn new() -> Self {
        Self {
            watcher: NudgeWatcher::default(),
            rules: vec![
                rule("Facebook", MatchOn::Title),
                rule("Reels", MatchOn::Title),
                rule("discord", MatchOn::Process),
            ],
            settings: Settings::default(),
        }
    }

    fn at(secs: i64) -> Timestamp {
        ts(START)
            .checked_add(SignedDuration::from_secs(secs))
            .unwrap()
    }

    fn sample(
        &mut self,
        secs: i64,
        context: Option<WatchContext>,
        w: Option<&ForegroundWindow>,
    ) -> Vec<NudgeCommand> {
        self.watcher.sample(Sample {
            now: Self::at(secs),
            context,
            window: w,
            rules: &self.rules,
            settings: &self.settings,
        })
    }

    /// Samples every 5 s over `[from, to]`, all in one context and window.
    fn span(
        &mut self,
        from: i64,
        to: i64,
        context: Option<WatchContext>,
        w: Option<&ForegroundWindow>,
    ) -> Vec<(i64, NudgeCommand)> {
        (from..=to)
            .step_by(5)
            .flat_map(|s| {
                self.sample(s, context, w)
                    .into_iter()
                    .map(move |c| (s, c))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn facebook(&self) -> Id {
        self.rules[0].meta.id
    }
}

const LEARNING: Option<WatchContext> = Some(WatchContext::Learning);
const OFFICE: Option<WatchContext> = Some(WatchContext::Office);

#[test]
fn a_distraction_nudges_once_after_the_grace_period_then_once_per_cooldown() {
    let mut run = Run::new();
    let fb = window("Reels | Facebook - Google Chrome", "chrome.exe");

    let commands = run.span(0, 300, LEARNING, Some(&fb));

    let rule = run.facebook();
    assert_eq!(
        commands,
        [
            (
                60,
                NudgeCommand::StartEvent {
                    rule,
                    context: WatchContext::Learning,
                    started_at: Run::at(0),
                }
            ),
            (
                60,
                NudgeCommand::Nudge {
                    rule,
                    context: WatchContext::Learning,
                }
            ),
            (
                180,
                NudgeCommand::Nudge {
                    rule,
                    context: WatchContext::Learning,
                }
            ),
            (
                300,
                NudgeCommand::Nudge {
                    rule,
                    context: WatchContext::Learning,
                }
            ),
        ]
    );
}

#[test]
fn looking_away_before_the_grace_period_records_nothing() {
    let mut run = Run::new();
    let fb = window("Facebook", "chrome.exe");
    let code = window("main.rs - wfh-tracker - Visual Studio Code", "Code.exe");

    let mut commands = run.span(0, 55, LEARNING, Some(&fb));
    commands.extend(run.span(60, 120, LEARNING, Some(&code)));
    commands.extend(run.span(125, 180, LEARNING, Some(&fb)));

    assert_eq!(commands, []);
}

#[test]
fn the_event_ends_when_the_window_stops_matching() {
    let mut run = Run::new();
    let fb = window("Facebook", "chrome.exe");
    let code = window("main.rs - Visual Studio Code", "Code.exe");

    run.span(0, 90, OFFICE, Some(&fb));
    let commands = run.span(95, 100, OFFICE, Some(&code));

    // It ends at the last sample that still matched, not at the first that didn't.
    assert_eq!(commands, [(95, NudgeCommand::EndEvent { at: Run::at(90) })]);
}

#[test]
fn the_event_ends_when_the_context_ends() {
    let mut run = Run::new();
    let fb = window("Facebook", "chrome.exe");

    run.span(0, 90, OFFICE, Some(&fb));
    let commands = run.span(95, 200, None, Some(&fb));

    assert_eq!(commands, [(95, NudgeCommand::EndEvent { at: Run::at(90) })]);
}

#[test]
fn the_event_ends_at_the_last_match_when_its_rule_is_disabled() {
    let mut run = Run::new();
    let fb = window("Facebook", "chrome.exe");

    run.span(0, 90, OFFICE, Some(&fb));
    run.rules[0].enabled = false;
    let commands = run.span(95, 100, OFFICE, Some(&fb));

    assert_eq!(commands, [(95, NudgeCommand::EndEvent { at: Run::at(90) })]);
}

#[test]
fn a_break_ends_the_event_at_the_last_match_even_in_a_learning_block() {
    let mut run = Run::new();
    let fb = window("Facebook", "chrome.exe");
    let learning = category(CategoryKind::Learning);
    let brk = category(CategoryKind::Break);
    let cats = [learning.clone(), brk.clone()];
    let blocks = [block(&learning)];
    let tz = TimeZone::UTC;

    // 21:00-21:01:30 in the learning block, then a break starts.
    let before = watch_context(None, &blocks, &cats, Run::at(0), &tz);
    run.span(0, 90, before, Some(&fb));
    let during = watch_context(Some(&running(&brk)), &blocks, &cats, Run::at(95), &tz);
    let commands = run.span(95, 300, during, Some(&fb));

    assert_eq!(during, None);
    assert_eq!(commands, [(95, NudgeCommand::EndEvent { at: Run::at(90) })]);
}

#[test]
fn stopping_the_watcher_ends_an_open_event() {
    let mut run = Run::new();
    let fb = window("Facebook", "chrome.exe");
    run.span(0, 90, OFFICE, Some(&fb));

    let commands = run.watcher.stop();

    // The last time Facebook was seen in front.
    assert_eq!(commands, [NudgeCommand::EndEvent { at: Run::at(90) }]);
    assert_eq!(run.watcher.stop(), []);
}

#[test]
fn switching_rule_or_context_starts_a_new_grace_period() {
    let mut run = Run::new();
    let fb = window("Facebook", "chrome.exe");
    let reels = window("Instagram Reels", "chrome.exe");

    run.span(0, 90, OFFICE, Some(&fb));
    // The Facebook event ends; Reels must last its own 60 s.
    let switched = run.span(95, 150, OFFICE, Some(&reels));
    assert_eq!(switched, [(95, NudgeCommand::EndEvent { at: Run::at(90) })]);

    // Reels wasn't recorded yet; in a new context its grace starts over.
    let commands = run.span(155, 400, LEARNING, Some(&reels));
    let reels_id = run.rules[1].meta.id;
    assert_eq!(
        commands[..2],
        [
            (
                215,
                NudgeCommand::StartEvent {
                    rule: reels_id,
                    context: WatchContext::Learning,
                    started_at: Run::at(155),
                }
            ),
            // The Facebook nudge was at 60 s, long past its cooldown.
            (
                215,
                NudgeCommand::Nudge {
                    rule: reels_id,
                    context: WatchContext::Learning,
                }
            ),
        ]
    );
}

#[test]
fn a_new_distraction_inside_the_cooldown_is_recorded_but_nudged_only_after_it() {
    let mut run = Run::new();
    let fb = window("Facebook", "chrome.exe");
    let code = window("Visual Studio Code", "Code.exe");

    run.span(0, 60, OFFICE, Some(&fb)); // nudge at 60
    run.span(65, 65, OFFICE, Some(&code));
    let commands = run.span(70, 180, OFFICE, Some(&fb));

    let rule = run.facebook();
    assert_eq!(
        commands,
        [
            (
                130,
                NudgeCommand::StartEvent {
                    rule,
                    context: WatchContext::Office,
                    started_at: Run::at(70),
                }
            ),
            (
                180,
                NudgeCommand::Nudge {
                    rule,
                    context: WatchContext::Office,
                }
            ),
        ]
    );
}

#[test]
fn a_gap_between_samples_ends_the_event_at_the_last_sample_and_starts_over() {
    let mut run = Run::new();
    let fb = window("Facebook", "chrome.exe");
    run.span(0, 90, OFFICE, Some(&fb));

    // The PC slept from 90 s to 1 h with Facebook in front.
    let commands = run.span(3600, 3655, OFFICE, Some(&fb));

    assert_eq!(
        commands,
        [(3600, NudgeCommand::EndEvent { at: Run::at(90) })]
    );
}

#[test]
fn rules_match_title_or_process_case_insensitively_and_only_when_enabled() {
    let fb = rule("facebook", MatchOn::Title);
    let discord = rule("Discord", MatchOn::Process);
    let mut off = rule("Instagram", MatchOn::Title);
    off.enabled = false;

    assert!(fb.matches(&window("(3) FACEBOOK - Chrome", "chrome.exe")));
    assert!(!fb.matches(&window("Notes", "facebook.exe")));
    assert!(discord.matches(&window("#general", "discord.exe")));
    assert!(!discord.matches(&window("Discord tips - YouTube", "chrome.exe")));
    assert!(!off.matches(&window("Instagram", "chrome.exe")));
}

#[test]
fn nothing_is_watched_without_a_context_or_a_window() {
    let mut run = Run::new();
    let fb = window("Facebook", "chrome.exe");

    let mut commands = run.span(0, 300, None, Some(&fb));
    commands.extend(run.span(305, 600, OFFICE, None));

    assert_eq!(commands, []);
}

#[test]
fn a_rule_pattern_is_trimmed_and_1_to_60_characters() {
    let ok = RuleDraft::new("  Twitter ", MatchOn::Title)
        .validated()
        .unwrap();
    assert_eq!(ok.pattern, "Twitter");

    for bad in ["   ", &"x".repeat(61)] {
        assert!(matches!(
            RuleDraft::new(bad, MatchOn::Title).validated(),
            Err(RepoError::Invalid(_))
        ));
    }
    assert!(
        RuleDraft::new(&"x".repeat(60), MatchOn::Process)
            .validated()
            .is_ok()
    );
}

#[test]
fn the_default_rules_are_the_four_from_the_spec() {
    let defaults: Vec<_> = RuleDraft::defaults()
        .into_iter()
        .map(|d| (d.pattern, d.match_on))
        .collect();
    assert_eq!(
        defaults,
        [
            ("Facebook".to_owned(), MatchOn::Title),
            ("Reels".to_owned(), MatchOn::Title),
            ("Shorts".to_owned(), MatchOn::Title),
            ("Instagram".to_owned(), MatchOn::Title),
        ]
    );
}

// ---- Watch context -------------------------------------------------------------

fn category(kind: CategoryKind) -> Category {
    Category {
        meta: meta(),
        name: format!("{kind:?}"),
        kind,
        color: Rgb(0, 0, 0),
    }
}

fn running(cat: &Category) -> Session {
    Session {
        meta: meta(),
        task_id: None,
        category_id: cat.meta.id,
        kind: cat.kind,
        started_at: ts("2026-10-22T20:30:00Z"),
        ended_at: None,
    }
}

/// Thursday 21:00-22:00.
fn block(cat: &Category) -> Block {
    Block {
        meta: meta(),
        category_id: cat.meta.id,
        weekdays: Weekdays::from_days(&[Weekday::Thursday]),
        start_min: 21 * 60,
        end_min: 22 * 60,
        label: String::new(),
    }
}

#[test]
fn the_watch_context_follows_blocks_and_running_timers() {
    let [office, meeting, learning, family, brk] = [
        CategoryKind::Office,
        CategoryKind::Meeting,
        CategoryKind::Learning,
        CategoryKind::Personal,
        CategoryKind::Break,
    ]
    .map(category);
    let cats = vec![
        office.clone(),
        meeting.clone(),
        learning.clone(),
        family.clone(),
        brk.clone(),
    ];
    let tz = TimeZone::UTC;
    let in_block = ts("2026-10-22T21:30:00Z");
    let after_block = ts("2026-10-22T22:30:00Z");
    let learning_block = [block(&learning)];
    let family_block = [block(&family)];
    let context = |run: Option<&Category>, blocks: &[Block], now| {
        watch_context(run.map(running).as_ref(), blocks, &cats, now, &tz)
    };

    assert_eq!(context(None, &learning_block, in_block), LEARNING);
    assert_eq!(context(None, &learning_block, after_block), None);
    assert_eq!(context(Some(&office), &learning_block, in_block), LEARNING);
    assert_eq!(context(Some(&learning), &[], after_block), LEARNING);
    assert_eq!(context(Some(&office), &[], after_block), OFFICE);
    assert_eq!(context(Some(&meeting), &[], after_block), OFFICE);
    assert_eq!(context(Some(&family), &family_block, in_block), None);
    assert_eq!(context(Some(&brk), &[], in_block), None);
    // No nudging during a break, even inside a learning block.
    assert_eq!(context(Some(&brk), &learning_block, in_block), None);
    assert_eq!(context(None, &[], in_block), None);
}
