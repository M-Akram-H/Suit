//! The editor's window, drawn with egui: slides on the left, the canvas in the middle, the
//! selected slide's steps and their properties on the right, problems along the bottom.
//!
//! Nothing here changes the deck directly. Every edit is a call into the engine's
//! [`Session`], which is what makes all of them undoable.

mod bar;
mod canvas;
mod editors;
mod problems;
mod properties;
mod slides;
mod stage;
mod steps;
mod style;

use std::path::PathBuf;

use eframe::egui;
use suit_core::VarId;
use suit_engine::Session;
use suit_render::Sidecar;

use crate::{bar::Dialog, canvas::Cached, stage::Stage};

pub struct SuitApp {
    pub session: Session,
    /// Where Python is, if previews are wanted. `None` leaves the canvas empty (tests).
    sidecar: Option<Sidecar>,
    /// Whether `session` has had its preview attached yet.
    previewing: bool,
    stage: Stage,
    thumbnails: Vec<Cached<(u64, [u32; 2])>>,
    /// The object last clicked on the canvas.
    pub picked: Option<VarId>,
    /// The outcome of the last action in the bar.
    notice: Option<String>,
    dialog: Option<Dialog>,
    screenshot: Option<PathBuf>,
    /// Frames drawn since the preview arrived; a screenshot waits for a few.
    settled: u32,
}

impl SuitApp {
    pub fn new(session: Session, sidecar: Option<Sidecar>) -> SuitApp {
        SuitApp {
            session,
            sidecar,
            previewing: false,
            stage: Stage::default(),
            thumbnails: Vec::new(),
            picked: None,
            notice: None,
            dialog: None,
            screenshot: None,
            settled: 0,
        }
    }

    /// Saves a picture of the window to `path` once the preview is ready, then closes it.
    pub fn with_screenshot(mut self, path: Option<PathBuf>) -> SuitApp {
        self.screenshot = path;
        self
    }

    /// The monospace font has the arrows and symbols the proportional one lacks.
    pub fn install_fonts(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();
        if let Some(proportional) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            proportional.push("Hack".to_owned());
        }
        ctx.set_fonts(fonts);
    }

    /// Replaces the open deck, starting its preview.
    fn open(&mut self, session: Session, ctx: &egui::Context) {
        self.session = session;
        self.previewing = false;
        self.attach_preview(ctx);
        self.picked = None;
        self.stage = Stage::default();
    }

    fn attach_preview(&mut self, ctx: &egui::Context) {
        if self.previewing {
            return;
        }
        self.previewing = true;
        if let Some(sidecar) = self.sidecar.clone() {
            let ctx = ctx.clone();
            let session = std::mem::replace(&mut self.session, Session::blank());
            self.session = session.with_preview(sidecar, move || ctx.request_repaint());
        }
    }

    /// Draws the whole window into `ui` and handles what the user did.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.attach_preview(ui.ctx());
        if let Some(wait) = self.session.tick() {
            ui.ctx().request_repaint_after(wait);
        }
        self.keys(ui.ctx());
        self.take_screenshot(ui.ctx());

        egui::Panel::top("bar").show(ui, |ui| self.bar(ui));
        egui::Panel::bottom("problems")
            .resizable(true)
            .default_size(110.0)
            .show(ui, |ui| self.problems(ui));
        egui::Panel::left("slides")
            .resizable(true)
            .default_size(208.0)
            .show(ui, |ui| self.slides(ui));
        egui::Panel::right("steps")
            .resizable(true)
            .default_size(380.0)
            .show(ui, |ui| {
                egui::Panel::bottom("properties")
                    .resizable(true)
                    .default_size(380.0)
                    .show(ui, |ui| self.properties(ui));
                egui::CentralPanel::default().show(ui, |ui| self.steps(ui));
            });
        egui::CentralPanel::default().show(ui, |ui| self.stage(ui));
        self.dialogs(ui.ctx());
    }

    fn keys(&mut self, ctx: &egui::Context) {
        use egui::{Key, Modifiers};
        let chord = |modifiers, key| ctx.input_mut(|input| input.consume_key(modifiers, key));
        if chord(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || chord(Modifiers::COMMAND, Key::Y)
        {
            self.session.redo();
        } else if chord(Modifiers::COMMAND, Key::Z) {
            self.session.undo();
        }
        if chord(Modifiers::COMMAND, Key::S) {
            self.save();
        }
        // The rest are plain keys, which belong to a text field while one is being typed in.
        if ctx.text_edit_focused() || self.dialog.is_some() {
            return;
        }
        let pressed = |key| ctx.input(|input| input.key_pressed(key));
        if pressed(Key::ArrowDown) {
            self.session.step_selection(true);
        }
        if pressed(Key::ArrowUp) {
            self.session.step_selection(false);
        }
        if pressed(Key::PageDown) {
            self.session.select(self.session.slide + 1, None);
        }
        if pressed(Key::PageUp) {
            self.session
                .select(self.session.slide.saturating_sub(1), None);
        }
        if pressed(Key::Delete) || pressed(Key::Backspace) {
            self.session.delete_step();
        }
    }

    /// With a screenshot requested: once the preview has arrived and been drawn, saves a
    /// picture of the window and closes it.
    fn take_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = &self.screenshot else { return };
        let taken = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = taken {
            let [width, height] = image.size.map(|side| side as u32);
            let saved = tiny_skia::IntSize::from_wh(width, height)
                .and_then(|size| tiny_skia::Pixmap::from_vec(image.as_raw().to_vec(), size))
                .map(|pixmap| pixmap.save_png(path));
            match saved {
                Some(Ok(())) => println!("saved {}", path.display()),
                _ => eprintln!("suit: could not save {}", path.display()),
            }
            self.screenshot = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.session.preview != suit_engine::Preview::Running {
            self.settled += 1;
            if self.settled == 4 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
        }
        ctx.request_repaint();
    }
}

impl eframe::App for SuitApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}
