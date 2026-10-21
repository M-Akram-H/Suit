//! A deck as a website: every slide rendered by Manim through manim-slides, and a player
//! for the browser that runs them the way Suit's presenter does (spec §7.1).
//!
//! Rendering takes minutes, so it is a [`Job`] on a thread of its own, which reports how
//! far it has got and can be stopped. The result is a folder of static files that any web
//! host serves as it is.

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use suit_check::Checked;
use suit_codegen::Options;
use suit_core::Deck;
use suit_render::{Failure, Prober, Sidecar, Snapshots, Watch};
use suit_sig::Signature;

use crate::pdf::{self, Pages};

const RENDER: &str = include_str!("render.py");
const INDEX: &str = include_str!("../assets/site/index.html");
const PLAYER: &str = include_str!("../assets/site/player.js");
const STYLE: &str = include_str!("../assets/site/player.css");
const README: &str = include_str!("../assets/site/README.md");
const VERCEL: &str = include_str!("../assets/site/vercel.json");
const ICON: &[u8] = include_bytes!("../../ui/assets/icon.png");

/// Marks the lines of the render script that say how far it has got.
const REPLY: char = '\u{1e}';

/// How long Manim gets to stop between two animations, once asked, before it is stopped by
/// force. One animation of a full-HD deck can take a while.
const GRACE: Duration = Duration::from_secs(20);

/// The folder, inside the website's, where the new website is put together before it
/// takes the place of the old one.
const STAGING: &str = ".suit-new";

/// The files of the folder that every export writes anew. Anything else in it is left alone.
const OURS: [&str; 5] = [
    "index.html",
    "player.js",
    "player.css",
    "deck.js",
    "slides.pdf",
];

/// What a website is to be made of.
pub struct Request {
    /// The deck as Manim is to see it: its pictures named in full.
    pub deck: Deck,
    pub checked: Checked,
    pub sig: Signature,
    /// What the page is called: the deck's name.
    pub title: String,
    /// The folder to write. It is made if need be.
    pub folder: PathBuf,
    /// The height of the videos, if not the deck's own.
    pub height: Option<u32>,
    /// Whether the slides' notes go with them, to be read with N.
    pub notes: bool,
    /// A PDF of the slides to download from the page, if wanted, and which pages it has.
    pub pdf: Option<Pages>,
    /// What Manim showed after every step, if the caller has it for the deck as it is;
    /// otherwise it is asked for when the PDF needs it.
    pub snapshots: Option<Snapshots>,
    pub sidecar: Sidecar,
}

/// How far a job has got.
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    /// What it is doing, in a few words.
    pub doing: String,
    /// How much of that is done, from 0 to 1, if that is known.
    pub done: Option<f32>,
}

/// What a finished job made.
#[derive(Clone, Debug)]
pub struct Report {
    pub folder: PathBuf,
    /// The size of the folder in bytes, without what version control and hosts keep in it.
    pub bytes: u64,
    pub slides: usize,
    pub pdf: bool,
}

/// A website being made on a thread of its own.
pub struct Job {
    progress: Arc<Mutex<Progress>>,
    stop: Arc<AtomicBool>,
    result: Receiver<Result<Report, String>>,
    folder: PathBuf,
    /// Held while the new website takes the place of the old one.
    placing: Arc<Mutex<()>>,
}

impl Job {
    pub fn start(mut request: Request) -> Job {
        // Manim runs in a folder of its own, where a relative name would mean another place.
        if let Ok(whole) = std::path::absolute(&request.folder) {
            request.folder = whole;
        }
        let progress = Arc::new(Mutex::new(Progress {
            doing: "Starting Manim".to_owned(),
            done: None,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let (send, result) = mpsc::channel();
        let folder = request.folder.clone();
        let placing = Arc::new(Mutex::new(()));
        let (shared, stopped, place) = (progress.clone(), stop.clone(), placing.clone());
        thread::spawn(move || {
            let outcome = make(request, &shared, &stopped, &place);
            let _ = send.send(outcome);
        });
        Job {
            progress,
            stop,
            result,
            folder,
            placing,
        }
    }

    /// The folder being written.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    pub fn progress(&self) -> Progress {
        self.progress
            .lock()
            .map(|progress| progress.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }

    /// Asks the job to stop. It does so between two animations, and then reports
    /// "Stopped." The folder is left as it was before the job.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    pub fn is_stopping(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    /// The outcome, once there is one.
    pub fn poll(&self) -> Option<Result<Report, String>> {
        match self.result.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("The export ended without a word.".to_owned()))
            }
        }
    }

