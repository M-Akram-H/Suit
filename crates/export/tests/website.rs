//! Making a website of a deck, against real Manim, so ignored unless asked for:
//! `cargo test -p suit-export -- --ignored`

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, PoisonError},
    thread,
    time::Duration,
};

use suit_check::check;
use suit_core::Deck;
use suit_export::{
    pdf::Pages,
    site::{Job, Request},
};
use suit_render::Sidecar;
use suit_sig::Signature;

/// Manim runs one at a time per cache directory, so these tests must not overlap.
fn alone() -> MutexGuard<'static, ()> {
    static TURN: Mutex<()> = Mutex::new(());
    TURN.lock().unwrap_or_else(PoisonError::into_inner)
}

fn sample() -> Deck {
    let text = include_str!("../../../examples/sample.suit.json");
    serde_json::from_str(text).expect("the sample deck is a deck")
}

/// A folder of its own for each test, empty.
fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("suit-website-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&folder);
    folder
}

fn request(folder: &Path, height: u32) -> Request {
    let deck = sample();
    let sig = Signature::builtin();
    let checked = check(&deck, &sig);
    Request {
        deck,
        checked,
        sig,
        title: "Sample <talk>".to_owned(),
        folder: folder.to_path_buf(),
        height: Some(height),
        notes: true,
        pdf: Some(Pages::Slides),
        snapshots: None,
        sidecar: Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR"))),
    }
}

#[test]
#[ignore = "needs Manim"]
fn a_deck_becomes_a_folder_that_any_web_host_serves() {
    let _alone = alone();
    let folder = folder("whole");
    let report = Job::start(request(&folder, 144))
        .wait(|_| {})
        .expect("the website is made");
    assert_eq!(report.slides, 6);
    assert!(report.pdf);
    for name in [
        "index.html",
        "player.js",
        "player.css",
        "deck.js",
        "slides.pdf",
        "README.md",
        "vercel.json",
        "icon.png",
    ] {
        assert!(folder.join(name).is_file(), "{name} is there");
    }
    assert!(
        !folder.join(".suit-new").exists(),
        "nothing half-made is left"
    );
    let videos = fs::read_dir(folder.join("videos")).unwrap().count();
    assert_eq!(videos, 12, "every slide forwards and backwards");

    let deck = fs::read_to_string(folder.join("deck.js")).unwrap();
    assert!(deck.contains("window.SUIT_DECK"));
    assert!(
        deck.contains("\"title\": \"Graph\""),
        "the slides keep their titles"
    );
    assert!(
        deck.contains("Welcome everyone."),
        "and their notes, when asked"
    );
    let index = fs::read_to_string(folder.join("index.html")).unwrap();
    assert!(index.contains("<title>Sample &lt;talk&gt;</title>"));
    let pdf = fs::read(folder.join("slides.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));

    // Exporting again replaces Suit's files and leaves the author's alone.
    fs::write(folder.join("README.md"), "mine").unwrap();
    fs::write(folder.join("videos").join("stale.mp4"), "old").unwrap();
    let mut again = request(&folder, 144);
    again.notes = false;
    again.pdf = None;
    Job::start(again).wait(|_| {}).expect("made again");
    assert_eq!(
        fs::read_to_string(folder.join("README.md")).unwrap(),
        "mine"
    );
    assert!(!folder.join("videos").join("stale.mp4").exists());
    assert!(!folder.join("slides.pdf").exists(), "no PDF was asked for");
    let deck = fs::read_to_string(folder.join("deck.js")).unwrap();
    assert!(!deck.contains("Welcome everyone."), "nor the notes");
    let _ = fs::remove_dir_all(&folder);
}

#[test]
#[ignore = "needs Manim"]
fn a_website_being_made_can_be_stopped() {
    let _alone = alone();
    let folder = folder("stopped");
    let job = Job::start(request(&folder, 1080));
    // Into the rendering, past starting Manim.
    let mut waited = 0;
    while !job.progress().doing.starts_with("Rendering") && waited < 600 {
        thread::sleep(Duration::from_millis(50));
        waited += 1;
    }
    thread::sleep(Duration::from_millis(500));
    job.stop();
    let outcome = job.wait(|_| {});
    assert_eq!(outcome.unwrap_err(), "Stopped.");
    assert!(!folder.exists(), "nothing was written, not even the folder");
    let _ = fs::remove_dir_all(&folder);
}
