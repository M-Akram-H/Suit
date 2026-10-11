//! Keeping the preview in step with the deck, off the thread that draws.
//!
//! One thread owns the Python process. It answers the newest request for the steps first,
//! because the canvas waits for those, and records slides in between: the ones asked for
//! (Play, Present), then the rest of the deck, so that playing a slide does not start with
//! a wait. A recording is given up the moment something more urgent turns up.

use std::{
    collections::{BTreeSet, VecDeque, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread,
    time::Duration,
};

use suit_check::{Checked, Diagnostic, Severity};
use suit_codegen::formula::{self, Names};
use suit_core::{Arg, Command, Deck, Expr, Object, SlideContent, VarId};
use suit_render::{Clip, Failure, Prober, Sidecar, Snapshots, Watch};
use suit_sig::Signature;

use crate::Session;

/// How long the deck has to hold still before Manim is asked about it.
const DEBOUNCE: Duration = Duration::from_millis(120);

/// How long the deck has to hold still before the slides nobody asked for are recorded.
const IDLE: Duration = Duration::from_millis(700);

/// How long Manim may take over the steps of a deck. Typesetting a deck full of new
/// formulas for the first time takes a second each, so this is generous.
const STEPS_LIMIT: Duration = Duration::from_secs(180);

/// How long Manim may take to record a slide: this, and a little for every frame.
const RECORDING_LIMIT: Duration = Duration::from_secs(60);
const PER_FRAME: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, PartialEq)]
pub enum Preview {
    /// The preview shows the deck as it is.
    Ready,
    /// A preview of the latest change is on its way; what is shown is older.
    Running,
    /// Manim stopped at some step; what it got through is still shown.
    Failed(String),
    /// Manim was stopped by hand before it finished; what is shown is older.
    Stopped,
}

/// What the session and its worker both look at.
#[derive(Default)]
pub(crate) struct Shared {
    /// Per slide, what its recording depends on; see [`render_keys`].
    keys: Mutex<Vec<u64>>,
    /// Whether the canvas has what it needs, so that there is time for recordings.
    quiet: AtomicBool,
    /// Counts the recordings asked for by hand, which go before the others.
    asks: AtomicU64,
    /// Counts the times everything was told to stop.
    stops: AtomicU64,
    /// Counts the changes to the deck.
    revision: AtomicU64,
}

impl Shared {
    fn key_of(&self, slide: usize) -> Option<u64> {
        self.keys.lock().ok()?.get(slide).copied()
    }
}

/// Stretches of the number line, as people write them: `[-1, -0.1] ∪ [0.1, 1]`.
pub fn stretches(pieces: &[[f64; 2]]) -> String {
    let number = |value: f64| {
        let rounded = (value * 100.0).round() / 100.0;
        // Rounding can leave a minus zero behind.
        format!("{}", rounded + 0.0)
    };
    let written: Vec<String> = pieces
        .iter()
        .map(|[from, to]| format!("[{}, {}]", number(*from), number(*to)))
        .collect();
    written.join(" ∪ ")
}

/// For every slide, a number that changes exactly when what Manim draws for it may have:
/// with the frame, and with the steps of that slide or of any slide before it. Titles,
/// notes and names do not come into it, and neither do later slides.
pub(crate) fn render_keys(deck: &Deck) -> Vec<u64> {
    let mut hasher = DefaultHasher::new();
    serde_json::to_string(&deck.config)
        .unwrap_or_default()
        .hash(&mut hasher);
    let mut keys = Vec::with_capacity(deck.slides.len());
    for slide in &deck.slides {
        serde_json::to_string(&slide.content)
            .unwrap_or_default()
            .hash(&mut hasher);
        // A looping slide has no hold at its end.
        slide.config.looping.hash(&mut hasher);
        keys.push(hasher.clone().finish());
    }
    keys
}

struct Request {
    revision: u64,
    deck: Arc<Deck>,
    checked: Arc<Checked>,
    /// What each slide of this deck looks like depends on; see [`render_keys`].
    keys: Arc<Vec<u64>>,
    work: Work,
}

