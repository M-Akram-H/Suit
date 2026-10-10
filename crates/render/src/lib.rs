//! Manim as a sidecar process: the reference interpretation of the catalogue (spec §2).
//!
//! The editor never guesses what Manim would draw. It asks: a generated probe script runs
//! the deck with animations skipped and reports the outlines on scene after every step.

use std::{
    collections::BTreeMap,
    env, fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, de::DeserializeOwned};
use suit_check::Checked;
use suit_core::{Deck, NodeId, VarId};
use suit_sig::Signature;

/// The scene after every step of a deck, in deck order.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Snapshots {
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

static RUNS: AtomicU64 = AtomicU64::new(0);

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
    fn read<T: DeserializeOwned>(&self) -> Option<Result<T, String>> {
        let json = fs::read_to_string(&self.result).ok()?;
        Some(serde_json::from_str(&json).map_err(|e| format!("unreadable probe result: {e}")))
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

        // Manim caches typeset text and formulas under the working directory, so a stable
        // one makes every run after the first much faster.
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
        let id = format!(
            "{}-{}",
            std::process::id(),
            RUNS.fetch_add(1, Ordering::Relaxed)
        );
        let run = Run {
            script: self.work.join(format!("probe-{id}.py")),
            result: self.work.join(format!("probe-{id}.json")),
        };
        fs::write(&run.script, script)
            .map_err(|e| format!("cannot write {}: {e}", run.script.display()))?;
        Ok(run)
    }

    /// Runs the deck in a fresh Python and returns the scene after every step. Takes a few
    /// seconds, most of them Manim being imported; [`Prober`] avoids paying that every time.
    pub fn probe(
        &self,
        deck: &Deck,
        sig: &Signature,
        checked: &Checked,
    ) -> Result<Snapshots, String> {
        let run = self.prepare(suit_codegen::probe(deck, sig, checked))?;
        let output = Command::new(&self.python)
            .arg(&run.script)
            .arg(&run.result)
            .current_dir(&self.work)
            .output()
            .map_err(|e| format!("cannot run {}: {e}", self.python.display()))?;
        run.read().unwrap_or_else(|| {
            // No result at all: Python failed before the scene ran (Manim missing, say).
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Manim did not run: {}", last_line(&stderr)))
        })
    }

    /// Starts a Python that imports Manim once and then waits for probe scripts.
    fn serve(&self) -> Result<Server, String> {
        fs::create_dir_all(&self.work)
            .map_err(|e| format!("cannot create {}: {e}", self.work.display()))?;
        let script = self.work.join("server.py");
        let log = self.work.join(format!("server-{}.log", std::process::id()));
        fs::write(&script, include_str!("server.py"))
            .map_err(|e| format!("cannot write {}: {e}", script.display()))?;
        let errors =
            fs::File::create(&log).map_err(|e| format!("cannot write {}: {e}", log.display()))?;
        let mut child = Command::new(&self.python)
            .arg("-u")
            .arg(&script)
            .current_dir(&self.work)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(errors)
            .spawn()
            .map_err(|e| format!("cannot run {}: {e}", self.python.display()))?;
        let requests = child.stdin.take().expect("stdin was piped");
        let replies = BufReader::new(child.stdout.take().expect("stdout was piped"));
        Ok(Server {
            child,
            requests,
            replies,
            log,
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
    requests: ChildStdin,
    replies: BufReader<ChildStdout>,
    /// Where the process's error output goes.
    log: PathBuf,
}

/// Marks the one line of output that answers a request.
const REPLY: char = '\u{1e}';

impl Server {
    /// Runs one script to completion. `Err` means the process is gone.
    fn run(&mut self, run: &Run) -> Result<(), String> {
        let request = format!("{}\t{}\n", run.script.display(), run.result.display());
        let sent = self
            .requests
            .write_all(request.as_bytes())
            .and_then(|()| self.requests.flush());
        if sent.is_err() {
            return Err(self.gone());
        }
        let mut line = String::new();
        loop {
            line.clear();
            match self.replies.read_line(&mut line) {
                Ok(0) | Err(_) => return Err(self.gone()),
                Ok(_) if line.starts_with(REPLY) => return Ok(()),
                Ok(_) => {}
            }
        }
    }

    /// Why the process is no longer there, from the end of its error output.
    fn gone(&self) -> String {
        let log = fs::read_to_string(&self.log).unwrap_or_default();
        format!("Manim did not run: {}", last_line(&log))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_file(&self.log);
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

    /// The scene after every step of the deck.
    pub fn probe(
        &mut self,
        deck: &Deck,
        sig: &Signature,
        checked: &Checked,
    ) -> Result<Snapshots, String> {
        self.run(suit_codegen::probe(deck, sig, checked))
    }

    /// Slide `slide` as Manim plays it, frame for frame. Takes about as long as the slide
    /// is complicated, typically a second or two.
    pub fn probe_frames(
        &mut self,
        deck: &Deck,
        sig: &Signature,
        checked: &Checked,
        slide: usize,
    ) -> Result<Clip, String> {
        self.run(suit_codegen::probe_frames(deck, sig, checked, slide))
    }

    fn run<T: DeserializeOwned>(&mut self, script: String) -> Result<T, String> {
        let run = self.sidecar.prepare(script)?;
        let server = match &mut self.server {
            Some(server) => server,
            None => self.server.insert(self.sidecar.serve()?),
        };
        if let Err(reason) = server.run(&run) {
            self.server = None;
            return Err(reason);
        }
        run.read()
            .unwrap_or_else(|| Err("Manim produced no result for this deck.".to_owned()))
    }
}
