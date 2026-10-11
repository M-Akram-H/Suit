//! Presenting: which slide is showing, which way it is running, and how far along it is.
//!
//! The transitions are those of manim-slides' own presenter (`present/player.py`, 5.7.0),
//! so that rehearsing here and presenting the exported deck there behave the same; spec §6.6
//! has them as a table. This is only the bookkeeping: it is told the time and how long the
//! current slide's recording is, and answers with the frame to show.

use suit_core::{Deck, SlideConfig};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Run {
    /// Loaded, and to run as soon as its recording is there.
    Waiting,
    /// Running: `from` frames in at time `since`.
    Playing { since: f64, from: f64 },
    /// Stopped by hand, `at` frames in.
    Paused { at: f64 },
    /// Played through; resting on its last frame.
    Ended,
}

/// Choices about how a presentation runs. manim-slides takes the same ones on its command
/// line; all are off unless asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PresentOptions {
    /// The first slide waits for a key before it runs (`--start-paused`).
    pub start_paused: bool,
    /// NEXT on a looping slide lets the loop finish its round and rest, instead of cutting
    /// it off; a second NEXT moves on (`--next-terminates-loop`).
    pub next_ends_loops: bool,
    /// Going past the last slide ends the presentation (`--exit-after-last-slide`).
    pub leave_after_last: bool,
    /// Every slide moves on by itself and none loops: a run-through (`--skip-all`).
    pub run_through: bool,
}

#[derive(Clone, Debug)]
pub struct Presenter {
    /// The slides that are shown, as indices into the deck: all but the skipped ones.
    order: Vec<usize>,
    /// Which of `order` is showing.
    at: usize,
    /// Whether the slide is running backwards.
    reversed: bool,
    run: Run,
    /// How many frames into its run the slide was when last looked at.
    position: f64,
    options: PresentOptions,
    /// NEXT was pressed on a loop that is to finish its round first.
    ending_loop: bool,
    /// The presentation went past its last slide and was to end there.
    over: bool,
}

impl Presenter {
    /// Starts at slide `start` of the deck, or at the next one that is shown if that one is
    /// skipped. `None` if the deck shows no slide at all.
    pub fn new(deck: &Deck, start: usize) -> Option<Presenter> {
        let shown = |&index: &usize| !deck.slides[index].config.skip;
        let order: Vec<usize> = (0..deck.slides.len()).filter(shown).collect();
        let at = order
            .iter()
            .position(|&slide| slide >= start)
            .or(order.len().checked_sub(1))?;
        Some(Presenter {
            order,
            at,
            reversed: false,
            run: Run::Waiting,
            position: 0.0,
            options: PresentOptions::default(),
            ending_loop: false,
            over: false,
        })
    }

    /// The same presentation, run with `options`. To be called before it starts.
    pub fn with(mut self, options: PresentOptions) -> Presenter {
        self.options = options;
        if options.start_paused {
            self.run = Run::Paused { at: 0.0 };
        }
        self
    }

    /// The slide showing, as an index into the deck.
    pub fn slide(&self) -> usize {
        self.order[self.at]
    }

    /// The slides shown, in order, as indices into the deck.
    pub fn slides(&self) -> &[usize] {
        &self.order
    }

    /// The slide that NEXT leads to, if any, as an index into the deck.
    pub fn following(&self) -> Option<usize> {
        match self.reversed {
            true => Some(self.slide()),
            false => self.order.get(self.at + 1).copied(),
        }
    }

    /// Which slide of how many is showing, counted from one.
    pub fn place(&self) -> (usize, usize) {
        (self.at + 1, self.order.len())
    }

    pub fn is_reversed(&self) -> bool {
        self.reversed
    }

    pub fn is_paused(&self) -> bool {
        matches!(self.run, Run::Paused { .. })
    }

    /// Whether anything is moving, so that another frame will be wanted soon.
    pub fn is_running(&self) -> bool {
        matches!(self.run, Run::Playing { .. })
    }

