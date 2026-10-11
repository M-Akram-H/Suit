//! Manim as a sidecar process: the reference interpretation of the catalogue (spec §2).
//!
//! The editor never guesses what Manim would draw. It asks: a generated probe script runs
//! the deck with animations skipped and reports the outlines on scene after every step.

use std::{
    collections::BTreeMap,
    env, fmt, fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, de::DeserializeOwned};
use suit_check::Checked;
use suit_core::{Deck, NodeId, VarId};
use suit_sig::Signature;

/// The scene after every step of a deck, in deck order.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Snapshots {
    /// The first slide reported: the ones before it were not asked for.
    #[serde(default)]
    pub first: usize,
    pub steps: Vec<Snapshot>,
    /// Why the run stopped early, if it did; the steps before it are still valid.
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Snapshot {
    pub slide: usize,
    /// The command this is the state after; `None` for the state a slide starts in.
    pub node: Option<NodeId>,
    /// What is drawn, back to front.
    pub drawn: Vec<Drawn>,
    /// Where each named object is, whether or not it is on scene.
    pub boxes: BTreeMap<VarId, Bounds>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Bounds {
    /// `[left, bottom, right, top]` in scene units.
    pub rect: [f32; 4],
    pub on: bool,
}

/// One filled and stroked outline: a Manim vector object without its children.
#[derive(Clone, Debug, Deserialize)]
pub struct Drawn {
    /// The named objects this belongs to, each with the path from that object down to it.
    /// Not reported for the frames of a [`Clip`].
    #[serde(default)]
    pub owners: Vec<(VarId, Vec<u32>)>,
    pub paths: Vec<Outline>,
    /// RGBA, each in `0..=1`.
    pub fill: [f32; 4],
    pub stroke: [f32; 4],
    /// In Manim's stroke units: hundredths of a scene unit at the default frame size.
    pub stroke_width: f32,
    /// Set for a picture, which is drawn from its pixels and has no outline.
    #[serde(default)]
    pub picture: Option<Picture>,
}

/// A picture on the scene.
#[derive(Clone, Debug, Deserialize)]
pub struct Picture {
    /// The file Manim wrote its pixels to. It says which picture this is; the pixels
    /// themselves are in `png`, because the file goes when that Manim does.
    pub file: PathBuf,
    /// Where its top left, top right and bottom left corners are, in scene units.
    pub corners: [[f32; 2]; 3],
    /// From 0, not there, to 1.
    pub opacity: f32,
    /// The pixels, as a PNG file's contents. Empty if the file could not be read.
    #[serde(skip)]
    pub png: Arc<Vec<u8>>,
}

/// Reads the pixels of every picture in `drawn`, each file once.
fn load_pictures<'d>(drawn: impl Iterator<Item = &'d mut Drawn>) {
    let mut read: BTreeMap<PathBuf, Arc<Vec<u8>>> = BTreeMap::new();
    for picture in drawn.filter_map(|drawn| drawn.picture.as_mut()) {
        let png = read
            .entry(picture.file.clone())
            .or_insert_with(|| Arc::new(fs::read(&picture.file).unwrap_or_default()));
        picture.png = png.clone();
    }
}

/// What a probe reports, once whatever it points to on disk has been read.
trait Loaded {
    fn load(&mut self);
}

impl Loaded for Snapshots {
    fn load(&mut self) {
        load_pictures(self.steps.iter_mut().flat_map(|step| &mut step.drawn));
    }
}

impl Loaded for Clip {
    fn load(&mut self) {
        load_pictures(self.pool.iter_mut());
    }
}

/// A chain of cubic Bézier curves: the start point, then three points per curve.
#[derive(Clone, Debug, Deserialize)]
pub struct Outline {
    pub points: Vec<[f32; 2]>,
    pub closed: bool,
}

/// One slide as Manim plays it: what is drawn in every frame.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Clip {
    /// Every distinct outline that occurs. Most are drawn in many frames.
    #[serde(default)]
    pub pool: Vec<Drawn>,
    /// Per frame, the outlines drawn, back to front, as indices into `pool`.
    #[serde(default)]
    pub frames: Vec<Vec<u32>>,
    /// For each step of the slide, in order: the frame after its last one.
    #[serde(default)]
    pub marks: Vec<(NodeId, u32)>,
    /// Why the run stopped early, if it did.
    pub error: Option<String>,
}

