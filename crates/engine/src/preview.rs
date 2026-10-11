//! Keeping the preview in step with the deck, off the thread that draws.

use std::{
    collections::{BTreeSet, VecDeque},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use suit_check::Checked;
use suit_core::Deck;
use suit_render::{Clip, Prober, Sidecar, Snapshots};
use suit_sig::Signature;

use crate::Session;

/// How long the deck has to hold still before Manim is asked about it.
const DEBOUNCE: Duration = Duration::from_millis(120);

#[derive(Clone, Debug, PartialEq)]
pub enum Preview {
    /// The preview shows the deck as it is.
    Ready,
    /// A preview of the latest change is on its way; what is shown is older.
    Running,
    /// Manim stopped at some step; what it got through is still shown.
    Failed(String),
}

/// A slide as Manim plays it, and which state of the deck it is a recording of.
pub struct Recording {
    pub clip: Clip,
    revision: u64,
}

struct Request {
    revision: u64,
    deck: Deck,
    checked: Checked,
    /// `None` asks for the scene after every step; a slide asks for that slide's frames.
    record: Option<usize>,
}

enum Reply {
    Steps {
        revision: u64,
        result: Result<Snapshots, String>,
    },
    Clip {
        revision: u64,
        slide: usize,
        result: Result<Clip, String>,
    },
}

/// A thread that owns the Python process. It answers the newest request for the steps, and
/// every request for a recording, one after another, as long as the deck has not changed.
pub(crate) struct Worker {
    requests: Sender<Request>,
    replies: Receiver<Reply>,
}

impl Worker {
    pub fn start(sidecar: Sidecar, sig: Signature, wake: impl Fn() + Send + 'static) -> Worker {
        let (requests, inbox) = mpsc::channel::<Request>();
        let (outbox, replies) = mpsc::channel();
        thread::spawn(move || {
            let mut prober = Prober::new(sidecar);
            // Recordings still to be made.
            let mut backlog: VecDeque<Request> = VecDeque::new();
            loop {
                // With nothing to do, wait; otherwise only look at what has come in. The
                // wait ends for good when the session, and with it the sender, is dropped.
                let arrived: Vec<Request> = match backlog.is_empty() {
                    true => match inbox.recv() {
                        Ok(first) => std::iter::once(first).chain(inbox.try_iter()).collect(),
                        Err(_) => return,
                    },
                    false => inbox.try_iter().collect(),
                };
                let newest = arrived
                    .iter()
                    .chain(&backlog)
                    .map(|request| request.revision)
                    .max();
                let mut steps = None;
                for request in arrived {
                    match request.record {
                        None => steps = Some(request),
                        Some(_) => backlog.push_back(request),
                    }
                }
                // A recording of the deck as it used to be is of no use to anyone, and
                // neither is the same recording twice.
                backlog.retain(|request| Some(request.revision) == newest);
                let mut slides = BTreeSet::new();
                backlog.retain(|request| slides.insert(request.record));
                // The steps first: the canvas is waiting for them. Then one recording, and
                // back to see whether anything more urgent has arrived.
                let Some(request) = steps.or_else(|| backlog.pop_front()) else {
                    continue;
                };
                let Request {
                    revision,
                    deck,
                    checked,
                    record,
                } = request;
                let reply = match record {
                    None => Reply::Steps {
                        revision,
                        result: prober.probe(&deck, &sig, &checked),
                    },
                    Some(slide) => {
                        let result = prober.probe_frames(&deck, &sig, &checked, slide);
                        Reply::Clip {
                            revision,
                            slide,
                            result,
                        }
                    }
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
                Reply::Steps { revision, result } => self.take_steps(revision, result),
                Reply::Clip {
                    revision,
                    slide,
                    result,
                } => {
                    self.recordings_wanted.remove(&(revision, slide));
                    let whole = result.and_then(|clip| match clip.error.clone() {
                        Some(error) => Err(error),
                        None => Ok(clip),
                    });
                    match whole {
                        Ok(clip) => {
                            self.recordings.insert(slide, Recording { clip, revision });
                        }
                        Err(problem) => {
                            self.recording_problems.insert(slide, (revision, problem));
                        }
                    }
                }
            }
        }

        if self.requested == Some(self.revision) {
            return None;
        }
        let waited = self.changed_at.elapsed();
        if waited < DEBOUNCE {
            self.preview = Preview::Running;
            return Some(DEBOUNCE - waited);
        }
        if self.send(None) {
            self.requested = Some(self.revision);
            self.preview = Preview::Running;
        }
        None
    }

    fn take_steps(&mut self, revision: u64, result: Result<Snapshots, String>) {
        self.generation += 1;
        let latest = self.requested == Some(revision);
        match result {
            Ok(snapshots) => {
                self.shown = Some(revision);
                self.preview = match (&snapshots.error, latest) {
                    (_, false) => Preview::Running,
                    (Some(error), true) => Preview::Failed(error.clone()),
                    (None, true) => Preview::Ready,
                };
                self.snapshots = snapshots;
            }
            // Python itself did not run; what was shown before is better than nothing.
            Err(reason) => self.preview = Preview::Failed(reason),
        }
    }

    fn send(&self, record: Option<usize>) -> bool {
        let request = Request {
            revision: self.revision,
            deck: self.deck.clone(),
            checked: self.checked.clone(),
            record,
        };
        self.worker
            .as_ref()
            .is_some_and(|worker| worker.requests.send(request).is_ok())
    }

    /// Asks for a fresh preview even though the deck has not changed.
    pub fn refresh_preview(&mut self) {
        self.requested = None;
    }

    // ── Playing slides ───────────────────────────────────────────────────────

    /// Slide `slide` as Manim plays it, if it has been recorded since the deck last changed.
    pub fn clip_of(&self, slide: usize) -> Option<&Clip> {
        let recording = self.recordings.get(&slide)?;
        (recording.revision == self.revision).then_some(&recording.clip)
    }

    /// The selected slide as Manim plays it.
    pub fn clip(&self) -> Option<&Clip> {
        self.clip_of(self.slide)
    }

    /// Has Manim record slide `slide`, unless that is done or under way. Recordings are made
    /// one after another, in the order they were asked for. The result turns up in
    /// [`Session::clip_of`]; a failure in [`Session::recording_problem_of`].
    pub fn record(&mut self, slide: usize) {
        let wanted = (self.revision, slide);
        if self.clip_of(slide).is_some() || self.recordings_wanted.contains(&wanted) {
            return;
        }
        // Whatever was asked for or went wrong before the deck changed is history.
        let revision = self.revision;
        self.recordings_wanted
            .retain(|(asked, _)| *asked == revision);
        self.recordings
            .retain(|_, recording| recording.revision == revision);
        self.recording_problems.remove(&slide);
        if self.send(Some(slide)) {
            self.recordings_wanted.insert(wanted);
        } else {
            let problem = "There is no Manim to play the slide with.".to_owned();
            self.recording_problems.insert(slide, (revision, problem));
        }
    }

    /// Has Manim record the selected slide.
    pub fn record_slide(&mut self) {
        self.record(self.slide);
    }

    /// Whether a recording of `slide` has been asked for and has not arrived yet.
    pub fn is_recording_slide(&self, slide: usize) -> bool {
        self.recordings_wanted.contains(&(self.revision, slide))
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording_slide(self.slide)
    }

    /// Why the last recording of `slide` asked for could not be made.
    pub fn recording_problem_of(&self, slide: usize) -> Option<&str> {
        let (revision, problem) = self.recording_problems.get(&slide)?;
        (*revision == self.revision).then_some(problem.as_str())
    }

    pub fn recording_problem(&self) -> Option<&str> {
        self.recording_problem_of(self.slide)
    }

    /// Uses `clip` as the recording of `slide` as it is now. For front ends that obtain
    /// recordings some other way, and for tests.
    pub fn install_recording_for(&mut self, slide: usize, clip: Clip) {
        self.recordings.insert(
            slide,
            Recording {
                clip,
                revision: self.revision,
            },
        );
    }

    pub fn install_recording(&mut self, clip: Clip) {
        self.install_recording_for(self.slide, clip);
    }
}
