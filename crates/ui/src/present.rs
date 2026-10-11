//! Presenting: the slides full screen, one key press at a time.
//!
//! The keys are manim-slides' own (→ next, ← previous, V reverse, R replay, Space
//! play/pause, F full screen, H hide the mouse, Q quit), plus the ones a clicker sends.

use eframe::egui::{self, Align2, Color32, FontId, Key, Sense, vec2};
use suit_engine::Presenter;

use crate::{
    SuitApp,
    canvas::{Cached, Frame, Raster},
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
}

impl SuitApp {
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
        // Have Manim record what will be needed: this slide and the ones after it first,
        // then the ones before, nearest first.
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
        let Some(presenting) = &mut self.presenting else {
            return;
        };
        let whole = ui.max_rect();
        let response = ui.allocate_rect(whole, Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Presentation")
        });

        let now = ctx.input(|input| input.time);
        let pressed =
            |keys: &[Key]| ctx.input(|input| keys.iter().any(|&key| input.key_pressed(key)));
        let presenter = &mut presenting.presenter;
        if pressed(&[Key::ArrowRight, Key::PageDown, Key::Enter]) || response.clicked() {
            presenter.next(now);
        }
        if pressed(&[Key::ArrowLeft, Key::PageUp]) {
            presenter.previous();
        }
        if pressed(&[Key::V]) {
            presenter.reverse();
        }
        if pressed(&[Key::R]) {
            presenter.replay();
            // A slide Manim could not play gets another try.
            let slide = presenter.slide();
            if self.session.recording_problem_of(slide).is_some() {
                self.session.record(slide);
            }
        }
        if pressed(&[Key::Space]) {
            presenter.play_pause(now);
        }
        if pressed(&[Key::H]) {
            presenting.mouse_hidden = !presenting.mouse_hidden;
        }
        if pressed(&[Key::F]) {
            presenting.full_screen = !presenting.full_screen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(presenting.full_screen));
        }
        let leave = pressed(&[Key::Q, Key::Escape]);
        let session = &self.session;

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
        if presenter.is_running() {
            ctx.request_repaint();
        }

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
        let state = match (presenter.is_paused(), presenter.is_reversed()) {
            (true, _) => "paused",
            (false, true) => "rewinding",
            (false, false) => "",
        };
        corner(
            whole.left_bottom() + vec2(14.0, -10.0),
            Align2::LEFT_BOTTOM,
            state.to_owned(),
        );

        if presenting.mouse_hidden && response.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::None);
        }
        if leave {
            self.stop_presenting(&ctx);
        }
    }
}
