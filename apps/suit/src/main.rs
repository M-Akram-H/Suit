//! Suit: a visual editor for Manim slides.

mod app;
mod canvas;
mod session;

use std::path::PathBuf;

use eframe::egui;

/// What the command line asked for.
#[derive(Default)]
pub struct Launch {
    pub deck: Option<PathBuf>,
    /// Save a picture of the window to this file once the preview is ready, then quit.
    pub screenshot: Option<PathBuf>,
    /// Start on this slide and step, both counted from 1; step 0 is the start of the slide.
    pub select: Option<(usize, usize)>,
}

fn launch() -> Result<Launch, String> {
    let mut launch = Launch::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
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
                    "usage: suit [DECK.suit.json] [--select SLIDE:STEP] [--screenshot FILE.png]"
                );
                std::process::exit(0);
            }
            _ => launch.deck = Some(PathBuf::from(arg)),
        }
    }
    Ok(launch)
}

fn main() -> eframe::Result {
    let launch = match launch() {
        Ok(launch) => launch,
        Err(message) => {
            eprintln!("suit: {message}");
            std::process::exit(2);
        }
    };
    let viewport = egui::ViewportBuilder::default()
        .with_title("Suit")
        .with_app_id("suit")
        .with_inner_size([1440.0, 880.0])
        .with_min_inner_size([960.0, 600.0]);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Suit",
        options,
        Box::new(move |creation| Ok(Box::new(app::SuitApp::new(creation, launch)?))),
    )
}
