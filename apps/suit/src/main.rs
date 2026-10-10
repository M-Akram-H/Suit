//! Suit: a visual editor for Manim slides.

use std::path::{Path, PathBuf};

use eframe::egui;
use suit_engine::Session;
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
}

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
            "-h" | "--help" => {
                println!(
                    "usage: suit [DECK.suit.json | --new] [--select SLIDE:STEP] [--screenshot FILE.png]"
                );
                std::process::exit(0);
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
    Ok(SuitApp::new(session, Some(Sidecar::locate(near))).with_screenshot(launch.screenshot))
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
