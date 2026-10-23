//! Nudges: watching the foreground window during protected time and deciding
//! when to nudge and what to record. Pure: the app feeds it samples and
//! timestamps and carries out the [`NudgeCommand`]s it returns.

use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp, Zoned};

use crate::{Block, Category, CategoryKind, DistractionRule, Id, Session, Settings, WatchContext};

/// The window in front, as the OS reports it. Matched in memory only: it is
/// never stored or logged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForegroundWindow {
    pub title: String,
    /// The program's executable name, e.g. `chrome.exe`.
    pub process: String,
}

/// Reads the foreground window. `None` when there is none or it can't be read.
pub trait ForegroundWatcher {
    fn foreground(&self) -> Option<ForegroundWindow>;
}

/// What the user should be doing now, if it's protected time: a learning
/// block or learning timer (learning wins), else an office or meeting timer.
/// A break is never protected time, not even inside a learning block.
pub fn watch_context(
    running: Option<&Session>,
    blocks: &[Block],
    categories: &[Category],
    now: Timestamp,
    tz: &TimeZone,
) -> Option<WatchContext> {
    if running.is_some_and(Session::is_break) {
        return None;
    }
    let local = now.to_zoned(tz.clone());
    let learning_block = learning_block_on(blocks, categories, &local).is_some();
    if learning_block || running.is_some_and(|s| s.kind == CategoryKind::Learning) {
        Some(WatchContext::Learning)
    } else if running.is_some_and(Session::counts_as_office) {
        Some(WatchContext::Office)
    } else {
        None
    }
}

/// The block of a Learning-kind category that is on at `local`; the one that
/// started first when several overlap.
pub fn learning_block_on<'a>(
    blocks: &'a [Block],
    categories: &[Category],
    local: &Zoned,
) -> Option<&'a Block> {
    let is_learning = |id: Id| {
        categories
            .iter()
            .any(|c| c.meta.id == id && c.kind == CategoryKind::Learning)
    };
    blocks
        .iter()
        .filter(|b| b.is_on(local) && is_learning(b.category_id))
        .min_by_key(|b| b.start_min)
}

/// The nudge's title: `Facebook during your Learning block` (the block's
/// label, else its category's name), `... during learning time` or
/// `... during office time`.
pub fn nudge_title(
    name: &str,
    context: WatchContext,
    blocks: &[Block],
    categories: &[Category],
    now: Timestamp,
    tz: &TimeZone,
) -> String {
    match context {
        WatchContext::Learning => {
            let local = now.to_zoned(tz.clone());
            match learning_block_on(blocks, categories, &local) {
                Some(block) => {
                    let block_name = if block.label.is_empty() {
                        categories
                            .iter()
                            .find(|c| c.meta.id == block.category_id)
                            .map_or("", |c| c.name.as_str())
                    } else {
                        block.label.as_str()
                    };
                    format!("{name} during your {block_name} block")
                }
                None => format!("{name} during learning time"),
            }
        }
        WatchContext::Office => format!("{name} during office time"),
    }
}

/// One look at the foreground window.
pub struct Sample<'a> {
    pub now: Timestamp,
    /// `None` outside protected time.
    pub context: Option<WatchContext>,
    pub window: Option<&'a ForegroundWindow>,
    /// Live rules, oldest first; the first match wins.
    pub rules: &'a [DistractionRule],
    pub settings: &'a Settings,
}

/// What the app should do after a sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NudgeCommand {
    /// Record a distraction that began at `started_at`.
    StartEvent {
        rule: Id,
        context: WatchContext,
        started_at: Timestamp,
    },
    /// End the open distraction at `at`.
    EndEvent { at: Timestamp },
    /// Tell the user.
    Nudge { rule: Id, context: WatchContext },
}

/// Samples further apart than this don't count as continuous (the PC slept
/// or the app was stuck), so a match starts over.
const MAX_GAP: SignedDuration = SignedDuration::from_secs(30);

/// The rule matching right now, since when, when it was last seen, and
/// whether it's been recorded.
#[derive(Clone, Copy, Debug)]
struct Match {
    rule: Id,
    context: WatchContext,
    since: Timestamp,
    /// The last sample that matched: an event ends here, not at the first
    /// sample that didn't match.
    last_seen: Timestamp,
    recorded: bool,
}

/// Decides when a distraction is recorded and when to nudge.
#[derive(Debug, Default)]
pub struct NudgeWatcher {
    current: Option<Match>,
    last_sample: Option<Timestamp>,
    last_nudge: Option<Timestamp>,
}

impl NudgeWatcher {
    pub fn sample(&mut self, s: Sample<'_>) -> Vec<NudgeCommand> {
        let mut commands = Vec::new();
        if let Some(last) = self.last_sample
            && s.now.duration_since(last) > MAX_GAP
        {
            self.end(&mut commands);
        }
        self.last_sample = Some(s.now);

        let found = s.context.zip(s.window).and_then(|(context, window)| {
            s.rules
                .iter()
                .find(|r| r.matches(window))
                .map(|r| (r.meta.id, context))
        });
        let Some((rule, context)) = found else {
            self.end(&mut commands);
            return commands;
        };
        match self.current.as_mut() {
            Some(m) if m.rule == rule && m.context == context => m.last_seen = s.now,
            _ => {
                self.end(&mut commands);
                self.current = Some(Match {
                    rule,
                    context,
                    since: s.now,
                    last_seen: s.now,
                    recorded: false,
                });
            }
        }
        let Some(m) = self.current.as_mut() else {
            return commands;
        };
        if s.now.duration_since(m.since) < secs(s.settings.distraction_grace_secs) {
            return commands;
        }
        if !m.recorded {
            m.recorded = true;
            commands.push(NudgeCommand::StartEvent {
                rule,
                context,
                started_at: m.since,
            });
        }
        let cooldown = secs(s.settings.nudge_cooldown_secs);
        if self
            .last_nudge
            .is_none_or(|last| s.now.duration_since(last) >= cooldown)
        {
            self.last_nudge = Some(s.now);
            commands.push(NudgeCommand::Nudge { rule, context });
        }
        commands
    }

    /// The app stops watching (it's quitting): ends an open distraction at
    /// the last sample that matched.
    pub fn stop(&mut self) -> Vec<NudgeCommand> {
        let mut commands = Vec::new();
        self.end(&mut commands);
        self.last_sample = None;
        commands
    }

    /// Ends the current match; a recorded one ends when it was last seen.
    fn end(&mut self, commands: &mut Vec<NudgeCommand>) {
        if let Some(m) = self.current.take()
            && m.recorded
        {
            commands.push(NudgeCommand::EndEvent { at: m.last_seen });
        }
    }
}

fn secs(s: u32) -> SignedDuration {
    SignedDuration::from_secs(i64::from(s))
}
