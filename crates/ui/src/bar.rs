//! The bar along the top: files, undo, export, and what the preview is doing.

use std::path::PathBuf;

use eframe::egui::{self, Align, Align2, Layout, RichText};
use suit_engine::{Preview, Session};

use crate::{
    SuitApp,
    files::{Answer, Browser, Purpose},
    style,
};

/// How many seconds Manim is waited for before a way to stop it is offered.
const PATIENCE: f64 = 4.0;

/// A question the bar is waiting on.
pub enum Dialog {
    /// Asking for the file to open.
    Open(Browser),
    /// Asking where to save, and what to go on with once that is done.
    SaveAs {
        browser: Browser,
        then: Option<AfterDiscard>,
    },
    /// Asking for the picture a new object is to show.
    Picture(Browser),
    /// Asking whether unsaved changes may be thrown away, before doing `then`.
    Discard { then: AfterDiscard },
}

/// What was about to happen when unsaved changes got in the way.
#[derive(Clone, Copy)]
pub enum AfterDiscard {
    New,
    Open,
    Sample,
    Quit,
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
                    self.ask_where_to_save(None);
                }
                ui.separator();
                let elsewhere = self.session.pictures_elsewhere().len();
                if ui
                    .add_enabled(elsewhere > 0, egui::Button::new("Gather pictures"))
                    .on_hover_text(
                        "Copy the pictures that are somewhere else into a folder next to \
                         the deck, so that the deck's folder has all it needs",
                    )
                    .on_disabled_hover_text("Every picture is in the deck's folder already.")
                    .clicked()
                {
                    self.notice = Some(match self.session.gather_pictures() {
                        Ok(1) => "Copied 1 picture next to the deck.".to_owned(),
                        Ok(count) => format!("Copied {count} pictures next to the deck."),
                        Err(problem) => problem,
                    });
                }
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
            // Several choices are made in one go, so a click on one leaves the menu open.
            let stays_open = egui::containers::menu::MenuConfig::new()
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
            let (how, _) = egui::containers::menu::MenuButton::new("▾")
                .config(stays_open)
                .ui(ui, |ui| {
                    let options = &mut self.present_options;
                    ui.checkbox(
                        &mut options.start_paused,
                        "Wait for a key before the first slide",
                    );
                    ui.checkbox(
                        &mut options.next_ends_loops,
                        "Let a looping slide finish its round before moving on",
                    );
                    ui.checkbox(&mut options.leave_after_last, "Leave after the last slide");
                    ui.checkbox(
                        &mut options.run_through,
                        "Run through without waiting for keys",
                    );
                    ui.separator();
                    ui.checkbox(
                        &mut self.notes_shown,
                        "Show my notes in a second window (N)",
                    );
                    let mut ahead = self.session.records_ahead_of_time();
                    let asked = ui
                        .checkbox(&mut ahead, "Get the slides ready to play while I edit")
                        .on_hover_text(
                            "Manim plays every slide through in the background whenever \
                             the deck rests, so that Play and Present start at once. Off, \
                             a slide is played through when it is first asked for.",
                        );
                    if asked.changed() {
                        self.session.record_ahead_of_time(ahead);
                    }
                });
            how.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "How to present")
            });
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
                    // The slide on the canvas is done first; the ones after it may have
                    // moved with it, and are done next.
                    ui.weak(match self.session.catching_up_after() {
                        true => "Updating the slides after this one…",
                        false => "Asking Manim…",
                    });
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
                    // The bar has room for a sentence. All of it is under Problems, and
                    // comes up here when the pointer rests on it.
                    let first = reason.split_inclusive(". ").next().unwrap_or(reason);
                    ui.colored_label(
                        style::WARNING,
                        format!("Preview stopped: {}", first.trim_end()),
                    )
                    .on_hover_text(reason);
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
            self.ask_where_to_save(None);
            return;
        }
        self.notice = Some(match self.session.save() {
            Ok(()) => "Saved.".to_owned(),
            Err(problem) => problem,
        });
    }

    fn ask_where_to_save(&mut self, then: Option<AfterDiscard>) {
        let browser = Browser::new(Purpose::SaveDeck, &self.suggested_path());
        self.dialog = Some(Dialog::SaveAs { browser, then });
    }

    /// The deck's file, or a name for it in the folder the editor was started in.
    fn suggested_path(&self) -> PathBuf {
        self.session.path.clone().unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .join("untitled.suit.json")
        })
    }

    /// The folder to start looking for files in: the deck's own.
    pub(crate) fn deck_folder(&self) -> PathBuf {
        let path = self.suggested_path();
        path.parent().map(PathBuf::from).unwrap_or(path)
    }

    /// Does `then`, first asking about unsaved changes if there are any.
    pub(crate) fn leave(&mut self, then: AfterDiscard, ctx: &egui::Context) {
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
                let browser = Browser::new(Purpose::OpenDeck, &self.deck_folder());
                self.dialog = Some(Dialog::Open(browser));
            }
            AfterDiscard::Quit => {
                self.leaving = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// The window is being closed: with unsaved changes, that waits for an answer.
    pub(crate) fn closing(&mut self, ctx: &egui::Context) {
        let asked = ctx.input(|input| input.viewport().close_requested());
        if asked && !self.leaving && self.session.is_dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.stop_presenting(ctx);
            self.dialog = Some(Dialog::Discard {
                then: AfterDiscard::Quit,
            });
        }
    }

    pub(crate) fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        // Each arm either puts a dialog back to keep it open, or leaves `self.dialog` empty.
        match dialog {
            Dialog::Discard { then } => {
                let mut choice = None;
                egui::Window::new("Unsaved changes")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label("This deck has changes that are not saved.");
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Save them").strong()).clicked() {
                                choice = Some(Unsaved::Save);
                            }
                            if ui.button("Throw them away").clicked() {
                                choice = Some(Unsaved::Discard);
                            }
                            if ui.button("Keep editing").clicked() {
                                choice = Some(Unsaved::Stay);
                            }
                        });
                    });
                match choice {
                    None => self.dialog = Some(Dialog::Discard { then }),
                    Some(Unsaved::Stay) => {}
                    Some(Unsaved::Discard) => self.proceed(then, ctx),
                    Some(Unsaved::Save) if self.session.path.is_none() => {
                        self.ask_where_to_save(Some(then));
                    }
                    Some(Unsaved::Save) => match self.session.save() {
                        Ok(()) => self.proceed(then, ctx),
                        Err(problem) => self.notice = Some(problem),
                    },
                }
            }
            Dialog::Open(mut browser) => match browser.show(ctx) {
                Answer::Pending => self.dialog = Some(Dialog::Open(browser)),
                Answer::Cancelled => {}
                Answer::Chosen(path) => match Session::open(&path) {
                    Ok(session) => self.open(session, ctx),
                    Err(reason) => self.dialog = Some(Dialog::Open(browser.with_problem(reason))),
                },
            },
            Dialog::SaveAs { mut browser, then } => match browser.show(ctx) {
                Answer::Pending => self.dialog = Some(Dialog::SaveAs { browser, then }),
                Answer::Cancelled => {}
                Answer::Chosen(path) => match self.session.save_as(&path) {
                    Ok(()) => {
                        self.notice = Some("Saved.".to_owned());
                        if let Some(then) = then {
                            self.proceed(then, ctx);
                        }
                    }
                    Err(reason) => {
                        let browser = browser.with_problem(reason);
                        self.dialog = Some(Dialog::SaveAs { browser, then });
                    }
                },
            },
            Dialog::Picture(mut browser) => match browser.show(ctx) {
                Answer::Pending => self.dialog = Some(Dialog::Picture(browser)),
                Answer::Cancelled => {}
                Answer::Chosen(path) => self.picked = self.session.add_picture(&path),
            },
        }
    }
}

/// What to do about unsaved changes.
enum Unsaved {
    Save,
    Discard,
    Stay,
}
