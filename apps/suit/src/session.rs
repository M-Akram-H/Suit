//! The open deck and everything derived from it.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

use eframe::egui;
use suit_check::{Checked, Diagnostic, check};
use suit_core::{Command, Deck, Node, NodeId, SlideContent};
use suit_render::{Sidecar, Snapshot, Snapshots};
use suit_sig::Signature;

/// Shown when no deck is given.
const SAMPLE: &str = include_str!("../../../examples/sample.suit.json");

pub enum Preview {
    Ready,
    Running,
    Failed(String),
}

pub struct Session {
    pub path: Option<PathBuf>,
    pub deck: Deck,
    pub sig: Signature,
    pub checked: Checked,
    /// Selected slide.
    pub slide: usize,
    /// Selected step of that slide; `None` is the state the slide starts in.
    pub step: Option<usize>,
    /// What Manim says is on scene after every step.
    pub snapshots: Snapshots,
    /// Counts the snapshot sets received, so that caches know when to refresh.
    pub generation: u64,
    pub preview: Preview,
    sidecar: Sidecar,
    worker: Option<mpsc::Receiver<Result<Snapshots, String>>>,
}

impl Session {
    pub fn open(path: Option<PathBuf>) -> Result<Session, String> {
        let text = match &path {
            Some(path) => fs::read_to_string(path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?,
            None => SAMPLE.to_owned(),
        };
        let deck: Deck = serde_json::from_str(&text).map_err(|e| format!("not a deck: {e}"))?;
        let sig = Signature::builtin();
        let checked = check(&deck, &sig);
        let near = path
            .as_deref()
            .and_then(Path::parent)
            .unwrap_or(Path::new("."));
        Ok(Session {
            sidecar: Sidecar::locate(near),
            path,
            deck,
            sig,
            checked,
            slide: 0,
            step: None,
            snapshots: Snapshots::default(),
            generation: 0,
            preview: Preview::Ready,
            worker: None,
        })
    }

    pub fn title(&self) -> String {
        match &self.path {
            Some(path) => path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            None => "Sample deck".to_owned(),
        }
    }

    /// Asks Manim, on another thread, what the deck looks like after every step.
    pub fn refresh_preview(&mut self, ctx: &egui::Context) {
        let (sender, receiver) = mpsc::channel();
        let (deck, sig, checked) = (self.deck.clone(), self.sig.clone(), self.checked.clone());
        let (sidecar, ctx) = (self.sidecar.clone(), ctx.clone());
        thread::spawn(move || {
            let _ = sender.send(sidecar.probe(&deck, &sig, &checked));
            ctx.request_repaint();
        });
        self.worker = Some(receiver);
        self.preview = Preview::Running;
    }

    /// Takes the preview from the worker if it has finished.
    pub fn poll(&mut self) {
        let Some(result) = self
            .worker
            .as_ref()
            .and_then(|worker| worker.try_recv().ok())
        else {
            return;
        };
        self.worker = None;
        self.generation += 1;
        match result {
            Ok(snapshots) => {
                self.preview = match &snapshots.error {
                    Some(error) => Preview::Failed(error.clone()),
                    None => Preview::Ready,
                };
                self.snapshots = snapshots;
            }
            Err(error) => {
                self.preview = Preview::Failed(error);
                self.snapshots = Snapshots::default();
            }
        }
    }

    /// The steps of a slide; empty for a media slide.
    pub fn steps_of(&self, slide: usize) -> &[Node<Command>] {
        match self.deck.slides.get(slide).map(|slide| &slide.content) {
            Some(SlideContent::Blueprint(blueprint)) => &blueprint.body,
            _ => &[],
        }
    }

    pub fn steps(&self) -> &[Node<Command>] {
        self.steps_of(self.slide)
    }

    pub fn selected(&self) -> Option<&Node<Command>> {
        self.steps().get(self.step?)
    }

    /// The scene for the current selection, if Manim got that far.
    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshots
            .find(self.slide, self.selected().map(|node| node.id))
    }

    pub fn select(&mut self, slide: usize, step: Option<usize>) {
        self.slide = slide.min(self.deck.slides.len().saturating_sub(1));
        let count = self.steps().len();
        self.step = step.filter(|&step| step < count);
    }

    /// Moves the selection by whole steps, crossing slide boundaries.
    pub fn move_step(&mut self, forward: bool) {
        let count = self.steps().len();
        match (forward, self.step) {
            (true, None) if count > 0 => self.step = Some(0),
            (true, Some(step)) if step + 1 < count => self.step = Some(step + 1),
            (true, _) if self.slide + 1 < self.deck.slides.len() => {
                self.select(self.slide + 1, None)
            }
            (false, Some(0)) => self.step = None,
            (false, Some(step)) => self.step = Some(step - 1),
            (false, None) if self.slide > 0 => {
                let last = self.steps_of(self.slide - 1).len().checked_sub(1);
                self.select(self.slide - 1, last);
            }
            _ => {}
        }
    }

    /// For every node of the deck: the slide and step it belongs to.
    pub fn locations(&self) -> BTreeMap<NodeId, (usize, usize)> {
        let mut locations = BTreeMap::new();
        for slide in 0..self.deck.slides.len() {
            for (step, node) in self.steps_of(slide).iter().enumerate() {
                locations.extend(node.ids().into_iter().map(|id| (id, (slide, step))));
            }
        }
        locations
    }

    /// The problems reported on `slide`, each with the step it concerns if it has one.
    pub fn problems_of(&self, slide: usize) -> Vec<(Option<usize>, &Diagnostic)> {
        let locations = self.locations();
        let step_of = |d: &Diagnostic| {
            d.node
                .and_then(|id| locations.get(&id))
                .map(|&(_, step)| step)
        };
        let on_slide = self.checked.diagnostics.iter().filter(|d| d.slide == slide);
        on_slide.map(|d| (step_of(d), d)).collect()
    }

    /// Writes the deck as a manim-slides file next to it; returns where.
    pub fn export(&self) -> Result<PathBuf, String> {
        if self.checked.has_errors() {
            return Err("The deck has errors; fix them before exporting.".to_owned());
        }
        let target = match &self.path {
            Some(path) => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let stem = name.split('.').next().unwrap_or("deck").to_owned();
                path.with_file_name(format!("{stem}.py"))
            }
            None => PathBuf::from("sample.py"),
        };
        let python =
            suit_codegen::export(&self.deck, &self.sig, &self.checked, &Default::default());
        fs::write(&target, python)
            .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
        Ok(target)
    }
}
