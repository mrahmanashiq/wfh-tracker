use wt_core::repo::RepoResult;
use wt_core::{
    Block, Category, CategoryKind, Clock, DistractionRepo, ForegroundWatcher, Notifier,
    NudgeCommand, NudgeWatcher, Sample, Settings, WatchContext, watch_context,
};

use crate::NudgeStore;

/// Looks at the foreground window during protected time, records
/// distractions and sends nudges. Call [`Nudges::poll`] every few seconds,
/// hidden or not. Window titles stay in memory: only rule ids reach the store.
#[derive(Default)]
pub struct Nudges {
    watcher: NudgeWatcher,
    /// Whether events left open by a previous run were ended.
    recovered: bool,
}

impl Nudges {
    pub fn poll(
        &mut self,
        store: &dyn NudgeStore,
        settings: &Settings,
        clock: &dyn Clock,
        foreground: &dyn ForegroundWatcher,
        notifier: &dyn Notifier,
    ) -> RepoResult<()> {
        let now = clock.now();
        let tz = clock.time_zone();
        if !self.recovered {
            // The app died with a distraction open: end it when the app was last alive.
            store.end_distractions(store.last_heartbeat()?.unwrap_or(now))?;
            self.recovered = true;
        }
        let blocks = store.blocks()?;
        let categories = store.list()?;
        let context = watch_context(store.running()?.as_ref(), &blocks, &categories, now, &tz);
        // Outside protected time the window isn't even looked at.
        let window = context.and_then(|_| foreground.foreground());
        let rules = store.rules()?;
        let commands = self.watcher.sample(Sample {
            now,
            context,
            window: window.as_ref(),
            rules: &rules,
            settings,
        });
        for command in commands {
            store.record(&command)?;
            if let NudgeCommand::Nudge { rule, context } = command {
                let name = rules
                    .iter()
                    .find(|r| r.meta.id == rule)
                    .map_or("A distraction", |r| r.pattern.as_str());
                notifier.notify(
                    &nudge_title(name, context, &blocks, &categories, clock),
                    "Back to it?",
                );
            }
        }
        Ok(())
    }

    /// The app is quitting: ends an open distraction now.
    pub fn stop(&mut self, store: &dyn DistractionRepo, clock: &dyn Clock) -> RepoResult<()> {
        for command in self.watcher.stop(clock.now()) {
            store.record(&command)?;
        }
        Ok(())
    }
}

/// `Facebook during your Learning block`, `… during learning time` or
/// `… during office time`.
fn nudge_title(
    name: &str,
    context: WatchContext,
    blocks: &[Block],
    categories: &[Category],
    clock: &dyn Clock,
) -> String {
    let category = |id| categories.iter().find(|c| c.meta.id == id);
    let local = clock.now().to_zoned(clock.time_zone());
    let learning_block = blocks
        .iter()
        .filter(|b| b.is_on(&local))
        .filter(|b| category(b.category_id).is_some_and(|c| c.kind == CategoryKind::Learning))
        .min_by_key(|b| b.start_min);
    match (context, learning_block) {
        (WatchContext::Learning, Some(block)) => {
            let block_name = match (block.label.as_str(), category(block.category_id)) {
                ("", Some(c)) => c.name.as_str(),
                (label, _) => label,
            };
            format!("{name} during your {block_name} block")
        }
        (WatchContext::Learning, None) => format!("{name} during learning time"),
        (WatchContext::Office, _) => format!("{name} during office time"),
    }
}
