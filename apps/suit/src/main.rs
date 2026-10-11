//! Suit: a visual editor for Manim slides.

use std::path::{Path, PathBuf};

use eframe::egui;
use suit_engine::{PresentOptions, Session};
use suit_render::Sidecar;
use suit_ui::SuitApp;

/// What the command line asked for.
#[derive(Default)]
struct Launch {
    deck: Option<PathBuf>,
    /// Start with an empty deck instead of the sample.
    blank: bool,
    /// Save a picture of the window to this file once the preview is ready, then quit.
    screenshot: Option<PathBuf>,
    /// Start on this slide and step, both counted from 1; step 0 is the start of the slide.
    select: Option<(usize, usize)>,
    /// Present the deck straight away.
    present: bool,
    options: PresentOptions,
    /// Show the notes in a second window while presenting.
    notes: bool,
}

const USAGE: &str = "\
usage: suit [DECK.suit.json | --new] [options]

  --select SLIDE:STEP       start with this slide and step selected (step 0: the slide's start)
  --screenshot FILE.png     save a picture of the window once the preview is ready, and quit
  --present                 present the deck straight away, from the selected slide
  --notes                   show the notes in a second window while presenting

How a presentation runs (as in manim-slides present):
  --start-paused            wait for a key before the first slide
  --next-terminates-loop    next lets a looping slide finish its round before moving on
  --exit-after-last-slide   going past the last slide ends the presentation
  --skip-all                run through every slide without waiting for keys";

fn launch() -> Result<Launch, String> {
    let mut launch = Launch::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--new" => launch.blank = true,
            "--screenshot" => {
                let path = args.next().ok_or("--screenshot needs a file name")?;
                launch.screenshot = Some(PathBuf::from(path));
            }
            "--select" => {
                let value = args.next().ok_or("--select needs SLIDE:STEP")?;
                let parsed = value
                    .split_once(':')
                    .and_then(|(slide, step)| Some((slide.parse().ok()?, step.parse().ok()?)));
                launch.select = Some(parsed.ok_or("--select needs SLIDE:STEP, for example 2:5")?);
            }
            "--present" => launch.present = true,
            "--notes" => launch.notes = true,
            "--start-paused" => launch.options.start_paused = true,
            "--next-terminates-loop" => launch.options.next_ends_loops = true,
            "--exit-after-last-slide" => launch.options.leave_after_last = true,
            "--skip-all" => launch.options.run_through = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            unknown if unknown.starts_with("--") => {
                return Err(format!("there is no option {unknown}\n\n{USAGE}"));
            }
            _ => launch.deck = Some(PathBuf::from(arg)),
        }
    }
    Ok(launch)
}

fn app(launch: Launch) -> Result<SuitApp, String> {
    let mut session = match (&launch.deck, launch.blank) {
        (Some(path), _) => Session::open(path)?,
        (None, true) => Session::blank(),
        (None, false) => Session::sample(),
    };
    if let Some((slide, step)) = launch.select {
        session.select(slide.saturating_sub(1), step.checked_sub(1));
    }
    let near = launch
        .deck
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or(Path::new("."));
    let app = SuitApp::new(session, Some(Sidecar::locate(near)))
        .with_screenshot(launch.screenshot)
        .with_presentation(launch.options, launch.notes);
    Ok(match launch.present {
        true => app.presenting_at_once(),
        false => app,
    })
}

fn main() -> eframe::Result {
    let app = match launch().and_then(app) {
        Ok(app) => app,
        Err(message) => {
            eprintln!("suit: {message}");
            std::process::exit(2);
        }
    };
    let viewport = egui::ViewportBuilder::default()
        .with_title("Suit")
        .with_app_id("suit")
        .with_inner_size([1480.0, 920.0])
        .with_min_inner_size([1000.0, 640.0]);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Suit",
        options,
        Box::new(move |creation| {
            SuitApp::install_fonts(&creation.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}
