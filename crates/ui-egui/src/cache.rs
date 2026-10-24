//! A read-through cache in front of the store, so drawing a frame doesn't
//! query the database. egui redraws on every mouse move; without this each
//! frame ran a dozen or more queries.
//!
//! [`CachedStore`] implements the same `wt-core` repository traits as the
//! store it wraps. Reads are kept until the data may have changed:
//!
//! - every write through it (from a screen, a reminder tick or a nudge poll)
//!   drops the whole cache, so the next read reloads;
//! - [`CachedStore::invalidate`] drops it on purpose (the app calls it when
//!   the screen changes and when the minute rolls over).
//!
//! Only stored rows are cached. Anything that depends on "now", such as a
//! running timer's elapsed time, is still worked out every frame from these
//! rows and the clock, so it keeps counting up.

use std::cell::{Cell, RefCell};

use jiff::Timestamp;
use wt_core::repo::{RepoError, RepoResult};
use wt_core::{
    Block, BlockDraft, BlockRepo, Category, CategoryRepo, DistractionEvent, DistractionEventRepo,
    DistractionRule, DistractionRuleRepo, Id, MeetingToggle, NewCategory, NudgeCommand, Period,
    Placement, Rgb, RuleDraft, Session, SessionRepo, Settings, SettingsRepo, Switch, Target,
    TargetRepo, Task, TaskRepo, TimerTarget, WatchContext,
};

/// Wraps a store and caches what it reads until something is written.
pub struct CachedStore<S> {
    inner: S,
    cache: RefCell<Cache>,
    revision: Cell<u64>,
}

/// Rows loaded with `since = from`. A later `since` is answered by filtering
/// them the way the store does: still open, or ended after `since`.
struct Window<T> {
    from: Timestamp,
    rows: Vec<T>,
}

#[derive(Default)]
struct Cache {
    settings: Option<Settings>,
    categories: Option<Vec<Category>>,
    tasks: Option<Vec<Task>>,
    running: Option<Option<Session>>,
    sessions: Option<Window<Session>>,
    targets: Option<Vec<Target>>,
    blocks: Option<Vec<Block>>,
    rules: Option<Vec<DistractionRule>>,
    distractions: Option<Window<DistractionEvent>>,
}

impl<S> CachedStore<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            cache: RefCell::default(),
            revision: Cell::new(0),
        }
    }

    /// The wrapped store. Writing to it directly bypasses the cache.
    pub fn get_ref(&self) -> &S {
        &self.inner
    }

    /// Counts the times the cache was dropped; it changes after every write.
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    /// Drops everything cached, so the next reads go to the store.
    pub fn invalidate(&self) {
        *self.cache.borrow_mut() = Cache::default();
        self.revision.set(self.revision.get().wrapping_add(1));
    }

    /// Runs a write, then drops the cache (also on failure, as a failed
    /// write may still have changed something).
    fn write<T>(&self, write: impl FnOnce(&S) -> RepoResult<T>) -> RepoResult<T> {
        let result = write(&self.inner);
        self.invalidate();
        result
    }

    /// The cached value in `slot`, or `load`'s result, cached when it worked.
    fn read<T: Clone>(
        &self,
        slot: impl Fn(&mut Cache) -> &mut Option<T>,
        load: impl FnOnce(&S) -> RepoResult<T>,
    ) -> RepoResult<T> {
        if let Some(hit) = slot(&mut self.cache.borrow_mut()).as_ref() {
            return Ok(hit.clone());
        }
        let value = load(&self.inner)?;
        *slot(&mut self.cache.borrow_mut()) = Some(value.clone());
        Ok(value)
    }

    /// Rows open or ended after `since`, from the cached window when it
    /// reaches back far enough, else loaded (and kept) from `since` on.
    fn read_window<T: Clone>(
        &self,
        since: Timestamp,
        slot: impl Fn(&mut Cache) -> &mut Option<Window<T>>,
        ended_at: impl Fn(&T) -> Option<Timestamp>,
        load: impl FnOnce(&S, Timestamp) -> RepoResult<Vec<T>>,
    ) -> RepoResult<Vec<T>> {
        if let Some(window) = slot(&mut self.cache.borrow_mut())
            .as_ref()
            .filter(|w| w.from <= since)
        {
            return Ok(window
                .rows
                .iter()
                .filter(|row| ended_at(row).is_none_or(|end| end > since))
                .cloned()
                .collect());
        }
        let rows = load(&self.inner, since)?;
        *slot(&mut self.cache.borrow_mut()) = Some(Window {
            from: since,
            rows: rows.clone(),
        });
        Ok(rows)
    }
}