    /// Waits for the outcome, telling `told` whenever the progress changes.
    pub fn wait(self, mut told: impl FnMut(&Progress)) -> Result<Report, String> {
        let mut last = None;
        loop {
            match self.result.recv_timeout(Duration::from_millis(200)) {
                Ok(outcome) => return outcome,
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("The export ended without a word.".to_owned());
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
            let now = self.progress();
            if last.as_ref() != Some(&now) {
                told(&now);
                last = Some(now);
            }
        }
    }
}

impl Drop for Job {
    /// A job nobody is waiting for any more stops, and what it made so far goes: Suit may
    /// be closing, and its thread with it. Should it be putting the new website in place
    /// at this very moment, that is let finish first.
    fn drop(&mut self) {
        self.stop();
        let _placing = self.placing.lock();
        let _ = fs::remove_dir_all(self.folder.join(STAGING));
    }
}

/// What the render script says about the slides it made.
#[derive(Deserialize)]
struct Manifest {
    slides: Vec<Rendered>,
}

#[derive(Deserialize)]
struct Rendered {
    kind: String,
    video: String,
    back: String,
    #[serde(rename = "loop")]
    looping: bool,
    auto_next: bool,
    playback_rate: f64,
    reversed_playback_rate: f64,
    notes: String,
}

/// What the player is told about the deck, in `deck.js`.
#[derive(Serialize)]
struct Shown<'a> {
    title: &'a str,
    width: u32,
    height: u32,
    background: String,
    notes: bool,
    pdf: Option<&'a str>,
    slides: Vec<ShownSlide<'a>>,
}

#[derive(Serialize)]
struct ShownSlide<'a> {
    title: &'a str,
    kind: &'a str,
    video: &'a str,
    back: &'a str,
    #[serde(rename = "loop")]
    looping: bool,
    auto_next: bool,
    playback_rate: f64,
    reversed_playback_rate: f64,
    notes: &'a str,
}

fn say(progress: &Mutex<Progress>, doing: &str, done: Option<f32>) {
    if let Ok(mut progress) = progress.lock() {
        *progress = Progress {
            doing: doing.to_owned(),
            done,
        };
    }
}

fn written(path: &Path, what: std::io::Result<()>) -> Result<(), String> {
    what.map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Whether `folder` may be written into: it is new, empty, has nothing but hidden things
/// (a repository, a host's settings), or was written by Suit before.
fn ours(folder: &Path) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Ok(());
    };
    let mut others = false;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "deck.js" {
            let text = fs::read_to_string(entry.path()).unwrap_or_default();
            if text.contains("SUIT_DECK") {
                return Ok(());
            }
        }
        others |= !name.starts_with('.');
    }
    match others {
        true => Err(format!(
            "{} has other files in it. Choose a new folder, or one Suit wrote a website into.",
            folder.display()
        )),
        false => Ok(()),
    }
}

