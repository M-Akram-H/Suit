//! Manim as a sidecar process: the reference interpretation of the catalogue (spec §2).
//!
//! The editor never guesses what Manim would draw. It asks: a generated probe script runs
//! the deck with animations skipped and reports the outlines on scene after every step.

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use serde::Deserialize;
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

    /// Runs the deck in Manim and returns the scene after every step. Blocks for as long as
    /// Python takes: a second or two, plus LaTeX for formulas not seen before.
    pub fn probe(
        &self,
        deck: &Deck,
        sig: &Signature,
        checked: &Checked,
    ) -> Result<Snapshots, String> {
        fs::create_dir_all(&self.work)
            .map_err(|e| format!("cannot create {}: {e}", self.work.display()))?;
        let run = format!(
            "{}-{}",
            std::process::id(),
            RUNS.fetch_add(1, Ordering::Relaxed)
        );
        let script = self.work.join(format!("probe-{run}.py"));
        let result = self.work.join(format!("probe-{run}.json"));
        fs::write(&script, suit_codegen::probe(deck, sig, checked))
            .map_err(|e| format!("cannot write {}: {e}", script.display()))?;

        let output = Command::new(&self.python)
            .arg(&script)
            .arg(&result)
            .current_dir(&self.work)
            .output()
            .map_err(|e| format!("cannot run {}: {e}", self.python.display()));
        let json = fs::read_to_string(&result);
        let _ = fs::remove_file(&script);
        let _ = fs::remove_file(&result);

        let output = output?;
        match json {
            Ok(json) => {
                serde_json::from_str(&json).map_err(|e| format!("unreadable probe result: {e}"))
            }
            // No result at all: Python failed before the scene ran (Manim missing, say).
            Err(_) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let reason = stderr.lines().rev().find(|line| !line.trim().is_empty());
                Err(format!(
                    "Manim did not run: {}",
                    reason.unwrap_or("no output").trim()
                ))
            }
        }
    }
}
