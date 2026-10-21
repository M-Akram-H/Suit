//! The editing session: the open deck, its undo history, the selection, and the preview kept
//! in step with it. No user interface lives here; every front end drives a [`Session`], and
//! every change to the deck goes through one, so that all of them can be undone.

mod edit;
mod pictures;
mod present;
mod preview;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use suit_check::{Checked, Diagnostic, ObjectState, check};
use suit_core::{Command, Deck, DeckConfig, Node, NodeId, SlideContent, SlideId, VarId};
use suit_export::site;
use suit_render::{Clip, Sidecar, Snapshot, Snapshots};

pub use suit_export::{
    pdf::Pages,
    site::{Job, Progress, Report},
};
use suit_sig::Signature;

pub use crate::present::{PresentOptions, Presenter};
pub use crate::preview::{Preview, stretches};
use crate::preview::{Shared, Worker};

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
    /// Per slide, what its snapshots were made from (`preview::render_keys`), if it has any.
    snapshot_keys: Vec<Option<u64>>,
    /// Per slide, what its recording depends on (`preview::render_keys`).
    keys: Vec<u64>,
    /// The recordings made, by what they are recordings of.
    recordings: HashMap<u64, Clip>,
    /// The recordings asked for that have not arrived.
    recordings_wanted: BTreeSet<u64>,
    /// Why recordings could not be made.
    recording_problems: BTreeMap<u64, String>,
    records_ahead: bool,
    shared: Arc<Shared>,
    /// What Manim remarked on while running the deck, worded like the checker's problems.
    remarked: Vec<Diagnostic>,
}

