//! Presenting: the slides full screen, one key press at a time.
//!
//! The keys are manim-slides' own (→ next, ← previous, V reverse, R replay, Space
//! play/pause, F full screen, H hide the mouse, Q quit), plus the ones a clicker sends and
//! N for the notes: a second window with what to say, what comes next and how long it has
//! been, to keep on the screen the audience does not see.

use eframe::egui::{self, Align2, Color32, FontId, Key, RichText, Sense, vec2};
use suit_engine::{PresentOptions, Presenter, Session};

use crate::{
    SuitApp,
    canvas::{Cached, Frame, Raster, rasterize},
    style::{UV, WARNING},
};

/// Frames are painted no taller than this and stretched to the screen from there: on a very
/// large screen that keeps the motion smooth at a small cost in sharpness.
const MOST_ROWS: f32 = 1440.0;

pub(crate) struct Presenting {
    presenter: Presenter,
    picture: Cached<(usize, usize, [u32; 2])>,
    raster: Raster,
    /// What is on screen: the slide, and the frame of its recording once there is one.
    showing: (usize, Option<usize>),
    mouse_hidden: bool,
    full_screen: bool,
    /// When the first slide came on, for the clock in the notes.
    began: Option<f64>,
    /// The picture of what comes next, for the notes.
    coming: Cached<(u64, usize, [u32; 2])>,
}

/// What a key or a click asks of the presentation.
#[derive(Clone, Copy, PartialEq)]
enum Ask {
    Next,
    Previous,
    Reverse,
    Replay,
    PlayPause,
    Mouse,
    FullScreen,
    Notes,
    Leave,
}

/// The keys pressed in the window that `ui` is in.
fn asked(ui: &egui::Ui) -> Vec<Ask> {
    let keys: [(&[Key], Ask); 9] = [
        (&[Key::ArrowRight, Key::PageDown, Key::Enter], Ask::Next),
        (&[Key::ArrowLeft, Key::PageUp], Ask::Previous),
        (&[Key::V], Ask::Reverse),
        (&[Key::R], Ask::Replay),
        (&[Key::Space], Ask::PlayPause),
        (&[Key::H], Ask::Mouse),
        (&[Key::F], Ask::FullScreen),
        (&[Key::N], Ask::Notes),
        (&[Key::Q, Key::Escape], Ask::Leave),
    ];
    ui.input(|input| {
        let pressed = |keys: &[Key]| keys.iter().any(|&key| input.key_pressed(key));
        keys.iter()
            .filter(|(keys, _)| pressed(keys))
            .map(|(_, ask)| *ask)
            .collect()
    })
}

impl SuitApp {
    /// How presentations are run from now on, and whether the notes are shown with them.
    pub fn with_presentation(mut self, options: PresentOptions, notes: bool) -> SuitApp {
        self.present_options = options;
        self.notes_shown = notes;
        self
    }

    /// Starts presenting as soon as the window is up, and closes it when the presentation
    /// is over: what `--present` on the command line means.
    pub fn presenting_at_once(mut self) -> SuitApp {
        self.present_at_start = true;
        self
    }

    pub fn is_presenting(&self) -> bool {
        self.presenting.is_some()
    }

    /// While presenting: the slide on screen, as an index into the deck, and the frame of
    /// its recording being shown, if it has been recorded.
    pub fn presenting(&self) -> Option<(usize, Option<usize>)> {
        self.presenting
            .as_ref()
            .map(|presenting| presenting.showing)
    }

    /// Shows the deck full screen, starting with the selected slide.
    pub fn start_presenting(&mut self, ctx: &egui::Context) {
        let Some(presenter) = Presenter::new(&self.session.deck, self.session.slide) else {
            self.notice = Some(
                "Every slide is set to be skipped, so there is nothing to present.".to_owned(),
            );
            return;
        };
        let presenter = presenter.with(self.present_options);
        // Have Manim record what is not recorded yet: this slide and the ones after it
        // first, then the ones before, nearest first.
        let (place, _) = presenter.place();
        let (before, onward) = presenter.slides().split_at(place - 1);
        for &slide in onward.iter().chain(before.iter().rev()) {
            self.session.record(slide);
        }
        self.stop_playing();
        let showing = (presenter.slide(), None);
        self.presenting = Some(Presenting {
            presenter,
            picture: Cached::default(),
            raster: Raster::default(),
            showing,
            mouse_hidden: true,
            full_screen: true,
            began: None,
            coming: Cached::default(),
        });
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
    }