fn make(
    request: Request,
    progress: &Mutex<Progress>,
    stop: &AtomicBool,
    placing: &Mutex<()>,
) -> Result<Report, String> {
    let Request {
        deck,
        checked,
        sig,
        title,
        folder,
        height,
        notes,
        pdf: pdf_pages,
        snapshots,
        sidecar,
    } = request;
    if checked.has_errors() {
        return Err("The deck has problems; fix the red ones before exporting.".to_owned());
    }
    ours(&folder)?;
    let stopped = || stop.load(Ordering::Relaxed);

    // What the PDF is drawn from, if one is wanted and the caller had nothing current.
    let snapshots = match (pdf_pages, snapshots) {
        (Some(_), None) => {
            say(progress, "Asking Manim how every slide looks", None);
            let wanted = || !stopped();
            let watch = Watch {
                limit: Duration::from_secs(600),
                wanted: &wanted,
            };
            let mut prober = Prober::new(sidecar.clone());
            let shown = prober
                .probe(&deck, &sig, &checked, watch)
                .map_err(|failure| failure.to_string())?;
            if let Some(error) = &shown.error {
                return Err(format!("Manim stopped: {error}"));
            }
            Some(shown)
        }
        (_, given) => given,
    };

    // Manim keeps what it has rendered under a name of its own for every website folder,
    // so that exporting again after a small change renders only what changed.
    let mut hasher = DefaultHasher::new();
    folder.hash(&mut hasher);
    let work = sidecar
        .work()
        .join(format!("render-{:016x}", hasher.finish()));
    fs::create_dir_all(&work).map_err(|e| format!("cannot create {}: {e}", work.display()))?;
    let (script, python, stop_file) = (
        work.join("render.py"),
        work.join("deck.py"),
        work.join("stop"),
    );
    let _ = fs::remove_file(&stop_file);
    // Removed and made again, so that the folder counts as recently used.
    let _ = fs::remove_file(&python);
    let options = Options {
        class_name: "Deck".to_owned(),
        pixel_height: height,
    };
    let code = suit_codegen::export(&deck, &sig, &checked, &options);
    written(&python, fs::write(&python, code))?;
    written(&script, fs::write(&script, RENDER))?;

    let new_folder = !folder.exists();
    fs::create_dir_all(&folder).map_err(|e| format!("cannot create {}: {e}", folder.display()))?;
    let staging = folder.join(STAGING);
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging)
        .map_err(|e| format!("cannot create {}: {e}", staging.display()))?;
    // Whatever happens from here on, the half-made files go.
    let outcome = render_and_place(RenderPlan {
        deck: &deck,
        checked: &checked,
        title: &title,
        folder: &folder,
        staging: &staging,
        work: &work,
        script: &script,
        python: &python,
        stop_file: &stop_file,
        notes,
        pdf_pages,
        snapshots: snapshots.as_ref(),
        sidecar: &sidecar,
        progress,
        stop,
        placing,
    });
    let _ = fs::remove_dir_all(&staging);
    if outcome.is_err() && new_folder {
        // Only if nothing else has been put in it meanwhile.
        let _ = fs::remove_dir(&folder);
    }
    outcome
}

struct RenderPlan<'a> {
    deck: &'a Deck,
    checked: &'a Checked,
    title: &'a str,
    folder: &'a Path,
    staging: &'a Path,
    work: &'a Path,
    script: &'a Path,
    python: &'a Path,
    stop_file: &'a Path,
    notes: bool,
    pdf_pages: Option<Pages>,
    snapshots: Option<&'a Snapshots>,
    sidecar: &'a Sidecar,
    progress: &'a Mutex<Progress>,
    stop: &'a AtomicBool,
    placing: &'a Mutex<()>,
}

