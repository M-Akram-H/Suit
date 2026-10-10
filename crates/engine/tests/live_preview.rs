//! The preview loop against real Manim, so ignored unless asked for:
//! `cargo test -p suit-engine -- --ignored`

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use suit_engine::{Preview, Session};
use suit_render::Sidecar;

/// Ticks like a user interface would until the preview shows the deck as it is.
fn settle(session: &mut Session) -> Duration {
    let started = Instant::now();
    while !session.preview_is_current() {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "no preview within a minute"
        );
        session.tick();
        thread::sleep(Duration::from_millis(5));
    }
    session.tick();
    started.elapsed()
}

#[test]
#[ignore = "needs Manim"]
fn edits_reach_the_preview_by_themselves() {
    let woken = Arc::new(AtomicUsize::new(0));
    let counter = woken.clone();
    let sidecar = Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR")));
    let mut session = Session::blank().with_preview(sidecar, move || {
        counter.fetch_add(1, Ordering::Relaxed);
    });

    let first = settle(&mut session);
    assert_eq!(session.preview, Preview::Ready);
    assert_eq!(
        session.snapshots.steps.len(),
        1,
        "an empty slide has only its start"
    );

    let circle = session.add_object("Circle").unwrap();
    assert!(!session.preview_is_current());
    let after_adding = settle(&mut session);
    let shown = session.snapshot().expect("the selected step has a picture");
    assert!(shown.boxes[&circle].on);
    assert_eq!(shown.drawn.len(), 1);
    let [left, _, right, _] = shown.boxes[&circle].rect;
    assert!(
        (left + 1.0).abs() < 0.01 && (right - 1.0).abs() < 0.01,
        "a unit circle at the origin"
    );

    // Dragging it two units to the right moves it in Manim's own picture too.
    assert!(session.place_at_creation(circle, [2.0, 0.0]));
    let after_moving = settle(&mut session);
    let [left, _, right, _] = session.snapshot().unwrap().boxes[&circle].rect;
    assert!(
        (left - 1.0).abs() < 0.01 && (right - 3.0).abs() < 0.01,
        "now centred on x = 2"
    );

    // And undoing brings it back.
    session.undo();
    settle(&mut session);
    let [left, ..] = session.snapshot().unwrap().boxes[&circle].rect;
    assert!((left + 1.0).abs() < 0.01);

    println!(
        "first preview {first:?}, after adding {after_adding:?}, after moving {after_moving:?}"
    );
    assert!(
        woken.load(Ordering::Relaxed) >= 4,
        "each preview wakes the interface"
    );
    assert!(
        after_moving < Duration::from_secs(2),
        "an edit shows up quickly once Manim is running"
    );
}
