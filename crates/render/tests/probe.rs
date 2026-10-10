//! Runs real Manim, so these are ignored unless asked for:
//! `cargo test -p suit-render -- --ignored`

use std::{path::Path, time::Instant};

use suit_check::check;
use suit_core::{Anim, Deck, DeckConfig, Object};
use suit_render::{Prober, Sidecar};
use suit_sig::Signature;

fn deck(text: &str) -> Deck {
    let mut deck = Deck::new(DeckConfig::default());
    let mut slide = deck.add_slide("One");
    let circle = slide.object("circle", Object::new("Circle"));
    let label = slide.object("label", Object::new("Text").arg("text", text));
    slide.play(vec![Anim::prim("Create").arg("mobject", &circle)]);
    slide.play(vec![Anim::prim("Write").arg("vmobject", &label)]);
    deck
}

fn prober() -> Prober {
    Prober::new(Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR"))))
}

#[test]
#[ignore = "needs Manim"]
fn a_kept_process_answers_repeated_probes() {
    let sig = Signature::builtin();
    let mut prober = prober();
    let mut times = Vec::new();
    for text in ["one", "two", "three"] {
        let deck = deck(text);
        let checked = check(&deck, &sig);
        let started = Instant::now();
        let snapshots = prober.probe(&deck, &sig, &checked).expect("Manim runs");
        times.push(started.elapsed());
        assert_eq!(snapshots.error, None);
        // The slide's start, then one snapshot per step.
        assert_eq!(snapshots.steps.len(), 5);
        let last = snapshots.steps.last().unwrap();
        assert_eq!(last.boxes.values().filter(|bounds| bounds.on).count(), 2);
        // The circle, and one outline per letter.
        assert_eq!(last.drawn.len(), 1 + text.len());
    }
    println!("probe times: {times:?}");
    assert!(times[2] < times[0], "later probes reuse the imported Manim");
}

#[test]
#[ignore = "needs Manim"]
fn a_failing_step_is_reported_with_what_came_before() {
    let sig = Signature::builtin();
    let mut deck = deck("fine");
    // An image that does not exist makes Manim raise when the step runs.
    deck.add_slide("Two").object(
        "photo",
        Object::new("ImageMobject").arg("filename_or_array", "no-such-file.png"),
    );
    let checked = check(&deck, &sig);
    let snapshots = prober()
        .probe(&deck, &sig, &checked)
        .expect("the probe itself runs");
    assert!(snapshots.error.is_some());
    assert_eq!(
        snapshots.steps.len(),
        6,
        "the first slide, and the start of the second"
    );
}
