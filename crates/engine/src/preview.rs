//! Keeping the preview in step with the deck, off the thread that draws.

use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use suit_check::Checked;
use suit_core::Deck;
use suit_render::{Prober, Sidecar, Snapshots};
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

struct Request {
    revision: u64,
    deck: Deck,
    checked: Checked,
}

struct Reply {
    revision: u64,
    result: Result<Snapshots, String>,
}

/// A thread that owns the Python process and probes whatever deck it was sent last.
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
            while let Ok(mut request) = inbox.recv() {
                // Only the newest deck is worth probing.
                while let Ok(newer) = inbox.try_recv() {
                    request = newer;
                }
                let result = prober.probe(&request.deck, &sig, &request.checked);
                if outbox
                    .send(Reply {
                        revision: request.revision,
                        result,
                    })
                    .is_err()
                {
                    break;
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
            self.generation += 1;
            let latest = self.requested == Some(reply.revision);
            match reply.result {
                Ok(snapshots) => {
                    self.shown = Some(reply.revision);
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

        if self.requested == Some(self.revision) {
            return None;
        }
        let waited = self.changed_at.elapsed();
        if waited < DEBOUNCE {
            self.preview = Preview::Running;
            return Some(DEBOUNCE - waited);
        }
        let request = Request {
            revision: self.revision,
            deck: self.deck.clone(),
            checked: self.checked.clone(),
        };
        let worker = self.worker.as_ref()?;
        if worker.requests.send(request).is_ok() {
            self.requested = Some(self.revision);
            self.preview = Preview::Running;
        }
        None
    }

    /// Asks for a fresh preview even though the deck has not changed.
    pub fn refresh_preview(&mut self) {
        self.requested = None;
    }
}