impl<S: SettingsRepo> SettingsRepo for CachedStore<S> {
    fn load(&self) -> RepoResult<Settings> {
        self.read(|c| &mut c.settings, S::load)
    }

    fn save(&self, settings: &Settings) -> RepoResult<()> {
        self.write(|s| s.save(settings))
    }
}

impl<S: CategoryRepo> CategoryRepo for CachedStore<S> {
    fn list(&self) -> RepoResult<Vec<Category>> {
        self.read(|c| &mut c.categories, S::list)
    }

    fn create(&self, new: NewCategory) -> RepoResult<Category> {
        self.write(|s| s.create(new))
    }

    fn update(&self, id: Id, name: &str, color: Rgb) -> RepoResult<Category> {
        self.write(|s| s.update(id, name, color))
    }

    fn delete(&self, id: Id) -> RepoResult<()> {
        self.write(|s| CategoryRepo::delete(s, id))
    }

    fn seed_defaults(&self) -> RepoResult<bool> {
        self.write(S::seed_defaults)
    }
}

/// Writes go to the store's own provided methods too, so a backend that
/// overrides one (say, to make it one transaction) keeps doing so.
impl<S: TaskRepo> TaskRepo for CachedStore<S> {
    fn list_live(&self) -> RepoResult<Vec<Task>> {
        self.read(|c| &mut c.tasks, S::list_live)
    }

    fn get(&self, id: Id) -> RepoResult<Task> {
        self.list_live()?
            .into_iter()
            .find(|t| t.meta.id == id)
            .ok_or(RepoError::NotFound(id))
    }

    fn subtasks(&self, id: Id) -> RepoResult<Vec<Task>> {
        Ok(self
            .list_live()?
            .into_iter()
            .filter(|t| t.parent_id == Some(id))
            .collect())
    }

    fn insert(&self, parent_id: Option<Id>, title: &str, category_id: Id) -> RepoResult<Task> {
        self.write(|s| s.insert(parent_id, title, category_id))
    }

    fn write_title(&self, id: Id, title: &str) -> RepoResult<()> {
        self.write(|s| s.write_title(id, title))
    }

    fn write_category(&self, id: Id, category_id: Id) -> RepoResult<()> {
        self.write(|s| s.write_category(id, category_id))
    }

    fn write_done(&self, id: Id, done: bool) -> RepoResult<()> {
        self.write(|s| s.write_done(id, done))
    }

    fn write_break_interval(&self, id: Id, minutes: Option<u32>) -> RepoResult<()> {
        self.write(|s| s.write_break_interval(id, minutes))
    }

    fn remove(&self, id: Id) -> RepoResult<()> {
        self.write(|s| s.remove(id))
    }

    fn add(&self, title: &str, placement: Placement) -> RepoResult<Task> {
        self.write(|s| s.add(title, placement))
    }

    fn rename(&self, id: Id, title: &str) -> RepoResult<()> {
        self.write(|s| s.rename(id, title))
    }

    fn set_category(&self, id: Id, category_id: Id) -> RepoResult<()> {
        self.write(|s| s.set_category(id, category_id))
    }

    fn set_break_interval(&self, id: Id, minutes: Option<u32>) -> RepoResult<()> {
        self.write(|s| s.set_break_interval(id, minutes))
    }

    fn set_done(&self, id: Id, done: bool) -> RepoResult<()> {
        self.write(|s| s.set_done(id, done))
    }

    fn delete(&self, id: Id) -> RepoResult<()> {
        self.write(|s| TaskRepo::delete(s, id))
    }

    fn finish_chain_above(&self, id: Id) -> RepoResult<()> {
        self.write(|s| s.finish_chain_above(id))
    }

    fn finish_parent_if_complete(&self, parent_id: Id) -> RepoResult<bool> {
        self.write(|s| s.finish_parent_if_complete(parent_id))
    }
}

impl<S: SessionRepo> SessionRepo for CachedStore<S> {
    fn running(&self) -> RepoResult<Option<Session>> {
        self.read(|c| &mut c.running, S::running)
    }

    fn sessions_since(&self, since: Timestamp) -> RepoResult<Vec<Session>> {
        self.read_window(
            since,
            |c| &mut c.sessions,
            |s| s.ended_at,
            S::sessions_since,
        )
    }

    fn switch_to(
        &self,
        task_id: Option<Id>,
        category_id: Id,
        kind: wt_core::CategoryKind,
    ) -> RepoResult<Switch> {
        self.write(|s| s.switch_to(task_id, category_id, kind))
    }

    fn end_running(&self, at: Option<Timestamp>) -> RepoResult<Option<Session>> {
        self.write(|s| s.end_running(at))
    }