impl Clip {
    /// What is drawn in frame `index`, back to front.
    pub fn frame(&self, index: usize) -> impl Iterator<Item = &Drawn> {
        let drawn = self.frames.get(index).map_or(&[][..], Vec::as_slice);
        drawn
            .iter()
            .filter_map(|&outline| self.pool.get(outline as usize))
    }
}

impl Snapshots {
    /// The state after `node`, or the state `slide` starts in when `node` is `None`.
    pub fn find(&self, slide: usize, node: Option<NodeId>) -> Option<&Snapshot> {
        self.steps
            .iter()
            .find(|step| step.slide == slide && step.node == node)
    }

    /// The last state recorded for `slide`: what it rests on.
    pub fn last_of(&self, slide: usize) -> Option<&Snapshot> {
        self.steps.iter().rev().find(|step| step.slide == slide)
    }
}

/// Where Python is, and a directory to run it in.
#[derive(Clone, Debug)]
pub struct Sidecar {
    python: PathBuf,
    work: PathBuf,
}

/// Why a run produced nothing.
#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// Its result was no longer wanted, so Manim was told to stop.
    Abandoned,
    /// Manim was still busy after this long, and was stopped.
    TooSlow(Duration),
    /// Anything else, in words.
    Broken(String),
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::Abandoned => f.write_str("Stopped."),
            Failure::TooSlow(limit) => write!(
                f,
                "Manim was still busy after {} seconds, so it was stopped.",
                limit.as_secs()
            ),
            Failure::Broken(reason) => f.write_str(reason),
        }
    }
}

impl From<String> for Failure {
    fn from(reason: String) -> Failure {
        Failure::Broken(reason)
    }
}

/// How long a run may take, and whether anyone still wants its result. Manim is stopped as
/// soon as either says so.
#[derive(Clone, Copy)]
pub struct Watch<'a> {
    pub limit: Duration,
    pub wanted: &'a dyn Fn() -> bool,
}

impl Watch<'static> {
    /// Waits ten minutes, whatever happens.
    pub fn patient() -> Watch<'static> {
        Watch {
            limit: Duration::from_secs(600),
            wanted: &|| true,
        }
    }
}

static RUNS: AtomicU64 = AtomicU64::new(0);

/// A name no other run or server has, in this process or another.
fn unique() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    )
}

/// What `server.py` should say.
const SERVER: &str = include_str!("server.py");

/// Files of ours this old were left behind by a Suit that was closed while Manim ran.
const STALE: Duration = Duration::from_secs(24 * 60 * 60);

/// How often a run in progress is asked whether it is still wanted.
const GLANCE: Duration = Duration::from_millis(20);

/// How long Manim gets to stop by itself once told to, before it is stopped by force.
const GRACE: Duration = Duration::from_secs(3);

/// The files of one probe run; removed again when this is dropped.
struct Run {
    script: PathBuf,
    result: PathBuf,
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.script);
        let _ = fs::remove_file(&self.result);
    }
}

impl Run {
    /// What the script wrote, if it got far enough to write anything.
    fn read<T: DeserializeOwned>(&self) -> Option<Result<T, Failure>> {
        let json = fs::read_to_string(&self.result).ok()?;
        let parsed = serde_json::from_str(&json);
        Some(parsed.map_err(|e| Failure::Broken(format!("unreadable probe result: {e}"))))
    }
}

impl Sidecar {
    /// Looks for a Python that has Manim: `$SUIT_PYTHON`, then a `.venv` in `near` or any
    /// directory above it (or above the working directory), then `python3`.
    pub fn locate(near: &Path) -> Sidecar {
        let from_env = env::var_os("SUIT_PYTHON").map(PathBuf::from);
        let starts = [Some(near.to_path_buf()), env::current_dir().ok()];
        let from_venv = starts.into_iter().flatten().find_map(|start| {
            let start = start.canonicalize().unwrap_or(start);
            start
                .ancestors()
                .map(|dir| dir.join(".venv/bin/python"))
                .find(|python| python.is_file())
        });
        let python = from_env
            .or(from_venv)
            .unwrap_or_else(|| PathBuf::from("python3"));

        // Manim caches typeset formulas under the working directory, so a stable one makes
        // every run after the first much faster.
        let cache = env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
            .unwrap_or_else(env::temp_dir);
        Sidecar {
            python,
            work: cache.join("suit"),
        }
    }

