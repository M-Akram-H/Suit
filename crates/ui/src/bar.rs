//! The bar along the top: files, undo, export, and what the preview is doing.

use std::path::PathBuf;

use eframe::egui::{self, Align, Align2, Layout, RichText};
use suit_engine::{Preview, Session};

use crate::{SuitApp, style};

/// How many seconds Manim is waited for before a way to stop it is offered.
const PATIENCE: f64 = 4.0;

/// A question the bar is waiting on.
pub enum Dialog {
    /// Asking for the file to open.
    Open {
        path: String,
        problem: Option<String>,
    },
    /// Asking where to save.
    SaveAs {
        path: String,
        problem: Option<String>,
    },
    /// Asking whether unsaved changes may be thrown away, before doing `then`.
    Discard { then: AfterDiscard },
}

#[derive(Clone, Copy)]
pub enum AfterDiscard {
    New,
    Open,
    Sample,
}

impl SuitApp {
    pub(crate) fn bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Suit").strong().size(16.0));
            let unsaved = if self.session.is_dirty() { " •" } else { "" };
            ui.label(format!("{}{unsaved}", self.session.title()));
            ui.separator();

            ui.menu_button("File", |ui| {
                if ui.button("New deck").clicked() {
                    self.leave(AfterDiscard::New, ui.ctx());
                }
                if ui.button("Open…").clicked() {
                    self.leave(AfterDiscard::Open, ui.ctx());
                }
                if ui.button("Open the sample deck").clicked() {
                    self.leave(AfterDiscard::Sample, ui.ctx());
                }
                ui.separator();
                if ui.button("Save").clicked() {
                    self.save();
                }
                if ui.button("Save as…").clicked() {
                    self.dialog = Some(Dialog::SaveAs {
                        path: self.suggested_path(),
                        problem: None,
                    });
                }
                ui.separator();
                if ui.button("Export Python").clicked() {
                    self.notice = Some(match self.session.export() {
                        Ok(path) => format!("Exported to {}", path.display()),
                        Err(problem) => problem,
                    });
                }
            });

            let undo = self.session.can_undo().map(str::to_owned);
            let redo = self.session.can_redo().map(str::to_owned);
            let hint = |what: &Option<String>, verb: &str| match what {
                Some(label) => format!("{verb}: {label}"),
                None => format!("Nothing to {}", verb.to_lowercase()),
            };
            if ui
                .add_enabled(undo.is_some(), egui::Button::new("Undo"))
                .on_hover_text(hint(&undo, "Undo"))
                .clicked()
            {
                self.session.undo();
            }
            if ui
                .add_enabled(redo.is_some(), egui::Button::new("Redo"))
                .on_hover_text(hint(&redo, "Redo"))
                .clicked()
            {
                self.session.redo();
            }
            ui.separator();
            if ui
                .button("Present")
                .on_hover_text("Show the slides full screen, starting with this one (F5)")
                .clicked()
            {
                self.start_presenting(ui.ctx());
            }
            ui.separator();