    /// Back to editing, on the slide that was showing.
    pub fn stop_presenting(&mut self, ctx: &egui::Context) {
        if let Some(presenting) = self.presenting.take() {
            self.session.select(presenting.presenter.slide(), None);
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
    }

    pub(crate) fn present(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let Self {
            presenting: Some(presenting),
            session,
            notes_shown,
            ..
        } = self
        else {
            return;
        };
        let whole = ui.max_rect();
        let response = ui.allocate_rect(whole, Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Presentation")
        });
        let now = ctx.input(|input| input.time);

        // What was asked for, in this window or in the one with the notes.
        let mut asks = asked(ui);
        if response.clicked() {
            asks.push(Ask::Next);
        }
        if *notes_shown {
            asks.extend(notes(&ctx, session, presenting, now));
        }
        let mut leave = false;
        for ask in asks {
            let presenter = &mut presenting.presenter;
            let slide = presenter.slide();
            match ask {
                Ask::Next => presenter.next(now, session.deck.slides[slide].config.looping),
                Ask::Previous => presenter.previous(),
                Ask::Reverse => presenter.reverse(),
                Ask::Replay => {
                    presenter.replay();
                    // A slide Manim could not play gets another try.
                    if session.recording_problem_of(slide).is_some() {
                        session.record(slide);
                    }
                }
                Ask::PlayPause => presenter.play_pause(now),
                Ask::Mouse => presenting.mouse_hidden = !presenting.mouse_hidden,
                Ask::FullScreen => {
                    presenting.full_screen = !presenting.full_screen;
                    let command = egui::ViewportCommand::Fullscreen(presenting.full_screen);
                    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, command);
                }
                Ask::Notes => *notes_shown = !*notes_shown,
                Ask::Leave => leave = true,
            }
        }
        let session = &*session;
        let presenter = &mut presenting.presenter;

        // Where the presentation is now, and what that looks like.
        let slide = presenter.slide();
        let recording = session.clip_of(slide);
        let frames = recording.map(|clip| clip.frames.len());
        let config = &session.deck.slides[slide].config;
        let shown = presenter.advance(now, frames, config, session.deck.config.fps);
        presenting.showing = match shown {
            Some((slide, frame)) => (slide, Some(frame)),
            None => (presenter.slide(), None),
        };
        if shown.is_some() {
            presenting.began.get_or_insert(now);
        }
        if presenter.is_running() {
            ctx.request_repaint();
        }
        leave |= presenter.is_over();