    pub fn python(&self) -> &Path {
        &self.python
    }

    /// Writes a probe script into the working directory.
    fn prepare(&self, script: String) -> Result<Run, String> {
        fs::create_dir_all(&self.work)
            .map_err(|e| format!("cannot create {}: {e}", self.work.display()))?;
        let id = unique();
        let run = Run {
            script: self.work.join(format!("probe-{id}.py")),
            result: self.work.join(format!("probe-{id}.json")),
        };
        fs::write(&run.script, script)
            .map_err(|e| format!("cannot write {}: {e}", run.script.display()))?;
        Ok(run)
    }

    /// Removes what earlier runs left behind in the working directory.
    fn sweep(&self) {
        let Ok(entries) = fs::read_dir(&self.work) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let ours = ["probe-", "server-", "texts-", "own-"]
                .iter()
                .any(|prefix| name.starts_with(prefix));
            let age = entry.metadata().and_then(|meta| meta.modified());
            let old = age.is_ok_and(|at| at.elapsed().is_ok_and(|age| age > STALE));
            if ours && old {
                let path = entry.path();
                let _ = match path.is_dir() {
                    true => fs::remove_dir_all(path),
                    false => fs::remove_file(path),
                };
            }
        }
    }

    /// Starts a Python that imports Manim once and then waits for probe scripts.
    fn serve(&self) -> Result<Server, String> {
        fs::create_dir_all(&self.work)
            .map_err(|e| format!("cannot create {}: {e}", self.work.display()))?;
        self.sweep();
        let id = unique();
        let script = self.work.join("server.py");
        let log = self.work.join(format!("server-{id}.log"));
        let own = self.work.join(format!("own-{id}"));
        // Another Suit may be starting its Python on this very file: replace it whole or
        // not at all.
        if fs::read_to_string(&script).ok().as_deref() != Some(SERVER) {
            let fresh = self.work.join(format!("server-{id}.py"));
            fs::write(&fresh, SERVER)
                .and_then(|()| fs::rename(&fresh, &script))
                .map_err(|e| format!("cannot write {}: {e}", script.display()))?;
        }
        let errors =
            fs::File::create(&log).map_err(|e| format!("cannot write {}: {e}", log.display()))?;
        let mut child = Command::new(&self.python)
            .arg("-u")
            .arg(&script)
            .arg(&own)
            .current_dir(&self.work)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(errors)
            .spawn()
            .map_err(|e| format!("cannot run {}: {e}", self.python.display()))?;
        let requests = child.stdin.take();
        let output = BufReader::new(child.stdout.take().expect("stdout was piped"));
        // Reading happens on a thread of its own, so that waiting for a reply can be given
        // up. The thread ends when the process closes its output.
        let (answered, replies) = mpsc::channel();
        thread::spawn(move || {
            for line in output.lines().map_while(Result::ok) {
                if line.starts_with(REPLY) && answered.send(()).is_err() {
                    return;
                }
            }
        });
        Ok(Server {
            child,
            requests,
            replies,
            log,
            own,
            usable: true,
        })
    }
}

fn last_line(text: &str) -> &str {
    text.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("no output")
}

/// A Python process with Manim already imported (`server.py`).
struct Server {
    child: Child,
    /// `None` once the process has been told there is nothing more to do.
    requests: Option<ChildStdin>,
    /// One message per request answered.
    replies: Receiver<()>,
    /// Where the process's error output goes.
    log: PathBuf,
    /// The process's own directory, for typeset text and the pixels of pictures.
    own: PathBuf,
    /// False once the process is gone or had to be stopped by force.
    usable: bool,
}

/// Marks the one line of output that answers a request.
const REPLY: char = '\u{1e}';
/// Starts a line of input that takes a request back.
const CANCEL: char = '\u{18}';

impl Server {
    fn say(&mut self, line: &str) -> bool {
        let Some(requests) = &mut self.requests else {
            return false;
        };
        let sent = requests
            .write_all(line.as_bytes())
            .and_then(|()| requests.flush());
        sent.is_ok()
    }

