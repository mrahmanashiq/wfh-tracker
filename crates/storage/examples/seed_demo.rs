//! Fills a fresh database with a believable day, for screenshots and trying
//! the UI without touching real data:
//!
//! ```text
//! cargo run -p wt-storage --example seed_demo -- <folder>
//! set WFH_TRACKER_DATA_DIR=<folder> && cargo run -p wfh-tracker
//! ```

use std::error::Error;
use std::sync::Arc;

use jiff::{SignedDuration, Timestamp};
use wt_core::{CategoryRepo, FixedClock, Placement, SessionRepo, TaskRepo, TimerTarget};
use wt_storage::SqliteStore;

fn main() -> Result<(), Box<dyn Error>> {
    let dir = std::env::args().nth(1).ok_or("usage: seed_demo <folder>")?;
    std::fs::create_dir_all(&dir)?;
    // Start the demo day 6h40m ago so a timer is running "now".
    let start = Timestamp::now() - SignedDuration::from_mins(400);
    let clock = Arc::new(FixedClock::new(start, jiff::tz::TimeZone::system()));
    let store = SqliteStore::open(
        &std::path::Path::new(&dir).join("wfh-tracker.db"),
        clock.clone(),
    )?;
    store.seed_defaults()?;
    let cats = store.list()?;
    let id_of = |name: &str| {
        cats.iter()
            .find(|c| c.name == name)
            .map(|c| c.meta.id)
            .ok_or(format!("no {name} category"))
    };
    let (office, learning, family) = (id_of("Office")?, id_of("Learning")?, id_of("Family")?);

    let checkout = store.add(
        "Checkout redesign",
        Placement::TopLevel {
            category_id: office,
        },
    )?;
    let validation = store.add(
        "Payment form validation",
        Placement::Under(checkout.meta.id),
    )?;
    let api = store.add("Stripe API integration", Placement::Under(checkout.meta.id))?;
    store.add("Order summary emails", Placement::Under(checkout.meta.id))?;
    let review = store.add(
        "Review PR #482 — rate limiter",
        Placement::TopLevel {
            category_id: office,
        },
    )?;
    store.add(
        "Rust async book, chapter 4",
        Placement::TopLevel {
            category_id: learning,
        },
    )?;
    store.add(
        "Call parents",
        Placement::TopLevel {
            category_id: family,
        },
    )?;

    let later = |minutes| clock.advance(SignedDuration::from_mins(minutes));
    store.start(TimerTarget::Task(validation.meta.id))?;
    later(95);
    store.set_done(validation.meta.id, true)?;
    store.take_break()?;
    later(10);
    store.start(TimerTarget::Task(review.meta.id))?;
    later(50);
    store.set_done(review.meta.id, true)?;
    store.toggle_meeting()?;
    later(45);
    store.start(TimerTarget::Task(api.meta.id))?;
    later(200);
    store.beat()?;

    println!("demo data written to {dir}");
    Ok(())
}