    /// Whether the presentation has gone past its last slide and was to end there.
    pub fn is_over(&self) -> bool {
        self.over
    }

    /// Brings the presentation to time `now`, in seconds on any steady clock. `frames` is
    /// the length of the showing slide's recording if it has been made, `config` that
    /// slide's settings, `fps` the deck's frame rate. Returns the slide and the frame of
    /// its recording to show; `None` while there is nothing to show yet.
    pub fn advance(
        &mut self,
        now: f64,
        frames: Option<usize>,
        config: &SlideConfig,
        fps: u32,
    ) -> Option<(usize, usize)> {
        let frames = frames.filter(|&frames| frames > 0)?;
        let last = (frames - 1) as f64;
        let rate = match self.reversed {
            true => config.reversed_playback_rate,
            false => config.playback_rate,
        };
        if self.run == Run::Waiting {
            self.run = Run::Playing {
                since: now,
                from: 0.0,
            };
        }
        // In a run-through nothing loops and everything moves on.
        let through = self.options.run_through;
        let (looping, auto_next) = (config.looping && !through, config.auto_next || through);
        // Moving on by itself changes these; what is returned is still about this slide.
        let (showing, reversed) = (self.slide(), self.reversed);
        self.position = match self.run {
            Run::Playing { since, from } => {
                let due = from + (now - since) * rate * f64::from(fps);
                match (due < frames as f64, looping, auto_next) {
                    (true, ..) => due,
                    // A looping slide never ends, so looping wins over moving on, unless
                    // it was told that this round is its last.
                    (false, true, _) if !self.ending_loop => {
                        // Counting from the start of the new round, so that the end of
                        // this one can be told from the ends of those before it.
                        let from = due % frames as f64;
                        self.run = Run::Playing { since: now, from };
                        from
                    }
                    (false, _, true) if self.load_next() => last,
                    // Nothing follows, or nothing is to follow by itself: it rests here.
                    (false, ..) => {
                        self.run = Run::Ended;
                        last
                    }
                }
            }
            Run::Paused { at } => at,
            Run::Ended => last,
            Run::Waiting => unreachable!("set running above"),
        };
        let index = (self.position.floor().max(0.0) as usize).min(frames - 1);
        Some((showing, if reversed { frames - 1 - index } else { index }))
    }

    /// NEXT: carry on if paused; otherwise load the next slide, even if this one has not
    /// finished. After a reversed slide, "next" is that same slide, forwards.
    ///
    /// `looping` says whether the slide showing loops: with
    /// [`PresentOptions::next_ends_loops`] such a slide finishes its round first.
    pub fn next(&mut self, now: f64, looping: bool) {
        let loops = looping && !self.options.run_through;
        match self.run {
            Run::Paused { at } => {
                self.run = Run::Playing {
                    since: now,
                    from: at,
                }
            }
            Run::Playing { .. } if self.options.next_ends_loops && loops => {
                self.ending_loop = true;
            }
            _ => {
                self.load_next();
            }
        }
    }

    /// Whether there was anything to load.
    fn load_next(&mut self) -> bool {
        if self.reversed {
            self.reversed = false;
        } else if self.at + 1 < self.order.len() {
            self.at += 1;
        } else {
            self.over = self.options.leave_after_last;
            return false;
        }
        self.load();
        true
    }

    /// Sets the slide now selected going from its start.
    fn load(&mut self) {
        self.run = Run::Waiting;
        self.ending_loop = false;
    }

    /// PREVIOUS: the slide before this one, forwards from its start.
    pub fn previous(&mut self) {
        if self.at > 0 {
            self.at -= 1;
            self.reversed = false;
            self.load();
        }
    }

    /// REVERSE: the slide before this one, backwards: from the picture this slide starts
    /// with back to the one that slide starts with.
    pub fn reverse(&mut self) {
        if self.at > 0 {
            self.at -= 1;
            self.reversed = true;
            self.load();
        }
    }