fn render_and_place(plan: RenderPlan) -> Result<Report, String> {
    let RenderPlan {
        deck,
        title,
        folder,
        staging,
        ..
    } = plan;
    render(&plan)?;

    say(plan.progress, "Writing the website", None);
    let manifest = staging.join("manifest.json");
    let text = fs::read_to_string(&manifest)
        .map_err(|e| format!("Manim said nothing about the slides it made: {e}"))?;
    let manifest: Manifest = serde_json::from_str(&text)
        .map_err(|e| format!("Manim's list of slides is unreadable: {e}"))?;
    let _ = fs::remove_file(staging.join("manifest.json"));

    let pdf_name = match (plan.pdf_pages, plan.snapshots) {
        (Some(which), Some(snapshots)) => {
            let pages = pdf::pages(deck, snapshots, which)?;
            let path = staging.join("slides.pdf");
            written(&path, fs::write(&path, pdf::write(deck, &pages, title)))?;
            Some("slides.pdf")
        }
        _ => None,
    };

    // The slides manim-slides made are the deck's, less the skipped ones; their titles
    // come from the deck.
    let shown: Vec<&suit_core::Slide> = deck
        .slides
        .iter()
        .filter(|slide| !slide.config.skip)
        .collect();
    let titled = shown.len() == manifest.slides.len();
    if !titled {
        eprintln!(
            "suit: the deck shows {} slides and Manim made {}; the website has no titles",
            shown.len(),
            manifest.slides.len()
        );
    }
    let slides = manifest
        .slides
        .iter()
        .enumerate()
        .map(|(index, made)| ShownSlide {
            title: match titled {
                true => shown[index].title.trim(),
                false => "",
            },
            kind: &made.kind,
            video: &made.video,
            back: &made.back,
            looping: made.looping,
            auto_next: made.auto_next,
            playback_rate: made.playback_rate,
            reversed_playback_rate: made.reversed_playback_rate,
            notes: match plan.notes {
                true => made.notes.as_str(),
                false => "",
            },
        })
        .collect();
    let config = &deck.config;
    let background = config.background.hex_rgb();
    let player_deck = Shown {
        title,
        width: config.width,
        height: config.height,
        background: background.clone(),
        notes: plan.notes,
        pdf: pdf_name,
        slides,
    };
    let json = serde_json::to_string_pretty(&player_deck).map_err(|e| e.to_string())?;
    let deck_js = format!(
        "// The slides of this website. Written by Suit; exporting again replaces it.\n\
         window.SUIT_DECK = {json};\n"
    );
    let ratio = f64::from(config.width) / f64::from(config.height.max(1));
    let index = INDEX
        .replace("{{title}}", &escape(title))
        .replace("{{background}}", &background)
        .replace("{{ratio}}", &format!("{ratio:.6}"));
    for (name, text) in [
        ("index.html", index.as_str()),
        ("player.js", PLAYER),
        ("player.css", STYLE),
        ("deck.js", deck_js.as_str()),
    ] {
        let path = staging.join(name);
        written(&path, fs::write(&path, text))?;
    }

    // The new website takes the place of the old one: Suit's own files and the videos.
    // What the author or a host keeps in the folder stays.
    let _placing = plan.placing.lock();
    if plan.stop.load(Ordering::Relaxed) {
        return Err("Stopped.".to_owned());
    }
    for name in OURS {
        let (new, old) = (staging.join(name), folder.join(name));
        if new.exists() {
            let _ = fs::remove_file(&old);
            fs::rename(&new, &old).map_err(|e| format!("cannot write {}: {e}", old.display()))?;
        } else if old.exists() {
            let _ = fs::remove_file(&old);
        }
    }
    let videos = folder.join("videos");
    let _ = fs::remove_dir_all(&videos);
    fs::rename(staging.join("videos"), &videos)
        .map_err(|e| format!("cannot write {}: {e}", videos.display()))?;
    let readme = README.replace("{{title}}", title);
    for (name, bytes) in [
        ("README.md", readme.as_bytes()),
        ("vercel.json", VERCEL.as_bytes()),
        ("icon.png", ICON),
    ] {
        let path = folder.join(name);
        if !path.exists() {
            written(&path, fs::write(&path, bytes))?;
        }
    }
    Ok(Report {
        folder: folder.to_path_buf(),
        bytes: size(folder),
        slides: manifest.slides.len(),
        pdf: pdf_name.is_some(),
    })
}

