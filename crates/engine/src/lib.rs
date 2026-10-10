//! The editing session: the open deck, its undo history, the selection, and the preview kept
//! in step with it. No user interface lives here; every front end drives a [`Session`], and
//! every change to the deck goes through one, so that all of them can be undone.

mod edit;
mod preview;

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use suit_check::{Checked, Diagnostic, ObjectState, check};
use suit_core::{Command, Deck, DeckConfig, Node, NodeId, SlideContent, SlideId, VarId};
use suit_render::{Sidecar, Snapshot, Snapshots};
use suit_sig::Signature;

pub use crate::preview::Preview;
use crate::preview::Worker;

/// The deck shown when none is opened.
const SAMPLE: &str = include_str!("../../../examples/sample.suit.json");

/// Changes to the same thing within this long of each other are one undo step: dragging a
/// number through fifty values should not take fifty undos to take back.
const MERGE_WINDOW: Duration = Duration::from_millis(900);

/// What a change was made to, for merging runs of changes into one undo step.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Step(NodeId),
    Slide(SlideId),
    Name(VarId),
}

/// The deck and selection at one point in the history.
struct Moment {
    deck: Deck,
    slide: usize,
    step: Option<usize>,
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
    /// What Manim says is on scene after every step, as of the last preview.
    pub snapshots: Snapshots,
    /// Counts the previews received, so that caches know when to refresh.
    pub generation: u64,
    pub preview: Preview,
    /// Counts the changes made to the deck.
    revision: u64,
    saved_revision: u64,
    undo: Vec<(String, Moment)>,
    redo: Vec<(String, Moment)>,
    last_change: Option<(Target, Instant)>,
    changed_at: Instant,
    worker: Option<Worker>,
    /// The revision last sent off for a preview.
    requested: Option<u64>,
    /// The revision the snapshots were made from.
    shown: Option<u64>,
}

impl Session {
    pub fn new(deck: Deck, path: Option<PathBuf>) -> Session {
        let sig = Signature::builtin();
        let checked = check(&deck, &sig);
        Session {
            path,
            deck,
            sig,
            checked,
            slide: 0,
            step: None,
            snapshots: Snapshots::default(),
            generation: 0,
            preview: Preview::Ready,
            revision: 0,
            saved_revision: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            last_change: None,
            changed_at: Instant::now(),
            worker: None,
            requested: None,
            shown: None,
        }
    }

    pub fn open(path: &Path) -> Result<Session, String> {
        let text =
            fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let deck = serde_json::from_str(&text)
            .map_err(|e| format!("{} is not a deck: {e}", path.display()))?;
        Ok(Session::new(deck, Some(path.to_path_buf())))
    }

    pub fn sample() -> Session {
        let deck = serde_json::from_str(SAMPLE).expect("the bundled sample deck is valid");
        Session::new(deck, None)
    }

    /// A deck with one empty slide.
    pub fn blank() -> Session {
        let mut deck = Deck::new(DeckConfig::default());
        deck.add_slide("Slide 1");
        Session::new(deck, None)
    }

    /// Keeps a preview up to date in the background, using the Python `sidecar` found.
    /// `wake` is called, from another thread, whenever a new preview is ready to be taken.
    pub fn with_preview(mut self, sidecar: Sidecar, wake: impl Fn() + Send + 'static) -> Session {
        self.worker = Some(Worker::start(sidecar, self.sig.clone(), wake));
        self
    }

    // ── What is open ─────────────────────────────────────────────────────────

    pub fn title(&self) -> String {
        match &self.path {
            Some(path) => path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            None => "Untitled".to_owned(),
        }
    }

    /// Whether there are changes that have not been saved.
    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
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

    /// The objects that exist at the selected point, with their kinds and whether they are
    /// on the slide there.
    pub fn objects(&self) -> &[ObjectState] {
        let after_step = self
            .selected()
            .and_then(|node| self.checked.after.get(&node.id));
        let at_start = self
            .checked
            .slides
            .get(self.slide)
            .map(|report| &report.entry);
        after_step.or(at_start).map_or(&[], Vec::as_slice)
    }

