//! `suit`: check, time and export Manim slide decks from the command line.

mod sample;

use std::{collections::BTreeMap, error::Error, fs, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};
use suit_check::{Checked, Diagnostic, Severity, check};
use suit_codegen::{Options, export};
use suit_core::{Anim, Command, Deck, Node, NodeId, Object, SlideContent, Updater};
use suit_sig::Signature;

#[derive(Parser)]
#[command(
    name = "suit",
    version,
    about = "Check, time and export Manim slide decks"
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
    /// Print the catalogue of objects, actions and animations as JSON
    Signature,
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("suit: {error}");
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
            print_timeline(&deck, &check(&deck, &sig));
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
                eprintln!("suit: not exported, because the deck has errors");
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

fn print_timeline(deck: &Deck, checked: &Checked) {
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
            println!("  {:>2}  {}{timing}", step + 1, describe(deck, node));
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

fn describe(deck: &Deck, node: &Node<Command>) -> String {
    let name = |var| deck.names.get(&var).map_or("?", String::as_str);
    let place = |p: &suit_core::Place| {
        let path: String = p.path.iter().map(|i| format!("[{i}]")).collect();
        format!("{}{path}", name(p.root))
    };
    let places = |ps: &[suit_core::Place]| ps.iter().map(place).collect::<Vec<_>>().join(", ");
    match &node.item {
        Command::LetObject { var, value } => {
            format!("new   {} = {}", name(*var), object_head(&value.item))
        }
        Command::LetValue { var, .. } => format!("value {}", name(*var)),
        Command::Apply { target, call } => format!("set   {}.{}", place(target), call.method),
        Command::Add { targets } => format!("add   {}", places(targets)),
        Command::Remove { targets } => format!("remove {}", places(targets)),
        Command::Play { anims } => {
            let heads: Vec<String> = anims.iter().map(|a| anim_head(&a.item, &place)).collect();
            format!("play  {}", heads.join(" + "))
        }
        Command::Wait { .. } => "wait".to_owned(),
        Command::Attach { target, updater } => {
            let call = match updater {
                Updater::Always(call) | Updater::Every { call, .. } => call,
            };
            format!("always {}.{}", place(target), call.method)
        }
        Command::Detach { target } => format!("detach {}", place(target)),
        Command::Raw(_) => "python".to_owned(),
    }
}

fn object_head(object: &Object) -> String {
    match object {
        Object::New { ctor, .. } => ctor.to_string(),
        Object::Group {
            vector: true,
            items,
        } => format!("vgroup of {}", items.len()),
        Object::Group {
            vector: false,
            items,
        } => format!("group of {}", items.len()),
        Object::Copy(_) => "copy".to_owned(),
        Object::Chain { base, .. } => object_head(&base.item),
    }
}

fn anim_head(anim: &Anim, place: &impl Fn(&suit_core::Place) -> String) -> String {
    match anim {
        Anim::Prim { ctor, args, .. } => {
            let first = args.iter().find_map(|(_, arg)| match arg {
                suit_core::Arg::Expr(suit_core::Expr::Place(p)) => Some(place(p)),
                _ => None,
            });
            format!("{ctor}({})", first.unwrap_or_default())
        }
        Anim::Animate { target, calls, .. } => {
            let methods: Vec<&str> = calls.iter().map(|call| call.method.as_str()).collect();
            format!("{}.{}", place(target), methods.join("."))
        }
        Anim::Par { items, .. } => format!("{} together", items.len()),
        Anim::Seq { items, .. } => format!("{} in sequence", items.len()),
    }
}