/// Runs the render script until it is done, is stopped, or fails.
fn render(plan: &RenderPlan) -> Result<(), String> {
    let log = plan.work.join("render.log");
    let errors =
        fs::File::create(&log).map_err(|e| format!("cannot write {}: {e}", log.display()))?;
    let mut python = Command::new(plan.sidecar.python());
    python
        .args(["-u", "-X", "utf8"])
        .arg(plan.script)
        .arg(plan.python)
        .arg(plan.staging)
        .arg(plan.stop_file)
        .current_dir(plan.work)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(errors);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        python.creation_flags(NO_WINDOW);
    }
    let mut child = python.spawn().map_err(|e| {
        let failure = Failure::Broken(format!(
            "There is no Python here to run Manim with ({}): {e}",
            plan.sidecar.python().display()
        ));
        plan.sidecar.explain(failure).to_string()
    })?;
    let output = BufReader::new(child.stdout.take().expect("stdout was piped"));
    let (lines, said) = mpsc::channel();
    thread::spawn(move || {
        for line in output.lines().map_while(Result::ok) {
            if let Some(line) = line.strip_prefix(REPLY)
                && lines.send(line.to_owned()).is_err()
            {
                return;
            }
        }
    });

    let fps = f64::from(plan.deck.config.fps.max(1));
    let total: f64 = plan
        .checked
        .slides
        .iter()
        .map(|slide| f64::from(slide.frames + slide.gap_frames) / fps)
        .sum();
    let slides = plan.deck.slides.iter().filter(|s| !s.config.skip).count();
    let part = |n: &str| n.parse::<f32>().ok().map(|n| n / slides.max(1) as f32);
    let mut asked_to_stop: Option<Instant> = None;
    let mut done = false;
    say(plan.progress, "Rendering the slides", Some(0.0));
    loop {
        match said.recv_timeout(Duration::from_millis(50)) {
            Ok(line) => {
                let (what, rest) = line.split_once(' ').unwrap_or((line.as_str(), ""));
                match what {
                    "at" => {
                        let at = rest.parse::<f64>().unwrap_or(0.0);
                        let fraction = (at / total.max(1e-9)).clamp(0.0, 1.0) as f32;
                        say(plan.progress, "Rendering the slides", Some(fraction));
                    }
                    "reverse" => say(plan.progress, "Making the rewind videos", part(rest)),
                    "stream" => say(
                        plan.progress,
                        "Getting the videos ready for the web",
                        part(rest),
                    ),
                    "done" => done = true,
                    _ => {}
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            // The script has closed its output: it is about to end.
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if plan.stop.load(Ordering::Relaxed) {
            match asked_to_stop {
                None => {
                    let _ = fs::write(plan.stop_file, "");
                    asked_to_stop = Some(Instant::now());
                    say(plan.progress, "Stopping", None);
                }
                Some(since) if since.elapsed() > GRACE => {
                    give_up(&mut child, plan.work);
                    return Err("Stopped.".to_owned());
                }
                Some(_) => {}
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let _ = fs::remove_file(plan.stop_file);
    if asked_to_stop.is_some() || status.code() == Some(3) {
        return Err("Stopped.".to_owned());
    }
    if status.success() && done {
        return Ok(());
    }
    let said = fs::read_to_string(&log).unwrap_or_default();
    let failure = Failure::Broken(format!("Manim stopped: {}", last_words(&said)));
    Err(plan.sidecar.explain(failure).to_string())
}

/// Stops Manim by force. What it was writing may be half a video, which Manim would take
/// for a finished one next time, so all it kept goes too.
fn give_up(child: &mut Child, work: &Path) {
    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(work.join("media").join("videos"));
    let _ = fs::remove_dir_all(work.join("slides"));
}

/// The line of Python's error output that says what went wrong.
fn last_words(log: &str) -> String {
    let lines: Vec<&str> = log
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    // A traceback ends with the error itself; Manim's own messages are boxed in by rich.
    let error = lines.iter().rev().find(|line| {
        line.contains("Error") || line.contains("Exception") || line.starts_with("No module")
    });
    error
        .or(lines.last())
        .map_or("no output".to_owned(), |line| {
            line.trim_matches(|c: char| "│┃|".contains(c))
                .trim()
                .to_owned()
        })
}

/// The bytes in `folder` that a host would be sent.
fn size(folder: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(folder) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => size(&entry.path()),
            _ => entry.metadata().map_or(0, |meta| meta.len()),
        })
        .sum()
}

/// Text made safe to put in HTML.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_with_other_things_in_it_is_not_written_into() {
        let base = std::env::temp_dir().join(format!("suit-site-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join(".git")).unwrap();
        assert!(ours(&base).is_ok(), "a repository and nothing else");
        assert!(
            ours(&base.join("new")).is_ok(),
            "a folder that is not there yet"
        );
        fs::write(base.join("notes.txt"), "mine").unwrap();
        assert!(ours(&base).is_err());
        fs::write(base.join("deck.js"), "window.SUIT_DECK = {};").unwrap();
        assert!(ours(&base).is_ok(), "one Suit wrote before");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn what_python_said_last_is_found_in_its_output() {
        let log = "Traceback (most recent call last):\n  File \"x\", line 1\n\
                   ValueError: no such colour\n\n";
        assert_eq!(last_words(log), "ValueError: no such colour");
        assert_eq!(last_words(""), "no output");
        assert_eq!(escape("A < B & \"C\""), "A &lt; B &amp; &quot;C&quot;");
    }
}