        let painter = ui.painter_at(whole);
        painter.rect_filled(whole, 0.0, Color32::BLACK);
        let frame = Frame::of(&session.deck.config);
        let rect = frame.fit(whole);
        let faint = Color32::from_white_alpha(90);
        let picture =
            shown.and_then(|(slide, index)| Some((slide, index, session.clip_of(slide)?)));
        match picture {
            Some((slide, index, clip)) => {
                let rows = (rect.height() * ctx.pixels_per_point()).min(MOST_ROWS);
                let size = [rows * frame.aspect(), rows].map(|side| side.round() as u32);
                let background = session.deck.config.background;
                let Presenting {
                    picture, raster, ..
                } = presenting;
                let texture = picture.get(&ctx, "presentation", (slide, index, size), || {
                    raster.draw(clip.frame(index), frame, background, size, None)
                });
                painter.image(texture, rect, UV, Color32::WHITE);
            }
            None => {
                let number = slide + 1;
                let (message, color) = match (session.recording_problem_of(slide), recording) {
                    (Some(problem), _) => (
                        format!("Slide {number} could not be played: {problem}\nR tries again."),
                        WARNING,
                    ),
                    (None, Some(_)) => (format!("Slide {number} has nothing to play."), faint),
                    (None, None) => (format!("Manim is playing slide {number} through…"), faint),
                };
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    message,
                    FontId::proportional(18.0),
                    color,
                );
            }
        }

        // Quiet reminders of where things stand, in the corners.
        let presenter = &presenting.presenter;
        let (place, count) = presenter.place();
        let corner = |at: egui::Pos2, anchor: Align2, text: String| {
            painter.text(at, anchor, text, FontId::proportional(13.0), faint);
        };
        corner(
            whole.right_bottom() - vec2(14.0, 10.0),
            Align2::RIGHT_BOTTOM,
            format!("{place} / {count}"),
        );
        corner(
            whole.left_bottom() + vec2(14.0, -10.0),
            Align2::LEFT_BOTTOM,
            state(presenter).to_owned(),
        );

        if presenting.mouse_hidden && response.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::None);
        }
        if leave {
            let over = presenter.is_over();
            self.stop_presenting(&ctx);
            // Started from the command line to present, and that is done.
            if over && self.present_at_start {
                self.leaving = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

/// What the presentation is doing, when that is not simply playing.
fn state(presenter: &Presenter) -> &'static str {
    match (presenter.is_paused(), presenter.is_reversed()) {
        (true, _) => "paused",
        (false, true) => "rewinding",
        (false, false) => "",
    }
}

/// Minutes and seconds.
fn clock(secs: f64) -> String {
    let secs = secs.max(0.0) as u64;
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// The window with the notes. Returns what was asked for in it.
fn notes(
    ctx: &egui::Context,
    session: &Session,
    presenting: &mut Presenting,
    now: f64,
) -> Vec<Ask> {
    let id = egui::ViewportId::from_hash_of("notes");
    let builder = egui::ViewportBuilder::default()
        .with_title("Suit — notes")
        .with_inner_size([560.0, 660.0]);
    let presenter = &presenting.presenter;
    let slide = &session.deck.slides[presenter.slide()];
    let (place, count) = presenter.place();
    let elapsed = presenting.began.map_or(0.0, |began| now - began);
    let coming = &mut presenting.coming;
    let mut asks = Vec::new();
    ctx.show_viewport_immediate(id, builder, |ui, class| {
        // In a window of its own it has keys of its own; embedded in the presentation it
        // shares them, and they have been read already.
        if class != egui::ViewportClass::EmbeddedWindow {
            asks = asked(ui);
        }
        if ui.input(|input| input.viewport().close_requested()) {
            asks.push(Ask::Notes);
        }
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(clock(elapsed)).size(30.0).strong());
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.label(format!("Slide {place} of {count}"));
                    ui.label(RichText::new(&slide.title).strong());
                });
                ui.add_space(12.0);
                ui.colored_label(WARNING, state(presenter));
            });
            ui.separator();

            let next = presenter.following();
            let frame = Frame::of(&session.deck.config);
            let height = 150.0;
            egui::Panel::bottom("coming")
                .show_separator_line(true)
                .show(ui, |ui| {
                    ui.add_space(6.0);
                    let Some(next) = next else {
                        ui.weak("This is the last slide.");
                        return;
                    };
                    let title = &session.deck.slides[next].title;
                    ui.weak(format!("Next: {title}"));
                    let size = vec2(height * frame.aspect(), height);
                    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                    let pixels = ui.ctx().pixels_per_point();
                    let wanted = [size.x * pixels, size.y * pixels].map(|side| side.round() as u32);
                    let key = (session.generation, next, wanted);
                    let texture = coming.get(ui.ctx(), "coming", key, || {
                        // What the next slide rests on once it has played.
                        let rest = session.snapshots.last_of(next);
                        let drawn = rest.into_iter().flat_map(|snapshot| &snapshot.drawn);
                        rasterize(drawn, frame, session.deck.config.background, wanted, None)
                    });
                    ui.painter().image(texture, rect, UV, Color32::WHITE);
                    ui.add_space(6.0);
                    ui.weak("→ next   ← previous   V rewind   R replay   Space hold   N notes");
                });

            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| match slide.config.notes.trim() {
                    "" => {
                        ui.weak("No notes for this slide.");
                    }
                    notes => {
                        ui.label(RichText::new(notes).size(20.0));
                    }
                });
        });
    });
    asks
}