#[derive(Clone, Copy, PartialEq)]
enum Work {
    /// The scene after every step of the slides from number `have` on; the session has
    /// the ones before. The slides up to the `selected` one are answered first, because
    /// the canvas shows one of those, and the rest after. `rest` marks that second part.
    Steps {
        have: usize,
        selected: usize,
        rest: bool,
    },
    /// The frames of one slide. `asked` is true if someone is waiting for them.
    Record { slide: usize, key: u64, asked: bool },
}

enum Reply {
    /// The scene after every step of slides `first` to `last` of a deck with these `keys`.
    Steps {
        revision: u64,
        keys: Arc<Vec<u64>>,
        first: usize,
        last: usize,
        /// False if the slides after `last` are still to come.
        done: bool,
        result: Result<Snapshots, Failure>,
    },
    Clip {
        key: u64,
        result: Result<Clip, Failure>,
    },
}

pub(crate) struct Worker {
    requests: Sender<Request>,
    replies: Receiver<Reply>,
}

impl Worker {
    pub fn start(
        sidecar: Sidecar,
        sig: Signature,
        shared: Arc<Shared>,
        wake: impl Fn() + Send + 'static,
    ) -> Worker {
        let (requests, inbox) = mpsc::channel::<Request>();
        let (outbox, replies) = mpsc::channel();
        thread::spawn(move || {
            let mut prober = Prober::new(sidecar);
            // Recordings still to be made: the ones asked for, and the rest.
            let mut asked: VecDeque<Request> = VecDeque::new();
            let mut ahead: VecDeque<Request> = VecDeque::new();
            let mut steps: Option<Request> = None;
            loop {
                // With nothing to do, wait; with recordings held back until the canvas has
                // been served, look again shortly. The wait ends for good when the session,
                // and with it the sender, is dropped.
                let idle = steps.is_none() && asked.is_empty() && ahead.is_empty();
                let held_back = steps.is_none() && !shared.quiet.load(Ordering::Relaxed);
                let first = match (idle, held_back) {
                    (true, _) => match inbox.recv() {
                        Ok(request) => Some(request),
                        Err(_) => return,
                    },
                    (false, true) => match inbox.recv_timeout(Duration::from_millis(15)) {
                        Ok(request) => Some(request),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => return,
                    },
                    (false, false) => None,
                };
                for request in first.into_iter().chain(inbox.try_iter()) {
                    match request.work {
                        Work::Steps { .. } => steps = Some(request),
                        Work::Record { asked: true, .. } => asked.push_back(request),
                        Work::Record { asked: false, .. } => ahead.push_back(request),
                    }
                }
                // A recording of a slide as it used to be is of no use to anyone, and
                // neither is the same recording twice.
                let mut seen = BTreeSet::new();
                for queue in [&mut asked, &mut ahead] {
                    queue.retain(|request| {
                        let Work::Record { slide, key, .. } = request.work else {
                            return false;
                        };
                        let current = shared.key_of(slide) == Some(key);
                        if !current {
                            let result = Err(Failure::Abandoned);
                            let _ = outbox.send(Reply::Clip { key, result });
                        }
                        current && seen.insert(key)
                    });
                }

                let stops = shared.stops.load(Ordering::Relaxed);
                let stopped = || shared.stops.load(Ordering::Relaxed) != stops;
                let reply = if let Some(request) = steps.take() {
                    let Work::Steps {
                        have,
                        selected,
                        rest,
                    } = request.work
                    else {
                        continue;
                    };
                    let end = request.deck.slides.len().saturating_sub(1);
                    // The slide on the canvas first, if there is more than that to do.
                    let last = if have <= selected && selected < end {
                        selected
                    } else {
                        end
                    };
                    // What the canvas waits for is seen through. The slides after it are
                    // given up as soon as the deck has changed again.
                    let revision = request.revision;
                    let current = || shared.revision.load(Ordering::Relaxed) == revision;
                    let wanted = || !stopped() && (!rest || current());
                    let watch = Watch {
                        limit: STEPS_LIMIT,
                        wanted: &wanted,
                    };
                    let (deck, checked) = (&request.deck, &request.checked);
                    let result = prober.probe_slides(deck, &sig, checked, have, last, watch);
                    let whole = result.as_ref().is_ok_and(|shown| shown.error.is_none());
                    let done = last == end || !whole;
                    let keys = request.keys.clone();
                    if !done {
                        // Unless something newer comes in first, which the top of the loop
                        // will find.
                        let work = Work::Steps {
                            have: last + 1,
                            selected: usize::MAX,
                            rest: true,
                        };
                        steps = Some(Request { work, ..request });
                    }
                    Reply::Steps {
                        revision,
                        keys,
                        first: have,
                        last,
                        done,
                        result,
                    }
                } else if !shared.quiet.load(Ordering::Relaxed) {
                    continue;
                } else if let Some(request) = asked.pop_front().or_else(|| ahead.pop_front()) {
                    let Work::Record {
                        slide,
                        key,
                        asked: by_hand,
                    } = request.work
                    else {
                        continue;
                    };
                    let asks = shared.asks.load(Ordering::Relaxed);
                    // Given up for the canvas, for a slide someone is waiting for, or
                    // because the slide has changed; taken up again unless it is the last.
                    let current = || shared.key_of(slide) == Some(key);
                    let wanted = || {
                        shared.quiet.load(Ordering::Relaxed)
                            && current()
                            && !stopped()
                            && (by_hand || shared.asks.load(Ordering::Relaxed) == asks)
                    };
                    let frames = request.checked.slides.get(slide);
                    let frames = frames.map_or(0, |report| report.frames + report.gap_frames);
                    let watch = Watch {
                        limit: RECORDING_LIMIT + PER_FRAME * frames,
                        wanted: &wanted,
                    };
                    let (deck, checked) = (&request.deck, &request.checked);
                    let result = prober.probe_frames(deck, &sig, checked, slide, watch);
                    if result.as_ref().err() == Some(&Failure::Abandoned) && current() && !stopped()
                    {
                        match by_hand {
                            true => asked.push_front(request),
                            false => ahead.push_front(request),
                        }
                        continue;
                    }
                    Reply::Clip { key, result }
                } else {
                    continue;
                };
                if outbox.send(reply).is_err() {
                    return;
                }
                wake();
            }
        });
        Worker { requests, replies }
    }
}