impl Session {
    pub fn new(deck: Deck, path: Option<PathBuf>) -> Session {
        let sig = Signature::builtin();
        let checked = check(&deck, &sig);
        let mut session = Session {
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
            snapshot_keys: Vec::new(),
            keys: Vec::new(),
            recordings: HashMap::new(),
            recordings_wanted: BTreeSet::new(),
            recording_problems: BTreeMap::new(),
            records_ahead: true,
            shared: Arc::default(),
            remarked: Vec::new(),
        };
        session.unsettle();
        session
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
        let shared = self.shared.clone();
        self.worker = Some(Worker::start(sidecar, self.sig.clone(), shared, wake));
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

    /// A number that goes up with every change to the deck.
    pub fn revision(&self) -> u64 {
        self.revision
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
        self.objects_before_step(self.slide, self.step)
    }

    /// The objects that exist just before step `step` of slide `slide`; with no step,
    /// when the slide starts.
    pub fn objects_before_step(&self, slide: usize, step: Option<usize>) -> &[ObjectState] {
        let previous = step
            .and_then(|step| step.checked_sub(1))
            .and_then(|step| self.steps_of(slide).get(step));
        let after_previous = previous.and_then(|node| self.checked.after.get(&node.id));
        let at_start = self.checked.slides.get(slide).map(|report| &report.entry);
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
        let on_slide = self.problems().filter(|d| d.slide == slide);
        on_slide.map(|d| (step_of(d), d)).collect()
    }

    /// Everything that is the matter with the deck: what the checker found, and what
    /// Manim remarked on when it ran it.
    pub fn problems(&self) -> impl Iterator<Item = &Diagnostic> {
        self.checked.diagnostics.iter().chain(&self.remarked)
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

    // ── Files ────────────────────────────────────────────────────────────────

    pub fn save_as(&mut self, path: &Path) -> Result<(), String> {
        // Pictures are named from where the deck is, so a deck that moves names them anew:
        // the deck itself, and every earlier state of it that undoing could bring back.
        let (from, to) = (self.path.as_deref().and_then(Path::parent), path.parent());
        let mut deck = self.deck.clone();
        pictures::rebase(&mut deck, from, to);
        let text = serde_json::to_string_pretty(&deck).map_err(|e| e.to_string())?;
        fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        if deck != self.deck {
            let history = self.undo.iter_mut().chain(&mut self.redo);
            for (_, moment) in history {
                pictures::rebase(&mut moment.deck, from, to);
            }
            self.deck = deck;
            self.settle();
        }
        self.path = Some(path.to_path_buf());
        self.saved_revision = self.revision;
        Ok(())
    }

    /// Saves to the file the deck was opened from or last saved to.
    pub fn save(&mut self) -> Result<(), String> {
        let path = self.path.clone().ok_or("This deck has no file yet.")?;
        self.save_as(&path)
    }

    /// The deck's file name without its endings: `talk` for `talk.suit.json`.
    pub fn stem(&self) -> Option<String> {
        let name = self
            .path
            .as_ref()?
            .file_name()?
            .to_string_lossy()
            .into_owned();
        let stem = name.split('.').next().filter(|stem| !stem.is_empty());
        Some(stem.unwrap_or("deck").to_owned())
    }

    /// Where an export called `name` goes: next to the deck, which must have been saved.
    pub fn export_path(&self, name: impl FnOnce(&str) -> String) -> Result<PathBuf, String> {
        let (Some(path), Some(stem)) = (&self.path, self.stem()) else {
            return Err("Save the deck first: what is exported goes next to it.".to_owned());
        };
        Ok(path.with_file_name(name(&stem)))
    }

    /// Writes the deck as a manim-slides file next to it; returns where.
    pub fn export(&self) -> Result<PathBuf, String> {
        if self.checked.has_errors() {
            return Err("The deck has problems; fix the red ones before exporting.".to_owned());
        }
        let target = match self.path {
            Some(_) => self.export_path(|stem| format!("{stem}.py"))?,
            None => PathBuf::from("deck.py"),
        };
        let python =
            suit_codegen::export(&self.deck, &self.sig, &self.checked, &Default::default());
        fs::write(&target, python)
            .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
        Ok(target)
    }

    /// Whether what Manim showed is the whole deck as it is now, so that it can be put on
    /// paper.
    pub fn shows_every_slide(&self) -> bool {
        self.held() >= self.deck.slides.len() && self.snapshots.error.is_none()
    }

    /// Writes the deck as a PDF next to it, from what Manim showed; returns where.
    pub fn export_pdf(&self, which: Pages) -> Result<PathBuf, String> {
        let target = self.export_path(|stem| format!("{stem}.pdf"))?;
        self.write_pdf(&target, which)?;
        Ok(target)
    }

    /// Writes the deck as a PDF to `target`.
    pub fn write_pdf(&self, target: &Path, which: Pages) -> Result<(), String> {
        if self.checked.has_errors() {
            return Err("The deck has problems; fix the red ones before exporting.".to_owned());
        }
        if !self.shows_every_slide() {
            return Err("Manim has not finished the slides yet.".to_owned());
        }
        let deck = self.deck_for_manim();
        let pages = suit_export::pdf::pages(&deck, &self.snapshots, which)?;
        let title = self.stem().unwrap_or_else(|| self.title());
        let bytes = suit_export::pdf::write(&deck, &pages, &title);
        fs::write(target, bytes).map_err(|e| format!("cannot write {}: {e}", target.display()))
    }

    /// The folder a website of the deck goes to: next to it, named after it.
    pub fn website_folder(&self) -> Result<PathBuf, String> {
        self.export_path(|stem| format!("{stem}-website"))
    }

    /// What it takes to make a website of the deck as it is, into `folder`. Manim is run
    /// with `sidecar`.
    pub fn website(
        &self,
        folder: PathBuf,
        choices: WebsiteChoices,
        sidecar: Sidecar,
    ) -> Result<site::Request, String> {
        if self.checked.has_errors() {
            return Err("The deck has problems; fix the red ones before exporting.".to_owned());
        }
        let snapshots = self.shows_every_slide().then(|| self.snapshots.clone());
        Ok(site::Request {
            deck: self.deck_for_manim(),
            checked: self.checked.clone(),
            sig: self.sig.clone(),
            title: self.stem().unwrap_or_else(|| self.title()),
            folder,
            height: choices
                .height
                .filter(|&height| height != self.deck.config.height),
            notes: choices.notes,
            pdf: choices.pdf,
            snapshots,
            sidecar,
        })
    }
}

/// How a website of the deck is to be made.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WebsiteChoices {
    /// The height of the videos, if not the deck's.
    pub height: Option<u32>,
    /// Whether the notes go with the slides.
    pub notes: bool,
    /// A PDF to download from the page, and its pages.
    pub pdf: Option<Pages>,
}