    /// Runs one script to completion, or until `watch` has had enough.
    fn run(&mut self, run: &Run, watch: Watch) -> Result<(), Failure> {
        let script = run.script.display().to_string();
        if !self.say(&format!("{script}\t{}\n", run.result.display())) {
            return Err(self.gone());
        }
        let started = Instant::now();
        loop {
            match self.replies.recv_timeout(GLANCE) {
                Ok(()) => return Ok(()),
                Err(RecvTimeoutError::Disconnected) => return Err(self.gone()),
                Err(RecvTimeoutError::Timeout) => {}
            }
            let failure = match ((watch.wanted)(), started.elapsed() > watch.limit) {
                (false, _) => Failure::Abandoned,
                (true, true) => Failure::TooSlow(watch.limit),
                (true, false) => continue,
            };
            // Ask nicely first: a script notices between two steps or two frames, and a
            // process that stops by itself can be used again.
            let asked = self.say(&format!("{CANCEL}{script}\n"));
            if !asked || self.replies.recv_timeout(GRACE).is_err() {
                self.usable = false;
                let _ = self.child.kill();
            }
            return Err(failure);
        }
    }

    /// Why the process is no longer there, from the end of its error output.
    fn gone(&mut self) -> Failure {
        self.usable = false;
        let log = fs::read_to_string(&self.log).unwrap_or_default();
        Failure::Broken(format!("Manim did not run: {}", last_line(&log)))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // With its input closed the process leaves by itself, between two writes rather
        // than in the middle of one.
        self.requests = None;
        let patience = Instant::now();
        while self.usable && patience.elapsed() < Duration::from_millis(500) {
            if matches!(self.child.try_wait(), Ok(Some(_)) | Err(_)) {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_file(&self.log);
        let _ = fs::remove_dir_all(&self.own);
    }
}

/// Probes decks on a Python it keeps running, so that only the first probe pays for
/// importing Manim. The process is started on first use and restarted if it dies.
pub struct Prober {
    sidecar: Sidecar,
    server: Option<Server>,
}

impl Prober {
    pub fn new(sidecar: Sidecar) -> Prober {
        Prober {
            sidecar,
            server: None,
        }
    }

    /// The Python process in use, if one has been started and is still there.
    pub fn process(&self) -> Option<u32> {
        self.server.as_ref().map(|server| server.child.id())
    }

    /// The scene after every step of the deck.
    pub fn probe(
        &mut self,
        deck: &Deck,
        sig: &Signature,
        checked: &Checked,
        watch: Watch,
    ) -> Result<Snapshots, Failure> {
        let last = deck.slides.len().saturating_sub(1);
        self.probe_slides(deck, sig, checked, 0, last, watch)
    }

    /// The scene after every step of the slides from number `have` to number `last`. The
    /// Python process remembers what the scene was like before every slide it has run, so
    /// this takes as long as those slides do, not as long as the deck up to them.
    pub fn probe_slides(
        &mut self,
        deck: &Deck,
        sig: &Signature,
        checked: &Checked,
        have: usize,
        last: usize,
        watch: Watch,
    ) -> Result<Snapshots, Failure> {
        let script = suit_codegen::probe_slides(deck, sig, checked, have, last);
        self.run(script, watch)
    }

    /// Slide `slide` as Manim plays it, frame for frame. Takes about as long as the slide
    /// is complicated, typically a second or two.
    pub fn probe_frames(
        &mut self,
        deck: &Deck,
        sig: &Signature,
        checked: &Checked,
        slide: usize,
        watch: Watch,
    ) -> Result<Clip, Failure> {
        self.run(suit_codegen::probe_frames(deck, sig, checked, slide), watch)
    }

    fn run<T: DeserializeOwned + Loaded>(
        &mut self,
        script: String,
        watch: Watch,
    ) -> Result<T, Failure> {
        let run = self.sidecar.prepare(script)?;
        let server = match &mut self.server {
            Some(server) => server,
            None => self.server.insert(self.sidecar.serve()?),
        };
        let outcome = server.run(&run, watch);
        if !server.usable {
            self.server = None;
        }
        outcome?;
        let nothing = || Failure::Broken("Manim produced no result for this deck.".to_owned());
        let mut result: T = run.read().unwrap_or_else(|| Err(nothing()))?;
        // While the Manim that wrote them is certainly still there.
        result.load();
        Ok(result)
    }
}