impl Session {
    /// Takes any finished preview and asks for a new one if the deck has changed. Call it
    /// often, for example once per frame. Returns how soon it wants to be called again when
    /// it is waiting for something that no event will announce.
    pub fn tick(&mut self) -> Option<Duration> {
        let worker = self.worker.as_ref()?;
        let replies: Vec<Reply> = worker.replies.try_iter().collect();
        for reply in replies {
            match reply {
                steps @ Reply::Steps { .. } => self.take_steps(steps),
                Reply::Clip { key, result } => {
                    self.recordings_wanted.remove(&key);
                    let whole = result.and_then(|clip| match clip.error.clone() {
                        Some(error) => Err(Failure::Broken(error)),
                        None => Ok(clip),
                    });
                    match whole {
                        Ok(clip) => self.keep_recording(key, clip),
                        // Given up for something more urgent; it will be asked for again.
                        Err(Failure::Abandoned) => {}
                        Err(problem) => {
                            self.recording_problems.insert(key, problem.to_string());
                        }
                    }
                }
            }
        }

        if self.requested != Some(self.revision) {
            let waited = self.changed_at.elapsed();
            if waited < DEBOUNCE {
                self.preview = Preview::Running;
                return Some(DEBOUNCE - waited);
            }
            let have = self.held();
            if have >= self.deck.slides.len() {
                // What changed is nothing Manim draws: a title, a note, a name. Or the
                // deck lost its last slides, and what is left is as it was.
                let count = self.deck.slides.len();
                self.snapshots.steps.retain(|step| step.slide < count);
                self.snapshots.remarks.retain(|remark| remark.slide < count);
                self.snapshot_keys.truncate(count);
                self.note_remarks();
                self.requested = Some(self.revision);
                self.preview = Preview::Ready;
                self.shared.quiet.store(true, Ordering::Relaxed);
                return None;
            }
            let work = Work::Steps {
                have,
                selected: self.slide,
                rest: false,
            };
            if self.send(&[work]) {
                self.requested = Some(self.revision);
                self.preview = Preview::Running;
            }
            return None;
        }
        // With the canvas served and the deck at rest, record the slides ahead of anyone
        // asking, so that playing and presenting start at once.
        if self.records_ahead && self.preview == Preview::Ready {
            let waited = self.changed_at.elapsed();
            if waited < IDLE {
                return Some(IDLE - waited);
            }
            self.record_ahead();
        }
        None
    }