    /// The objects that exist just before the selected step: what that step can refer to.
    pub fn objects_before(&self) -> &[ObjectState] {
        let previous = self
            .step
            .and_then(|step| step.checked_sub(1))
            .and_then(|step| self.steps().get(step));
        let after_previous = previous.and_then(|node| self.checked.after.get(&node.id));
        let at_start = self
            .checked
            .slides
            .get(self.slide)
            .map(|report| &report.entry);
        after_previous.or(at_start).map_or(&[], Vec::as_slice)
    }

    pub fn name(&self, var: VarId) -> &str {
        self.deck.names.get(&var).map_or("?", String::as_str)
    }

    /// The slide and step where `var` is made.
    pub fn creation(&self, var: VarId) -> Option<(usize, usize)> {
        (0..self.deck.slides.len()).find_map(|slide| {
            let made = |node: &Node<Command>| matches!(&node.item, Command::LetObject { var: made, .. } if *made == var);
            self.steps_of(slide).iter().position(made).map(|step| (slide, step))
        })
    }

    /// For every node of the deck: the slide and step it belongs to.
    pub fn locations(&self) -> std::collections::BTreeMap<NodeId, (usize, usize)> {
        let mut locations = std::collections::BTreeMap::new();
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

    // ── Selection ────────────────────────────────────────────────────────────

    pub fn select(&mut self, slide: usize, step: Option<usize>) {
        self.slide = slide.min(self.deck.slides.len().saturating_sub(1));
        let count = self.steps().len();
        self.step = step.filter(|&step| step < count);
    }

    /// Moves the selection by one step, crossing slide boundaries.
    pub fn step_selection(&mut self, forward: bool) {
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

    // ── Preview ──────────────────────────────────────────────────────────────

    /// The scene for the selection. While a preview of the latest change is still on its
    /// way, this is the nearest earlier step Manim has already shown.
    pub fn snapshot(&self) -> Option<&Snapshot> {
        let steps = self.steps();
        let after = |step: usize| {
            let id = steps.get(step)?.id;
            self.snapshots
                .steps
                .iter()
                .find(|snapshot| snapshot.node == Some(id))
        };
        let selected = self.step.into_iter().flat_map(|step| (0..=step).rev());
        selected
            .filter_map(after)
            .next()
            .or_else(|| self.snapshots.find(self.slide, None))
    }

    /// Whether the snapshots are of the deck as it is now, rather than of an earlier state
    /// while Manim catches up.
    pub fn preview_is_current(&self) -> bool {
        self.shown == Some(self.revision)
    }

    // ── Files ────────────────────────────────────────────────────────────────

    pub fn save_as(&mut self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(&self.deck).map_err(|e| e.to_string())?;
        fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        self.path = Some(path.to_path_buf());
        self.saved_revision = self.revision;
        Ok(())
    }

    /// Saves to the file the deck was opened from or last saved to.
    pub fn save(&mut self) -> Result<(), String> {
        let path = self.path.clone().ok_or("This deck has no file yet.")?;
        self.save_as(&path)
    }

    /// Writes the deck as a manim-slides file next to it; returns where.
    pub fn export(&self) -> Result<PathBuf, String> {
        if self.checked.has_errors() {
            return Err("The deck has problems; fix the red ones before exporting.".to_owned());
        }
        let target = match &self.path {
            Some(path) => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let stem = name.split('.').next().unwrap_or("deck").to_owned();
                path.with_file_name(format!("{stem}.py"))
            }
            None => PathBuf::from("deck.py"),
        };
        let python =
            suit_codegen::export(&self.deck, &self.sig, &self.checked, &Default::default());
        fs::write(&target, python)
            .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
        Ok(target)
    }
}
