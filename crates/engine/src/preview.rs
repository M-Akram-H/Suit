//! Keeping the preview in step with the deck, off the thread that draws.

use std::{
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
    pub slide: usize,
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

/// A thread that owns the Python process and answers the newest request of each kind.
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
            // Ends when the session, and with it the sending side, is dropped.
            while let Ok(first) = inbox.recv() {
                // Of everything that has piled up, only the newest of each kind is worth
                // answering; the steps first, because the canvas is waiting for them.
                let (mut steps, mut clip) = (None, None);
                for request in std::iter::once(first).chain(inbox.try_iter()) {
                    match request.record {
                        None => steps = Some(request),
                        Some(_) => clip = Some(request),
                    }
                }
                for request in [steps, clip].into_iter().flatten() {
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
                    if self.recording_wanted == Some((revision, slide)) {
                        self.recording_wanted = None;
                    }
                    match result.and_then(|clip| match clip.error.clone() {
                        Some(error) => Err(error),
                        None => Ok(clip),
                    }) {
                        Ok(clip) => {
                            self.recording = Some(Recording {
                                slide,
                                clip,
                                revision,
                            })
                        }
                        Err(problem) => self.recording_problem = Some(problem),
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

    // ── Playing a slide ──────────────────────────────────────────────────────

    /// The selected slide as Manim plays it, if it has been recorded since the deck last
    /// changed.
    pub fn clip(&self) -> Option<&Clip> {
        let recording = self.recording.as_ref()?;
        let current = recording.revision == self.revision && recording.slide == self.slide;
        current.then_some(&recording.clip)
    }

    /// Has Manim record the selected slide, unless that is done or under way. The result
    /// turns up in [`Session::clip`]; a failure in [`Session::recording_problem`].
    pub fn record_slide(&mut self) {
        let wanted = (self.revision, self.slide);
        if self.clip().is_some() || self.recording_wanted == Some(wanted) {
            return;
        }
        self.recording_problem = None;
        if self.send(Some(self.slide)) {
            self.recording_wanted = Some(wanted);
        } else {
            self.recording_problem = Some("There is no Manim to play the slide with.".to_owned());
        }
    }

    /// Whether a recording of the selected slide has been asked for and not yet arrived.
    pub fn is_recording(&self) -> bool {
        self.recording_wanted == Some((self.revision, self.slide))
    }

    /// Why the last recording asked for could not be made.
    pub fn recording_problem(&self) -> Option<&str> {
        self.recording_problem.as_deref()
    }

    /// Uses `clip` as the recording of the selected slide as it is now. For front ends that
    /// obtain recordings some other way, and for tests.
    pub fn install_recording(&mut self, clip: Clip) {
        self.recording = Some(Recording {
            slide: self.slide,
            clip,
            revision: self.revision,
        });
    }
}