    /// Puts what Manim remarked on into words, to be listed with the checker's problems.
    pub(crate) fn note_remarks(&mut self) {
        let worded = |remark: &suit_render::Remark| {
            let name = self.name(remark.var);
            let (message, help) = match remark.what.as_str() {
                "undefined" => (
                    format!("Manim draws nothing of {name}: some of its points are not numbers."),
                    "If it is a graph, its rule has no value somewhere in its range, like the \
                     logarithm of a negative number. Give it a range where it has one."
                        .to_owned(),
                ),
                // A graph that was told where to draw, and whose rule has no value on
                // some of that. Left to itself it looks only where there is one, and
                // there is nothing to say.
                "plot" if remark.missing && remark.chosen => {
                    let rule = self.rule_of(remark.var).unwrap_or_else(|| name.to_owned());
                    let asked = stretches(&remark.asked);
                    let help = match remark.pieces.is_empty() {
                        true => "It has no value anywhere there, so nothing is drawn.".to_owned(),
                        false => format!(
                            "It is drawn where it has a value: {}.",
                            stretches(&remark.pieces)
                        ),
                    };
                    (
                        format!("The function “{rule}” is not defined on all of {asked}."),
                        help,
                    )
                }
                "plot" => return None,
                other => (format!("Manim remarks on {name}: {other}."), String::new()),
            };
            Some(Diagnostic {
                severity: Severity::Warning,
                code: "manim",
                slide: remark.slide,
                node: remark.node,
                message,
                help: Some(help).filter(|help| !help.is_empty()),
            })
        };
        self.remarked = self.snapshots.remarks.iter().filter_map(worded).collect();
    }

    /// The rule of the graph `var`, written as it is typed.
    fn rule_of(&self, var: VarId) -> Option<String> {
        fn rule(object: &Object) -> Option<&Expr> {
            match object {
                Object::New { args, .. } => args.iter().find_map(|(_, arg)| match arg {
                    Arg::Fun { body, .. } => Some(&**body),
                    _ => None,
                }),
                Object::Chain { base, .. } => rule(&base.item),
                _ => None,
            }
        }
        let (slide, step) = self.creation(var)?;
        let Command::LetObject { value, .. } = &self.steps_of(slide)[step].item else {
            return None;
        };
        let names = Names {
            deck: &self.deck,
            sig: &self.sig,
            objects: &[],
            values: &[],
        };
        Some(formula::write(rule(&value.item)?, &names))
    }

    /// What the graph `var` found out about its rule when Manim made it: where it drew,
    /// and the asymptotes it came across.
    pub fn found_by_graph(&self, var: VarId) -> Option<&suit_render::Remark> {
        let remarks = self.snapshots.remarks.iter().rev();
        remarks
            .into_iter()
            .find(|remark| remark.var == var && remark.what == "plot")
    }

    /// How many slides, from the first, the snapshots show as they are now.
    fn held(&self) -> usize {
        let current = |(have, now): &(&Option<u64>, &u64)| **have == Some(**now);
        self.snapshot_keys
            .iter()
            .zip(&self.keys)
            .take_while(current)
            .count()
    }

    /// Whether the canvas shows the selected slide as it is now, rather than as it was
    /// while Manim catches up.
    pub fn preview_is_current(&self) -> bool {
        self.held() > self.slide
    }