    fn beat(&self) -> RepoResult<()> {
        self.write(S::beat)
    }

    fn last_heartbeat(&self) -> RepoResult<Option<Timestamp>> {
        // Read once at start-up; never worth caching.
        self.inner.last_heartbeat()
    }

    fn start(&self, target: TimerTarget) -> RepoResult<Session> {
        self.write(|s| s.start(target))
    }

    fn stop(&self) -> RepoResult<Option<Session>> {
        self.write(S::stop)
    }

    fn take_break(&self) -> RepoResult<Switch> {
        self.write(S::take_break)
    }

    fn toggle_meeting(&self) -> RepoResult<MeetingToggle> {
        self.write(S::toggle_meeting)
    }

    fn recover(
        &self,
        now: Timestamp,
        max_gap: jiff::SignedDuration,
    ) -> RepoResult<Option<Session>> {
        self.write(|s| s.recover(now, max_gap))
    }
}

impl<S: TargetRepo> TargetRepo for CachedStore<S> {
    fn targets(&self) -> RepoResult<Vec<Target>> {
        self.read(|c| &mut c.targets, S::targets)
    }

    fn write_target(&self, category_id: Id, period: Period, minutes: u32) -> RepoResult<Target> {
        self.write(|s| s.write_target(category_id, period, minutes))
    }

    fn delete_target(&self, id: Id) -> RepoResult<()> {
        self.write(|s| s.delete_target(id))
    }

    fn set_target(&self, category_id: Id, period: Period, hours: u32) -> RepoResult<Target> {
        self.write(|s| s.set_target(category_id, period, hours))
    }
}

impl<S: BlockRepo> BlockRepo for CachedStore<S> {
    fn blocks(&self) -> RepoResult<Vec<Block>> {
        self.read(|c| &mut c.blocks, S::blocks)
    }

    fn insert_block(&self, draft: &BlockDraft) -> RepoResult<Block> {
        self.write(|s| s.insert_block(draft))
    }

    fn write_block(&self, id: Id, draft: &BlockDraft) -> RepoResult<Block> {
        self.write(|s| s.write_block(id, draft))
    }

    fn delete_block(&self, id: Id) -> RepoResult<()> {
        self.write(|s| s.delete_block(id))
    }

    fn add_block(&self, draft: &BlockDraft) -> RepoResult<Block> {
        self.write(|s| s.add_block(draft))
    }

    fn update_block(&self, id: Id, draft: &BlockDraft) -> RepoResult<Block> {
        self.write(|s| s.update_block(id, draft))
    }
}

impl<S: DistractionRuleRepo> DistractionRuleRepo for CachedStore<S> {
    fn rules(&self) -> RepoResult<Vec<DistractionRule>> {
        self.read(|c| &mut c.rules, S::rules)
    }

    fn rules_ever(&self) -> RepoResult<u64> {
        self.inner.rules_ever()
    }

    fn insert_rule(&self, draft: &RuleDraft) -> RepoResult<DistractionRule> {
        self.write(|s| s.insert_rule(draft))
    }

    fn set_rule_enabled(&self, id: Id, enabled: bool) -> RepoResult<()> {
        self.write(|s| s.set_rule_enabled(id, enabled))
    }

    fn delete_rule(&self, id: Id) -> RepoResult<()> {
        self.write(|s| s.delete_rule(id))
    }

    fn add_rule(&self, draft: &RuleDraft) -> RepoResult<DistractionRule> {
        self.write(|s| s.add_rule(draft))
    }

    fn seed_default_rules(&self) -> RepoResult<bool> {
        self.write(S::seed_default_rules)
    }
}

impl<S: DistractionEventRepo> DistractionEventRepo for CachedStore<S> {
    fn insert_distraction(
        &self,
        rule_id: Id,
        context: WatchContext,
        started_at: Timestamp,
    ) -> RepoResult<DistractionEvent> {
        self.write(|s| s.insert_distraction(rule_id, context, started_at))
    }

    fn end_distractions(&self, at: Timestamp) -> RepoResult<()> {
        self.write(|s| s.end_distractions(at))
    }

    fn distractions_since(&self, since: Timestamp) -> RepoResult<Vec<DistractionEvent>> {
        self.read_window(
            since,
            |c| &mut c.distractions,
            |e| e.ended_at,
            S::distractions_since,
        )
    }

    fn record(&self, command: &NudgeCommand) -> RepoResult<()> {
        // A `Nudge` writes nothing, so it keeps the cache.
        if matches!(command, NudgeCommand::Nudge { .. }) {
            return self.inner.record(command);
        }
        self.write(|s| s.record(command))
    }
}
