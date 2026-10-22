#![allow(clippy::unwrap_used)] // test code may unwrap (SPEC: Code Style)

use std::sync::Arc;

use jiff::{SignedDuration, Timestamp};
use wt_core::{
    Category, CategoryKind, CategoryRepo, FixedClock, NewCategory, Placement, RepoError, Rgb, Task,
    TaskRepo,
};
use wt_storage::SqliteStore;

fn at(ts: &str) -> Timestamp {
    ts.parse().unwrap()
}

struct World {
    store: SqliteStore,
    clock: Arc<FixedClock>,
    office: Category,
    learning: Category,
}

fn world() -> World {
    let clock = Arc::new(FixedClock::utc(at("2026-10-22T04:00:00Z")));
    let store = SqliteStore::open_in_memory(clock.clone()).unwrap();
    let office = store
        .create(NewCategory::new(
            "Office",
            CategoryKind::Office,
            Rgb(0, 0, 255),
        ))
        .unwrap();
    let learning = store
        .create(NewCategory::new(
            "Learning",
            CategoryKind::Learning,
            Rgb(0, 255, 0),
        ))
        .unwrap();
    World {
        store,
        clock,
        office,
        learning,
    }
}

impl World {
    fn top(&self, title: &str) -> Task {
        self.store
            .add(
                title,
                Placement::TopLevel {
                    category_id: self.office.meta.id,
                },
            )
            .unwrap()
    }

    fn sub(&self, title: &str, parent: &Task) -> Task {
        self.store
            .add(title, Placement::Under(parent.meta.id))
            .unwrap()
    }

    fn get(&self, task: &Task) -> Task {
        self.store.get(task.meta.id).unwrap()
    }
}

#[test]
fn subtasks_take_their_parents_category() {
    let w = world();
    let checkout = w
        .store
        .add(
            "Checkout",
            Placement::TopLevel {
                category_id: w.learning.meta.id,
            },
        )
        .unwrap();

    let api = w.sub("API", &checkout);

    assert_eq!(api.parent_id, Some(checkout.meta.id));
    assert_eq!(api.category_id, w.learning.meta.id);
}

#[test]
fn a_subtask_cannot_have_subtasks() {
    let w = world();
    let checkout = w.top("Checkout");
    let api = w.sub("API", &checkout);

    let result = w.store.add("Too deep", Placement::Under(api.meta.id));

    assert!(matches!(result, Err(RepoError::Invalid(_))));
}

#[test]
fn ticking_records_when_and_unticking_clears_it() {
    let w = world();
    let task = w.top("Read chapter 4");

    w.store.set_done(task.meta.id, true).unwrap();
    assert_eq!(w.get(&task).done_at, Some(at("2026-10-22T04:00:00Z")));

    w.store.set_done(task.meta.id, false).unwrap();
    assert_eq!(w.get(&task).done_at, None);
}

#[test]
fn parent_is_done_once_every_subtask_is_done() {
    let w = world();
    let checkout = w.top("Checkout");
    let validation = w.sub("Validation", &checkout);
    let api = w.sub("API", &checkout);

    w.store.set_done(validation.meta.id, true).unwrap();
    assert!(!w.get(&checkout).is_done());

    w.clock.advance(SignedDuration::from_hours(1));
    w.store.set_done(api.meta.id, true).unwrap();
    assert_eq!(w.get(&checkout).done_at, Some(at("2026-10-22T05:00:00Z")));
}

#[test]
fn unticking_a_subtask_reopens_the_parent() {
    let w = world();
    let checkout = w.top("Checkout");
    let api = w.sub("API", &checkout);
    w.store.set_done(api.meta.id, true).unwrap();
    assert!(w.get(&checkout).is_done());

    w.store.set_done(api.meta.id, false).unwrap();

    assert_eq!(w.get(&checkout).done_at, None);
}