    /// REPLAY: the same slide again from where it began, in the direction it was going.
    pub fn replay(&mut self) {
        self.load();
    }

    /// PLAY/PAUSE: stops a running slide where it is, or sets a paused one going again. A
    /// slide that has ended stays ended.
    pub fn play_pause(&mut self, now: f64) {
        self.run = match self.run {
            Run::Playing { .. } => Run::Paused { at: self.position },
            Run::Paused { at } => Run::Playing {
                since: now,
                from: at,
            },
            other => other,
        };
    }
}

#[cfg(test)]
mod tests {
    use suit_core::DeckConfig;

    use super::*;

    const FPS: u32 = 60;

    /// A deck of `count` empty slides; each test says how long their recordings are.
    fn deck(count: usize) -> Deck {
        let mut deck = Deck::new(DeckConfig::default());
        for index in 0..count {
            deck.add_slide(&format!("Slide {}", index + 1));
        }
        deck
    }

    /// Every slide's recording is `frames` long.
    fn at(
        presenter: &mut Presenter,
        deck: &Deck,
        now: f64,
        frames: usize,
    ) -> Option<(usize, usize)> {
        let config = deck.slides[presenter.slide()].config.clone();
        presenter.advance(now, Some(frames), &config, FPS)
    }

    #[test]
    fn a_slide_plays_and_rests_on_its_last_frame() {
        let deck = deck(2);
        let mut presenter = Presenter::new(&deck, 0).unwrap();
        assert_eq!(
            presenter.advance(0.0, None, &deck.slides[0].config, FPS),
            None,
            "not recorded yet"
        );
        // The clock starts when there is something to show, not when the slide was loaded.
        assert_eq!(at(&mut presenter, &deck, 5.0, 60), Some((0, 0)));
        assert_eq!(at(&mut presenter, &deck, 5.5, 60), Some((0, 30)));
        assert!(presenter.is_running());
        assert_eq!(at(&mut presenter, &deck, 6.5, 60), Some((0, 59)));
        assert!(!presenter.is_running());
        assert_eq!(
            at(&mut presenter, &deck, 99.0, 60),
            Some((0, 59)),
            "it waits for a key"
        );
        assert_eq!(presenter.place(), (1, 2));
    }

    #[test]
    fn next_and_previous_load_slides_from_their_start() {
        let deck = deck(3);
        let mut presenter = Presenter::new(&deck, 0).unwrap();
        at(&mut presenter, &deck, 0.0, 60);
        // NEXT does not wait for the slide to finish.
        at(&mut presenter, &deck, 0.25, 60);
        presenter.next(0.25, false);
        assert_eq!(at(&mut presenter, &deck, 0.25, 60), Some((1, 0)));
        assert_eq!(at(&mut presenter, &deck, 0.75, 60), Some((1, 30)));
        presenter.next(0.75, false);
        presenter.next(0.75, false);
        presenter.next(0.75, false);
        assert_eq!(
            presenter.slide(),
            2,
            "there is nothing after the last slide"
        );

        presenter.previous();
        assert_eq!(at(&mut presenter, &deck, 2.0, 60), Some((1, 0)));
        presenter.previous();
        presenter.previous();
        assert_eq!(at(&mut presenter, &deck, 3.0, 60), Some((0, 0)));
        assert_eq!(
            at(&mut presenter, &deck, 3.5, 60),
            Some((0, 30)),
            "and it is not reloaded at the first"
        );
    }

