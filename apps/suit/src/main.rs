//! Suit: a visual editor for Manim slides.

// On Windows a program is either one with windows or one with a console, and says which.
// Without this every start would open a black console behind the editor.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
};

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

/// The ways of drawing a window that are tried, in turn, when the one the system offers
/// first does not start. Each is tried in a fresh run of the program, because a program
/// gets to open its window system once.
const OTHER_WAYS: [&str; 2] = ["vulkan", "gl"];
/// How many of them the run that started this one has been through.
const TRIES: &str = "SUIT_DRAWING_TRIES";

/// Says what went wrong where someone will see it. On Windows a program with a window has
/// nowhere to print to, so the message is written down and opened.
fn say(message: &str) {
    eprintln!("suit: {message}");
    if cfg!(windows)
        && let Some(own) = suit_render::own_folder()
    {
        let note = own.join("suit-did-not-start.txt");
        if std::fs::create_dir_all(&own).is_ok() && std::fs::write(&note, message).is_ok() {
            let _ = Command::new("notepad").arg(note).spawn();
        }
    }
}

fn main() {
    let app = match launch().and_then(app) {
        Ok(app) => app,
        Err(message) => {
            say(&message);
            std::process::exit(2);
        }
    };
    let viewport = egui::ViewportBuilder::default()
        .with_title("Suit")
        .with_app_id("suit")
        .with_inner_size([1480.0, 920.0])
        .with_min_inner_size([1000.0, 640.0]);
    let viewport = match SuitApp::icon() {
        Some(icon) => viewport.with_icon(icon),
        None => viewport,
    };
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    static OPENED: AtomicBool = AtomicBool::new(false);
    let run = eframe::run_native(
        "Suit",
        options,
        Box::new(move |creation| {
            OPENED.store(true, Ordering::Relaxed);
            SuitApp::install_fonts(&creation.egui_ctx);
            Ok(Box::new(app))
        }),
    );
    let Err(problem) = run else { return };
    // The window never opened: the graphics the system offered first would not start.
    // Another way of drawing may, and is tried before giving up.
    let tried: usize = env::var(TRIES)
        .ok()
        .and_then(|count| count.parse().ok())
        .unwrap_or(0);
    if !OPENED.load(Ordering::Relaxed)
        && let Some(way) = OTHER_WAYS.get(tried)
        && let Ok(this) = env::current_exe()
    {
        let again = Command::new(this)
            .args(env::args_os().skip(1))
            .env("WGPU_BACKEND", way)
            .env(TRIES, (tried + 1).to_string())
            .status();
        if let Ok(ended) = again {
            std::process::exit(ended.code().unwrap_or(1));
        }
    }
    say(&format!(
        "Suit could not open its window: {problem}\n\nThis is mostly a graphics driver \
         that is missing or too old. Updating it is the thing to try."
    ));
    std::process::exit(1);
}
