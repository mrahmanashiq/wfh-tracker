use wt_core::repo::RepoResult;
use wt_core::{
    Clock, DistractionEventRepo, ForegroundWatcher, Notifier, NudgeCommand, NudgeWatcher, Sample,
    Settings, nudge_title, watch_context,
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
                    &nudge_title(name, context, &blocks, &categories, now, &tz),
                    "Back to it?",
                );
            }
        }
        Ok(())
    }

    /// The app is quitting: ends an open distraction when it was last seen.
    pub fn stop(&mut self, store: &dyn DistractionEventRepo) -> RepoResult<()> {
        for command in self.watcher.stop() {
            store.record(&command)?;
        }
        Ok(())
    }
}