    fn take_steps(&mut self, reply: Reply) {
        let Reply::Steps {
            revision,
            keys,
            first,
            last,
            done,
            result,
        } = reply
        else {
            return;
        };
        // An answer about a deck that has since been asked about again is not merged in:
        // what the session holds would no longer be what it said it held when it asked.
        if self.requested != Some(revision) {
            return;
        }
        self.generation += 1;
        match result {
            Ok(shown) => {
                // Where Manim stopped, if it did: that slide is there in part, and the
                // ones after it not at all.
                let stuck = shown.error.as_ref().map(|_| {
                    let reached = shown.steps.last().map_or(first, |step| step.slide);
                    reached.max(first)
                });
                let through = if done { usize::MAX } else { last };
                let kept =
                    |step: &suit_render::Snapshot| step.slide < first || step.slide > through;
                self.snapshots.steps.retain(kept);
                self.snapshots.steps.extend(shown.steps);
                self.snapshots.steps.sort_by_key(|step| step.slide);
                self.snapshots.error = shown.error;
                let stays =
                    |remark: &suit_render::Remark| remark.slide < first || remark.slide > through;
                self.snapshots.remarks.retain(stays);
                self.snapshots.remarks.extend(shown.remarks);
                self.note_remarks();

                self.snapshot_keys.resize(keys.len(), None);
                for slide in first..keys.len() {
                    self.snapshot_keys[slide] = match stuck {
                        Some(stuck) if slide >= stuck => None,
                        _ if slide <= last => Some(keys[slide]),
                        _ if done => None,
                        _ => self.snapshot_keys[slide],
                    };
                }
                self.preview = match (&self.snapshots.error, done) {
                    (Some(error), _) => Preview::Failed(error.clone()),
                    (None, true) => Preview::Ready,
                    (None, false) => Preview::Running,
                };
            }
            Err(Failure::Abandoned) => self.preview = Preview::Stopped,
            // Python itself did not run; what was shown before is better than nothing.
            Err(reason) => self.preview = Preview::Failed(reason.to_string()),
        }
        if self.preview != Preview::Running {
            self.shared.quiet.store(true, Ordering::Relaxed);
        }
    }

    /// Uses `snapshots` as what Manim shows of the deck as it is now. For front ends that
    /// obtain them some other way, and for tests.
    pub fn install_snapshots(&mut self, snapshots: Snapshots) {
        self.snapshots = snapshots;
        self.snapshot_keys = self.keys.iter().map(|key| Some(*key)).collect();
        self.generation += 1;
        self.note_remarks();
    }

    /// Hands the worker the deck as it is, with `work` to do on it.
    fn send(&self, work: &[Work]) -> bool {
        let Some(worker) = &self.worker else {
            return false;
        };
        let (deck, checked) = (
            Arc::new(self.deck_for_manim()),
            Arc::new(self.checked.clone()),
        );
        let keys = Arc::new(self.keys.clone());
        work.iter().all(|&work| {
            let request = Request {
                revision: self.revision,
                deck: deck.clone(),
                checked: checked.clone(),
                keys: keys.clone(),
                work,
            };
            worker.requests.send(request).is_ok()
        })
    }

    /// The deck changed: the canvas comes first again.
    pub(crate) fn unsettle(&mut self) {
        self.keys = render_keys(&self.deck);
        if let Ok(mut keys) = self.shared.keys.lock() {
            keys.clone_from(&self.keys);
        }
        self.shared.quiet.store(false, Ordering::Relaxed);
        self.shared.revision.store(self.revision, Ordering::Relaxed);
    }

    /// Asks for a fresh preview even though the deck has not changed.
    pub fn refresh_preview(&mut self) {
        self.requested = None;
        self.snapshot_keys.clear();
        self.shared.quiet.store(false, Ordering::Relaxed);
    }

    /// Tells Manim to stop whatever it is doing for this deck. The preview stays as it is
    /// until the deck changes or [`Session::refresh_preview`] asks again.
    pub fn stop_preview(&mut self) {
        self.shared.stops.fetch_add(1, Ordering::Relaxed);
    }

    /// Whether the slides are recorded ahead of being played, while nothing else is going
    /// on. On unless turned off.
    pub fn record_ahead_of_time(&mut self, on: bool) {
        self.records_ahead = on;
    }