            let now = ui.input(|input| input.time);
            let running = self.session.preview == Preview::Running;
            let since = *self.asking_since.get_or_insert(now);
            if !running {
                self.asking_since = None;
            }
            match &self.session.preview {
                Preview::Running => {
                    ui.spinner();
                    ui.weak("Asking Manim…");
                    // What takes this long may never finish; nobody should have to wait
                    // it out or close the window.
                    if now - since > PATIENCE {
                        if ui
                            .small_button("Stop")
                            .on_hover_text("Stop Manim. The canvas keeps what it last showed.")
                            .clicked()
                        {
                            self.session.stop_preview();
                        }
                    } else {
                        ui.ctx()
                            .request_repaint_after(std::time::Duration::from_millis(500));
                    }
                }
                Preview::Failed(reason) => {
                    ui.colored_label(style::WARNING, format!("Preview stopped: {reason}"));
                    if ui.small_button("Try again").clicked() {
                        self.session.refresh_preview();
                    }
                }
                Preview::Stopped => {
                    ui.colored_label(style::WARNING, "Preview stopped.");
                    if ui.small_button("Try again").clicked() {
                        self.session.refresh_preview();
                    }
                }
                Preview::Ready => {}
            }
            if let Some(notice) = &self.notice {
                ui.weak(notice);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let config = &self.session.deck.config;
                let frames: u32 = self
                    .session
                    .checked
                    .slides
                    .iter()
                    .map(|s| s.frames + s.gap_frames)
                    .sum();
                let slides = match self.session.deck.slides.len() {
                    1 => "1 slide".to_owned(),
                    count => format!("{count} slides"),
                };
                ui.weak(format!(
                    "{slides} · {} · {}×{} at {} fps",
                    style::seconds(f64::from(frames) / f64::from(config.fps)),
                    config.width,
                    config.height,
                    config.fps
                ));
            });
        });
        ui.add_space(4.0);
    }

    /// Saves to the deck's file, asking for one if it has none.
    pub(crate) fn save(&mut self) {
        if self.session.path.is_none() {
            self.dialog = Some(Dialog::SaveAs {
                path: self.suggested_path(),
                problem: None,
            });
            return;
        }
        self.notice = Some(match self.session.save() {
            Ok(()) => "Saved.".to_owned(),
            Err(problem) => problem,
        });
    }

    fn suggested_path(&self) -> String {
        let path = self.session.path.clone().unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .join("untitled.suit.json")
        });
        path.display().to_string()
    }

    /// Does `then`, first asking about unsaved changes if there are any.
    fn leave(&mut self, then: AfterDiscard, ctx: &egui::Context) {
        match self.session.is_dirty() {
            true => self.dialog = Some(Dialog::Discard { then }),
            false => self.proceed(then, ctx),
        }
    }

    fn proceed(&mut self, then: AfterDiscard, ctx: &egui::Context) {
        self.dialog = None;
        match then {
            AfterDiscard::New => self.open(Session::blank(), ctx),
            AfterDiscard::Sample => self.open(Session::sample(), ctx),
            AfterDiscard::Open => {
                self.dialog = Some(Dialog::Open {
                    path: self.suggested_path(),
                    problem: None,
                });
            }
        }
    }

    pub(crate) fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        let window = |title: &str| {
            egui::Window::new(title.to_owned())
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        };
        // Each arm either puts a dialog back to keep it open, or leaves `self.dialog` empty.
        match dialog {
            Dialog::Discard { then } => {
                window("Unsaved changes").show(ctx, |ui| {
                    ui.label("This deck has changes that are not saved. Throw them away?");
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Throw away").clicked() {
                            self.proceed(then, ctx);
                        } else if !ui.button("Keep editing").clicked() {
                            self.dialog = Some(Dialog::Discard { then });
                        }
                    });
                });
            }
            Dialog::Open {
                mut path,
                mut problem,
            } => {
                let mut choice = None;
                window("Open a deck").show(ctx, |ui| {
                    choice = path_form(ui, "Open", &mut path, &problem);
                });
                match choice {
                    Some(true) => match Session::open(&PathBuf::from(path.trim())) {
                        Ok(session) => self.open(session, ctx),
                        Err(reason) => {
                            problem = Some(reason);
                            self.dialog = Some(Dialog::Open { path, problem });
                        }
                    },
                    Some(false) => {}
                    None => self.dialog = Some(Dialog::Open { path, problem }),
                }
            }
            Dialog::SaveAs {
                mut path,
                mut problem,
            } => {
                let mut choice = None;
                window("Save the deck as").show(ctx, |ui| {
                    choice = path_form(ui, "Save", &mut path, &problem);
                });
                match choice {
                    Some(true) => match self.session.save_as(&PathBuf::from(path.trim())) {
                        Ok(()) => self.notice = Some("Saved.".to_owned()),
                        Err(reason) => {
                            problem = Some(reason);
                            self.dialog = Some(Dialog::SaveAs { path, problem });
                        }
                    },
                    Some(false) => {}
                    None => self.dialog = Some(Dialog::SaveAs { path, problem }),
                }
            }
        }
    }
}

/// A file path field with a confirm and a cancel button. Returns `Some(true)` when
/// confirmed, `Some(false)` when cancelled, `None` while still open.
fn path_form(
    ui: &mut egui::Ui,
    confirm: &str,
    path: &mut String,
    problem: &Option<String>,
) -> Option<bool> {
    ui.label("File");
    let field = ui.add(egui::TextEdit::singleline(path).desired_width(520.0));
    if let Some(problem) = problem {
        ui.colored_label(style::ERROR, problem);
    }
    ui.add_space(6.0);
    let entered = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
    ui.horizontal(|ui| {
        if ui.button(confirm).clicked() || entered {
            Some(true)
        } else if ui.button("Cancel").clicked() {
            Some(false)
        } else {
            None
        }
    })
    .inner
}
