//! `suit-cli`: check, time and export Manim slide decks from the command line.

mod sample;

use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use suit_check::{Checked, Diagnostic, Severity, check};
use suit_codegen::{Options, export, pretty::Pretty};
use suit_core::{Deck, NodeId, SlideContent};
use suit_engine::{Job, Pages, Session, WebsiteChoices};
use suit_render::{Prober, Sidecar, Watch};
use suit_sig::Signature;

#[derive(Parser)]
#[command(
    name = "suit-cli",
    version,
    about = "Check, time and export Manim slide decks: as Python, a PDF or a website"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Write the sample deck
    Sample {
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Report the problems in a deck
    Check { deck: PathBuf },
    /// Show how long each slide and each step takes
    Timeline { deck: PathBuf },
    /// Write the deck as a Python file for manim-slides
    Export {
        deck: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Render at this pixel height instead of the deck's (for quick previews)
        #[arg(long)]
        height: Option<u32>,
        #[arg(long, default_value = "Deck")]
        class_name: String,
    },
    /// Write the Python script that reports the scene after every step (used by the editor)
    Probe {
        deck: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Report every frame of this slide (counted from 1) instead of every step of the deck
        #[arg(long, value_name = "SLIDE")]
        frames: Option<usize>,
    },
    /// Write the deck as a PDF: a page for every slide, as it ends (needs Manim)
    Pdf {
        deck: PathBuf,
        /// Where to write it; next to the deck if not given
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// A page for every step that changes the picture, instead of one per slide
        #[arg(long)]
        every_step: bool,
    },
    /// Write the deck as a website to watch in a browser and put online (needs Manim)
    Website {
        deck: PathBuf,
        /// The folder to write; DECK-website next to the deck if not given
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Make the videos this many pixels high instead of the deck's height
        #[arg(long)]
        height: Option<u32>,
        /// Put the slides' notes on the website, shown with N
        #[arg(long)]
        notes: bool,
        /// Leave out the PDF to download
        #[arg(long)]
        no_pdf: bool,
        /// The PDF has a page for every step instead of one per slide
        #[arg(long)]
        every_step: bool,
    },
    /// Print the catalogue of objects, actions and animations as JSON
    Signature,
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("suit-cli: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Cmd) -> Result<ExitCode, Box<dyn Error>> {
    let sig = Signature::builtin();
    match command {
        Cmd::Sample { output } => emit(output, &serde_json::to_string_pretty(&sample::deck())?)?,
        Cmd::Signature => println!("{}", serde_json::to_string_pretty(&sig)?),
        Cmd::Check { deck } => {
            let deck = load(&deck)?;
            let checked = check(&deck, &sig);
            print_diagnostics(&deck, &checked);
            if checked.has_errors() {
                return Ok(ExitCode::FAILURE);
            }
        }
        Cmd::Timeline { deck } => {
            let deck = load(&deck)?;
            print_timeline(&deck, &sig, &check(&deck, &sig));
        }
        Cmd::Probe {
            deck,
            output,
            frames,
        } => {
            let deck = load(&deck)?;
            let checked = check(&deck, &sig);
            let script = match frames {
                Some(slide) if (1..=deck.slides.len()).contains(&slide) => {
                    suit_codegen::probe_frames(&deck, &sig, &checked, slide - 1)
                }
                Some(slide) => return Err(format!("there is no slide {slide}").into()),
                None => suit_codegen::probe(&deck, &sig, &checked),
            };
            emit(output, &script)?;
        }
        Cmd::Pdf {
            deck,
            output,
            every_step,
        } => {
            let mut session = manim_session(&deck)?;
            let which = pages(every_step);
            let target = match output {
                Some(path) => path,
                None => session.export_path(|stem| format!("{stem}.pdf"))?,
            };
            let sidecar = Sidecar::locate(&deck_folder(&deck));
            let deck_for_manim = session.deck_for_manim();
            eprintln!("Asking Manim how every slide looks…");
            let shown = Prober::new(sidecar)
                .probe(&deck_for_manim, &sig, &session.checked, Watch::patient())
                .map_err(|failure| failure.to_string())?;
            if let Some(error) = &shown.error {
                return Err(format!("Manim stopped: {error}").into());
            }
            session.install_snapshots(shown);
            session.write_pdf(&target, which)?;
            eprintln!("Wrote {}", target.display());
        }
        Cmd::Website {
            deck,
            output,
            height,
            notes,
            no_pdf,
            every_step,
        } => {
            let session = manim_session(&deck)?;
            let folder = match output {
                Some(path) => path,
                None => session.website_folder()?,
            };
            let choices = WebsiteChoices {
                height,
                notes,
                pdf: (!no_pdf).then(|| pages(every_step)),
            };
            let sidecar = Sidecar::locate(&deck_folder(&deck));
            let job = Job::start(session.website(folder, choices, sidecar)?);
            let report = job.wait(|progress| match progress.done {
                Some(done) => eprintln!("{} … {:.0}%", progress.doing, done * 100.0),
                None => eprintln!("{} …", progress.doing),
            })?;
            let megabytes = report.bytes as f64 / 1e6;
            eprintln!(
                "Wrote {} slides to {} ({megabytes:.1} MB). Open index.html in it to watch them; \
                 its README.md says how to put them online.",
                report.slides,
                report.folder.display()
            );
            if megabytes > 100.0 {
                eprintln!(
                    "That is more than the 100 MB Vercel's free plan takes in one upload; \
                     --height 720 makes the videos smaller."
                );
            }
        }
        Cmd::Export {
            deck,
            output,
            height,
            class_name,
        } => {
            let deck = load(&deck)?;
            let checked = check(&deck, &sig);
            print_diagnostics(&deck, &checked);
            if checked.has_errors() {
                eprintln!("suit-cli: not exported, because the deck has errors");
                return Ok(ExitCode::FAILURE);
            }
            let options = Options {
                class_name,
                pixel_height: height,
            };
            emit(output, &export(&deck, &sig, &checked, &options))?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// The deck at `path`, checked, and refused if it has errors: what Manim is to run.
fn manim_session(path: &Path) -> Result<Session, Box<dyn Error>> {
    let session = Session::open(path)?;
    print_diagnostics(&session.deck, &session.checked);
    if session.checked.has_errors() {
        return Err("not exported, because the deck has errors".into());
    }
    Ok(session)
}

/// The folder the deck is in, where a `.venv` with Manim may be found.
fn deck_folder(deck: &Path) -> PathBuf {
    let folder = deck.parent().unwrap_or(Path::new("."));
    match folder.as_os_str().is_empty() {
        true => PathBuf::from("."),
        false => folder.to_path_buf(),
    }
}

fn pages(every_step: bool) -> Pages {
    match every_step {
        true => Pages::Steps,
        false => Pages::Slides,
    }
}

fn load(path: &PathBuf) -> Result<Deck, Box<dyn Error>> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(
        serde_json::from_str(&text)
            .map_err(|e| format!("{} is not a deck: {e}", path.display()))?,
    )
}

fn emit(output: Option<PathBuf>, text: &str) -> Result<(), Box<dyn Error>> {
    match output {
        Some(path) => {
            fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?
        }
        None => print!("{text}"),
    }
    Ok(())
}

/// Where each node sits: slide index and, within the slide, the step number from 1.
fn steps(deck: &Deck) -> BTreeMap<NodeId, usize> {
    let mut steps = BTreeMap::new();
    for slide in &deck.slides {
        let SlideContent::Blueprint(blueprint) = &slide.content else {
            continue;
        };
        for (index, node) in blueprint.body.iter().enumerate() {
            steps.extend(node.ids().into_iter().map(|id| (id, index + 1)));
        }
    }
    steps
}

fn print_diagnostics(deck: &Deck, checked: &Checked) {
    let steps = steps(deck);
    for Diagnostic {
        severity,
        code,
        slide,
        node,
        message,
        help,
    } in &checked.diagnostics
    {
        let severity = match severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let title = deck.slides.get(*slide).map_or("", |s| s.title.as_str());
        let step = node
            .and_then(|id| steps.get(&id))
            .map(|n| format!(", step {n}"))
            .unwrap_or_default();
        eprintln!(
            "{severity}[{code}] slide {} “{title}”{step}: {message}",
            slide + 1
        );
        if let Some(help) = help {
            eprintln!("    {help}");
        }
    }
    let errors = checked
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warnings = checked.diagnostics.len() - errors;
    eprintln!("{errors} error(s), {warnings} warning(s)");
}

fn print_timeline(deck: &Deck, sig: &Signature, checked: &Checked) {
    let pretty = Pretty { deck, sig };
    let fps = f64::from(deck.config.fps);
    let secs = |frames: u32| f64::from(frames) / fps;
    let mut total = 0;
    for (index, (slide, report)) in deck.slides.iter().zip(&checked.slides).enumerate() {
        let frames = report.frames + report.gap_frames;
        total += frames;
        let approx = if report.exact { "" } else { " (at least)" };
        let looping = if slide.config.looping {
            "  ⟳ loops"
        } else {
            ""
        };
        println!(
            "Slide {} “{}”  {:.2} s  {frames} frames{approx}{looping}",
            index + 1,
            slide.title,
            secs(frames)
        );
        let SlideContent::Blueprint(blueprint) = &slide.content else {
            continue;
        };
        for (step, node) in blueprint.body.iter().enumerate() {
            let note = checked.notes.get(&node.id).cloned().unwrap_or_default();
            let timing = match note.frames {
                0 => String::new(),
                frames => format!(
                    "  {:.2} s, frames {}–{}",
                    secs(frames),
                    note.start_frame,
                    note.start_frame + frames - 1
                ),
            };
            println!("  {:>2}  {}{timing}", step + 1, pretty.headline(&node.item));
        }
        if report.gap_frames > 0 {
            println!("      hold  {:.2} s", secs(report.gap_frames));
        }
    }
    println!(
        "Total  {:.2} s  {total} frames at {} fps",
        secs(total),
        deck.config.fps
    );
}