    pub fn records_ahead_of_time(&self) -> bool {
        self.records_ahead
    }

    // ── Playing slides ───────────────────────────────────────────────────────

    fn key_of(&self, slide: usize) -> Option<u64> {
        self.keys.get(slide).copied()
    }

    /// Slide `slide` as Manim plays it, if it has been recorded as it is now.
    pub fn clip_of(&self, slide: usize) -> Option<&Clip> {
        self.recordings.get(&self.key_of(slide)?)
    }

    /// The selected slide as Manim plays it.
    pub fn clip(&self) -> Option<&Clip> {
        self.clip_of(self.slide)
    }

    fn keep_recording(&mut self, key: u64, clip: Clip) {
        // Recordings of slides as they used to be are kept for a while, so that undoing
        // does not mean recording again, but not for ever.
        if self.recordings.len() > 3 * self.keys.len() + 8 {
            let keys = &self.keys;
            self.recordings.retain(|key, _| keys.contains(key));
        }
        self.recordings.insert(key, clip);
    }

    fn lacks_recording(&self, slide: usize) -> Option<u64> {
        let key = self.key_of(slide)?;
        let known = self.recordings.contains_key(&key) || self.recordings_wanted.contains(&key);
        (!known).then_some(key)
    }

    /// Has Manim record slide `slide`, unless that is done or under way, ahead of the
    /// slides nobody asked for. The result turns up in [`Session::clip_of`]; a failure in
    /// [`Session::recording_problem_of`].
    pub fn record(&mut self, slide: usize) {
        let Some(key) = self.lacks_recording(slide) else {
            return;
        };
        self.recording_problems.remove(&key);
        self.shared.asks.fetch_add(1, Ordering::Relaxed);
        let work = Work::Record {
            slide,
            key,
            asked: true,
        };
        if self.send(&[work]) {
            self.recordings_wanted.insert(key);
        } else {
            let problem = "There is no Manim to play the slide with.".to_owned();
            self.recording_problems.insert(key, problem);
        }
    }

    /// Has Manim record the selected slide.
    pub fn record_slide(&mut self) {
        self.record(self.slide);
    }

    /// Asks for every slide that has no recording yet: the selected one first, then the
    /// ones after it, then the ones before.
    fn record_ahead(&mut self) {
        let count = self.deck.slides.len();
        let order = (self.slide..count).chain((0..self.slide.min(count)).rev());
        let work: Vec<Work> = order
            .filter(|&slide| matches!(self.deck.slides[slide].content, SlideContent::Blueprint(_)))
            .filter_map(|slide| {
                let key = self.lacks_recording(slide)?;
                let failed = self.recording_problems.contains_key(&key);
                (!failed).then_some(Work::Record {
                    slide,
                    key,
                    asked: false,
                })
            })
            .collect();
        if !work.is_empty() && self.send(&work) {
            for work in work {
                if let Work::Record { key, .. } = work {
                    self.recordings_wanted.insert(key);
                }
            }
        }
    }

    /// Whether a recording of `slide` has been asked for and has not arrived yet.
    pub fn is_recording_slide(&self, slide: usize) -> bool {
        self.key_of(slide)
            .is_some_and(|key| self.recordings_wanted.contains(&key))
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording_slide(self.slide)
    }

    /// Why the last recording of `slide` asked for could not be made.
    pub fn recording_problem_of(&self, slide: usize) -> Option<&str> {
        let problem = self.recording_problems.get(&self.key_of(slide)?)?;
        Some(problem.as_str())
    }

    pub fn recording_problem(&self) -> Option<&str> {
        self.recording_problem_of(self.slide)
    }

    /// Uses `clip` as the recording of `slide` as it is now. For front ends that obtain
    /// recordings some other way, and for tests.
    pub fn install_recording_for(&mut self, slide: usize, clip: Clip) {
        if let Some(key) = self.key_of(slide) {
            self.keep_recording(key, clip);
        }
    }

    pub fn install_recording(&mut self, clip: Clip) {
        self.install_recording_for(self.slide, clip);
    }
}