    #[test]
    fn reverse_rewinds_the_slide_before_and_next_plays_it_again() {
        let deck = deck(3);
        let mut presenter = Presenter::new(&deck, 2).unwrap();
        presenter.reverse();
        assert!(presenter.is_reversed());
        // Slide 2, backwards: its last frame first.
        assert_eq!(at(&mut presenter, &deck, 0.0, 60), Some((1, 59)));
        assert_eq!(at(&mut presenter, &deck, 0.5, 60), Some((1, 29)));
        assert_eq!(
            at(&mut presenter, &deck, 2.0, 60),
            Some((1, 0)),
            "it rests where the slide begins"
        );

        // NEXT after a reversed slide is that slide forwards, not the one after it.
        presenter.next(2.0, false);
        assert!(!presenter.is_reversed());
        assert_eq!(at(&mut presenter, &deck, 2.0, 60), Some((1, 0)));
        assert_eq!(at(&mut presenter, &deck, 2.5, 60), Some((1, 30)));

        // At the first slide there is nothing to rewind.
        let mut first = Presenter::new(&deck, 0).unwrap();
        first.reverse();
        assert!(!first.is_reversed());
    }

    #[test]
    fn pausing_keeps_the_place_and_next_carries_on() {
        let deck = deck(2);
        let mut presenter = Presenter::new(&deck, 0).unwrap();
        at(&mut presenter, &deck, 0.0, 120);
        at(&mut presenter, &deck, 0.5, 120);
        presenter.play_pause(0.5);
        assert!(presenter.is_paused());
        assert_eq!(at(&mut presenter, &deck, 9.0, 120), Some((0, 30)));
        // While paused, NEXT means "carry on", as in manim-slides.
        presenter.next(9.0, false);
        assert_eq!(at(&mut presenter, &deck, 9.5, 120), Some((0, 60)));
        presenter.play_pause(9.5);
        presenter.play_pause(10.0);
        assert_eq!(at(&mut presenter, &deck, 10.5, 120), Some((0, 90)));

        presenter.replay();
        assert_eq!(at(&mut presenter, &deck, 11.0, 120), Some((0, 0)));
        // A slide that has ended is not set going by PLAY/PAUSE.
        at(&mut presenter, &deck, 20.0, 120);
        presenter.play_pause(20.0);
        assert_eq!(at(&mut presenter, &deck, 21.0, 120), Some((0, 119)));
    }

    #[test]
    fn loops_and_moving_on() {
        let mut deck = deck(3);
        deck.slides[0].config.looping = true;
        deck.slides[0].config.auto_next = true;
        deck.slides[1].config.auto_next = true;
        deck.slides[1].config.playback_rate = 2.0;
        let mut presenter = Presenter::new(&deck, 0).unwrap();
        at(&mut presenter, &deck, 0.0, 60);
        // A looping slide starts over, and so never moves on by itself.
        assert_eq!(at(&mut presenter, &deck, 1.25, 60), Some((0, 15)));
        assert_eq!(at(&mut presenter, &deck, 7.5, 60), Some((0, 30)));

        presenter.next(7.5, false);
        assert_eq!(at(&mut presenter, &deck, 7.5, 60), Some((1, 0)));
        // At twice the speed, a quarter of a second is thirty frames.
        assert_eq!(at(&mut presenter, &deck, 7.75, 60), Some((1, 30)));
        // When it ends it shows its last frame once more and loads the next slide.
        assert_eq!(at(&mut presenter, &deck, 8.0, 60), Some((1, 59)));
        assert_eq!(presenter.slide(), 2);
        assert_eq!(at(&mut presenter, &deck, 8.0, 60), Some((2, 0)));

        // Rewound, a slide that moves on by itself goes straight into playing forwards.
        presenter.reverse();
        at(&mut presenter, &deck, 9.0, 60);
        assert_eq!(
            at(&mut presenter, &deck, 10.0, 60),
            Some((1, 0)),
            "its first frame, the end of the rewind"
        );
        assert!(!presenter.is_reversed());
        assert_eq!(at(&mut presenter, &deck, 10.0, 60), Some((1, 0)));
        assert_eq!(at(&mut presenter, &deck, 10.25, 60), Some((1, 30)));

        // The last slide has nothing to move on to, so it comes to rest like any other.
        deck.slides[2].config.auto_next = true;
        at(&mut presenter, &deck, 10.5, 60);
        assert_eq!(at(&mut presenter, &deck, 10.5, 60), Some((2, 0)));
        assert_eq!(at(&mut presenter, &deck, 12.0, 60), Some((2, 59)));
        assert!(!presenter.is_running());
    }

