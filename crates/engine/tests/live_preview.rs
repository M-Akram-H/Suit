//! The preview loop against real Manim, so ignored unless asked for:
//! `cargo test -p suit-engine -- --ignored`

use std::{
    path::Path,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use suit_core::{Command, Raw, RawIface, Time};
use suit_engine::{Preview, Session};
use suit_render::{Prober, Sidecar, Watch};

/// Manim runs one at a time per cache directory, so tests that time it must not overlap.
fn alone() -> MutexGuard<'static, ()> {
    static TURN: Mutex<()> = Mutex::new(());
    TURN.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Ticks like a user interface would until the preview shows the deck as it is.
fn settle(session: &mut Session) -> Duration {
    let started = Instant::now();
    // What the session says about its preview is as of its last tick.
    session.tick();
    let stopped =
        |session: &Session| matches!(session.preview, Preview::Failed(_) | Preview::Stopped);
    while !session.preview_is_current() && !stopped(session) {
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
    let _alone = alone();
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

#[test]
#[ignore = "needs Manim"]
fn a_slide_can_be_recorded_for_playing() {
    let _alone = alone();
    let sidecar = Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR")));
    let mut session = Session::sample().with_preview(sidecar, || {});
    // Only what is asked for, so that the test can tell what was.
    session.record_ahead_of_time(false);
    session.select(1, None);
    assert!(session.clip().is_none());
    session.record_slide();
    assert!(session.is_recording());

    let started = Instant::now();
    while session.clip().is_none() {
        assert_eq!(session.recording_problem(), None);
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "no recording within two minutes"
        );
        session.tick();
        thread::sleep(Duration::from_millis(5));
    }
    println!("recorded slide 2 in {:?}", started.elapsed());
    assert!(!session.is_recording());
    let report = &session.checked.slides[1];
    let frames = session.clip().unwrap().frames.len() as u32;
    assert_eq!(
        frames,
        report.frames + report.gap_frames,
        "Manim plays what the checker predicts"
    );

    // The recording belongs to the slide and to the deck as it was.
    session.select(2, None);
    assert!(session.clip().is_none());
    session.select(1, None);
    assert!(session.clip().is_some());
    session.add_object("Circle");
    assert!(
        session.clip().is_none(),
        "an edit makes the recording stale"
    );
}

/// Ticks until `done`, for at most `limit`.
fn until(session: &mut Session, limit: Duration, done: impl Fn(&Session) -> bool) -> Duration {
    let started = Instant::now();
    while !done(session) {
        assert!(started.elapsed() < limit, "not within {limit:?}");
        session.tick();
        thread::sleep(Duration::from_millis(5));
    }
    started.elapsed()
}

#[test]
#[ignore = "needs Manim"]
fn slides_are_recorded_ahead_and_an_edit_cuts_in() {
    let _alone = alone();
    let sidecar = Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR")));
    let mut session = Session::sample().with_preview(sidecar, || {});
    settle(&mut session);

    // Nobody asked, and the selected slide is recorded all the same.
    let minute = Duration::from_secs(60);
    let first = until(&mut session, minute, |session| session.clip_of(0).is_some());
    println!("the first slide was recorded {first:?} after the preview");

    // The others are on their way. A change to the last slide does not wait for them, and
    // takes nothing away from the slides before it.
    session.select(5, None);
    session.add_object("Circle");
    assert!(session.clip_of(0).is_some());
    let cut_in = settle(&mut session);
    println!("an edit reached the preview in {cut_in:?} while slides were being recorded");
    assert!(
        cut_in < Duration::from_secs(2),
        "recording gives way: {cut_in:?}"
    );

    until(&mut session, 2 * minute, |session| {
        (0..6).all(|slide| session.clip_of(slide).is_some())
    });
    for (slide, report) in session.checked.slides.iter().enumerate() {
        assert_eq!(session.recording_problem_of(slide), None);
        let frames = session.clip_of(slide).unwrap().frames.len() as u32;
        assert_eq!(
            frames,
            report.frames + report.gap_frames,
            "slide {}",
            slide + 1
        );
    }

    // Undoing the change finds the last slide's old recording gone but the rest in place,
    // and a title is not part of what a slide looks like.
    session.select(2, None);
    session.edit_slide(|slide| slide.title = "Renamed".to_owned());
    assert!((0..6).all(|slide| session.clip_of(slide).is_some()));
    session.select(2, Some(0));
    session.delete_step();
    assert!(session.clip_of(1).is_some() && session.clip_of(2).is_none());
    session.undo();
    assert!(
        session.clip_of(2).is_some(),
        "a recording is of the slide as it was, and it is that again"
    );
}

#[test]
#[ignore = "needs Manim"]
fn a_manim_that_never_finishes_is_stopped_by_hand() {
    let _alone = alone();
    let sidecar = Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR")));
    let mut session = Session::blank().with_preview(sidecar, || {});
    settle(&mut session);

    let python = "import time\ntime.sleep(600)".to_owned();
    let iface = RawIface {
        duration: Time::from_secs(1.0),
        ..RawIface::default()
    };
    session.insert_step(Command::Raw(Raw { python, iface }));
    let patience = Instant::now();
    while patience.elapsed() < Duration::from_millis(600) {
        session.tick();
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(session.preview, Preview::Running);

    session.stop_preview();
    let ten = Duration::from_secs(10);
    until(&mut session, ten, |session| {
        session.preview == Preview::Stopped
    });

    // With the step gone the deck is what it was, which the session still has a picture
    // of; after another change, a new Manim shows it.
    session.delete_step();
    settle_all(&mut session);
    assert_eq!(session.preview, Preview::Ready);
    let circle = session.add_object("Circle").unwrap();
    settle_all(&mut session);
    assert_eq!(session.preview, Preview::Ready);
    assert!(session.snapshot().unwrap().boxes[&circle].on);
}

#[test]
#[ignore = "needs Manim"]
fn manim_makes_every_object_as_it_is_first_added() {
    let _alone = alone();
    let sidecar = Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR")));
    let mut session = Session::blank().with_preview(sidecar, || {});
    session.record_ahead_of_time(false);
    let kinds: Vec<String> = session.sig.ctors.keys().map(|c| c.to_string()).collect();
    session.add_object("Circle").unwrap();
    for ctor in kinds.iter().filter(|ctor| *ctor != "ImageMobject") {
        let var = session.add_object(ctor).expect(ctor);
        settle(&mut session);
        assert_eq!(session.preview, Preview::Ready, "{ctor}");
        assert_eq!(session.checked.diagnostics, [], "{ctor}");
        if ctor != "ValueTracker" {
            let shown = session.snapshot().expect("the step is shown");
            assert!(shown.boxes[&var].on, "{ctor} is on the slide");
        }
    }
    let group = session.add_group(&[session.objects()[0].var, session.objects()[1].var]);
    settle(&mut session);
    assert_eq!(session.preview, Preview::Ready);
    session.duplicate(group.unwrap()).unwrap();
    settle(&mut session);
    assert_eq!(session.preview, Preview::Ready);
    assert_eq!(session.checked.diagnostics, []);
}

/// Ticks until Manim has shown every slide of the deck as it is.
fn settle_all(session: &mut Session) {
    settle(session);
    let minute = Duration::from_secs(60);
    until(session, minute, |session| {
        session.preview != Preview::Running
    });
}

/// What a Manim that has never seen the deck says it looks like.
fn from_scratch(session: &Session) -> String {
    let sidecar = Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR")));
    let anew = Prober::new(sidecar)
        .probe(
            &session.deck,
            &session.sig,
            &session.checked,
            Watch::patient(),
        )
        .expect("Manim runs");
    assert_eq!(anew.error, None);
    format!("{:?}", anew.steps)
}

#[test]
#[ignore = "needs Manim"]
fn the_preview_after_many_edits_is_the_preview_of_the_deck() {
    let _alone = alone();
    let sidecar = Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR")));
    let mut session = Session::sample().with_preview(sidecar, || {});
    session.record_ahead_of_time(false);
    settle_all(&mut session);
    assert_eq!(session.preview, Preview::Ready);
    assert_eq!(
        format!("{:?}", session.snapshots.steps),
        from_scratch(&session)
    );

    // Edits all over the deck, each built on what Manim kept from the ones before.
    session.select(5, None);
    let star = session.add_object("Star").unwrap();
    settle_all(&mut session);
    session.select(1, Some(3));
    let ring = session.add_object("Circle").unwrap();
    assert!(session.place_at_creation(ring, [3.0, 2.0]));
    settle_all(&mut session);
    session.select(3, None);
    session.add_object("Text");
    settle_all(&mut session);
    session.undo();
    settle_all(&mut session);
    session.select(2, Some(0));
    session.insert_move(star, [0.0, 0.0], true);
    // That step is before the star is made: Manim stops there, and says so.
    settle_all(&mut session);
    assert!(
        matches!(session.preview, Preview::Failed(_)),
        "{:?}",
        session.preview
    );
    session.undo();
    settle_all(&mut session);
    session.select(4, None);
    session.move_slide(false);
    settle_all(&mut session);
    session.undo();
    session.select(0, Some(1));
    session.edit_slide(|slide| slide.title = "Another title".to_owned());
    settle_all(&mut session);
    session.select(2, None);
    session.add_slide();
    session.add_object("Triangle");
    settle_all(&mut session);

    assert_eq!(session.preview, Preview::Ready);
    assert_eq!(session.checked.diagnostics, []);
    assert_eq!(
        format!("{:?}", session.snapshots.steps),
        from_scratch(&session)
    );
}

#[test]
#[ignore = "needs Manim"]
fn an_edit_late_in_a_long_deck_does_not_replay_the_deck() {
    let _alone = alone();
    let sidecar = Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR")));
    let mut session = Session::blank().with_preview(sidecar, || {});
    session.record_ahead_of_time(false);
    // Thirty slides with a line of text and a formula each: long enough that what every
    // edit costs anyway, the wait for typing to pause above all, is the smaller part.
    for slide in 0..30 {
        if slide > 0 {
            session.add_slide();
        }
        session.add_object("Text");
        session.add_object("MathTex");
    }
    settle_all(&mut session);
    // The whole deck, by a Manim that is up and has typeset everything before: what
    // every edit used to cost.
    session.refresh_preview();
    let whole = Instant::now();
    settle_all(&mut session);
    let whole = whole.elapsed();

    session.add_object("Circle");
    let late = settle(&mut session);
    session.select(15, Some(1));
    session.add_object("Square");
    let middle = settle(&mut session);
    let rest = Instant::now();
    settle_all(&mut session);
    println!(
        "the whole deck {whole:?}; an edit on the last slide {late:?}; on a middle one \
         {middle:?} until the canvas has it, {:?} more for the slides after",
        rest.elapsed()
    );
    assert!(late < whole / 3, "{late:?} against {whole:?}");
    assert!(middle < whole / 3, "{middle:?} against {whole:?}");
    assert_eq!(
        format!("{:?}", session.snapshots.steps),
        from_scratch(&session)
    );
}