#[test]
fn adding_a_subtask_to_a_done_parent_reopens_it() {
    let w = world();
    let checkout = w.top("Checkout");
    w.store.set_done(checkout.meta.id, true).unwrap();

    w.sub("Follow-up fix", &checkout);

    assert!(!w.get(&checkout).is_done());
}

#[test]
fn ticking_a_parent_by_hand_leaves_its_subtasks_alone() {
    let w = world();
    let checkout = w.top("Checkout");
    let api = w.sub("API", &checkout);

    w.store.set_done(checkout.meta.id, true).unwrap();

    assert!(w.get(&checkout).is_done());
    assert!(!w.get(&api).is_done());
}

#[test]
fn deleting_a_task_deletes_its_subtasks() {
    let w = world();
    let checkout = w.top("Checkout");
    let api = w.sub("API", &checkout);
    let other = w.top("Other");

    TaskRepo::delete(&w.store, checkout.meta.id).unwrap();

    assert_eq!(w.store.list_live().unwrap(), [other]);
    assert!(matches!(
        w.store.get(api.meta.id),
        Err(RepoError::NotFound(_))
    ));
}

#[test]
fn changing_a_parents_category_moves_its_subtasks_too() {
    let w = world();
    let checkout = w.top("Checkout");
    let api = w.sub("API", &checkout);

    w.store
        .set_category(checkout.meta.id, w.learning.meta.id)
        .unwrap();

    assert_eq!(w.get(&checkout).category_id, w.learning.meta.id);
    assert_eq!(w.get(&api).category_id, w.learning.meta.id);
}

#[test]
fn a_subtask_cannot_change_category_on_its_own() {
    let w = world();
    let checkout = w.top("Checkout");
    let api = w.sub("API", &checkout);

    let result = w.store.set_category(api.meta.id, w.learning.meta.id);

    assert!(matches!(result, Err(RepoError::Invalid(_))));
}

#[test]
fn titles_are_trimmed_and_must_be_1_to_200_characters() {
    let w = world();
    let task = w.top("  Checkout  ");
    assert_eq!(task.title, "Checkout");

    w.store.rename(task.meta.id, " Checkout v2 ").unwrap();
    assert_eq!(w.get(&task).title, "Checkout v2");

    let blank = Placement::TopLevel {
        category_id: w.office.meta.id,
    };
    assert!(matches!(
        w.store.add("   ", blank),
        Err(RepoError::Invalid(_))
    ));
    assert!(matches!(
        w.store.rename(task.meta.id, &"x".repeat(201)),
        Err(RepoError::Invalid(_))
    ));
    w.store.rename(task.meta.id, &"é".repeat(200)).unwrap();
}

#[test]
fn a_category_in_use_cannot_be_deleted() {
    let w = world();
    let checkout = w.top("Checkout");
    w.sub("API", &checkout);

    let result = CategoryRepo::delete(&w.store, w.office.meta.id);

    assert!(
        matches!(&result, Err(RepoError::InUse { tasks }) if tasks == &["Checkout"]),
        "{result:?}"
    );

    TaskRepo::delete(&w.store, checkout.meta.id).unwrap();
    CategoryRepo::delete(&w.store, w.office.meta.id).unwrap();
}

#[test]
fn deleting_the_last_open_subtask_completes_the_parent() {
    let w = world();
    let checkout = w.top("Checkout");
    let done = w.sub("Validation", &checkout);
    let open = w.sub("Dropped idea", &checkout);
    w.store.set_done(done.meta.id, true).unwrap();

    TaskRepo::delete(&w.store, open.meta.id).unwrap();

    assert!(w.get(&checkout).is_done());
}

#[test]
fn ticking_one_subtask_keeps_a_parent_that_was_ticked_by_hand() {
    let w = world();
    let checkout = w.top("Checkout");
    let validation = w.sub("Validation", &checkout);
    w.sub("API", &checkout);
    w.store.set_done(checkout.meta.id, true).unwrap();

    w.store.set_done(validation.meta.id, true).unwrap();

    assert!(w.get(&checkout).is_done());
}