    #[test]
    fn skipped_slides_are_not_shown() {
        let mut deck = deck(4);
        deck.slides[1].config.skip = true;
        deck.slides[3].config.skip = true;
        let mut presenter = Presenter::new(&deck, 1).unwrap();
        assert_eq!(presenter.slides(), [0, 2]);
        assert_eq!(
            presenter.slide(),
            2,
            "starting on a skipped slide starts on the next shown one"
        );
        assert_eq!(presenter.place(), (2, 2));
        presenter.previous();
        assert_eq!(presenter.slide(), 0);
        assert_eq!(
            Presenter::new(&deck, 3).unwrap().slide(),
            2,
            "or on the last, if none follows"
        );

        deck.slides[0].config.skip = true;
        deck.slides[2].config.skip = true;
        assert!(Presenter::new(&deck, 0).is_none());
    }

    #[test]
    fn the_ways_a_presentation_can_be_run() {
        let mut deck = deck(2);
        deck.slides[0].config.looping = true;

        // Held at the start until a key.
        let held = PresentOptions {
            start_paused: true,
            ..PresentOptions::default()
        };
        let mut presenter = Presenter::new(&deck, 0).unwrap().with(held);
        assert_eq!(at(&mut presenter, &deck, 0.0, 60), Some((0, 0)));
        assert_eq!(at(&mut presenter, &deck, 9.0, 60), Some((0, 0)));
        assert!(presenter.is_paused());
        presenter.next(9.0, true);
        assert_eq!(
            at(&mut presenter, &deck, 9.5, 60),
            Some((0, 30)),
            "next lets it go"
        );

        // A loop that finishes its round: NEXT in the middle of the second round lets it
        // run to its end and rest there; the NEXT after that moves on.
        let gentle = PresentOptions {
            next_ends_loops: true,
            ..PresentOptions::default()
        };
        let mut presenter = Presenter::new(&deck, 0).unwrap().with(gentle);
        at(&mut presenter, &deck, 0.0, 60);
        assert_eq!(at(&mut presenter, &deck, 1.5, 60), Some((0, 30)));
        presenter.next(1.5, true);
        assert_eq!(presenter.slide(), 0);
        assert_eq!(at(&mut presenter, &deck, 1.75, 60), Some((0, 45)));
        assert_eq!(at(&mut presenter, &deck, 2.5, 60), Some((0, 59)));
        assert!(!presenter.is_running());
        presenter.next(2.5, true);
        assert_eq!(at(&mut presenter, &deck, 2.5, 60), Some((1, 0)));

        // Leaving after the last slide, and running through without a key.
        let through = PresentOptions {
            run_through: true,
            leave_after_last: true,
            ..PresentOptions::default()
        };
        let mut presenter = Presenter::new(&deck, 0).unwrap().with(through);
        at(&mut presenter, &deck, 0.0, 60);
        assert_eq!(
            at(&mut presenter, &deck, 1.0, 60),
            Some((0, 59)),
            "the loop does not loop"
        );
        assert_eq!(at(&mut presenter, &deck, 1.0, 60), Some((1, 0)));
        assert!(!presenter.is_over());
        at(&mut presenter, &deck, 2.0, 60);
        assert!(presenter.is_over());

        let mut presenter = Presenter::new(&deck, 1).unwrap();
        presenter.next(0.0, false);
        assert!(
            !presenter.is_over(),
            "unless asked to, it stays on the last slide"
        );
    }
}
