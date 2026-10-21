//! Exporting a PDF or a website: the windows that ask how, and the website being made.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Align2, RichText};
use suit_engine::{Job, Pages, Preview, Session, WebsiteChoices};

use crate::{SuitApp, style};

/// Vercel's free plan takes no more than this in one upload from its command line.
const VERCEL_FREE: u64 = 100_000_000;

/// How a PDF is to be made.
#[derive(Clone, Copy, Default)]
pub struct PdfForm {
    pub pages: Pages,
}

/// How a website is to be made.
#[derive(Clone, Copy)]
pub struct WebsiteForm {
    /// The height of the videos; `None` for the deck's own.
    pub height: Option<u32>,
    pub notes: bool,
    pub pdf: bool,
    pub pages: Pages,
}

impl Default for WebsiteForm {
    fn default() -> WebsiteForm {
        WebsiteForm {
            height: None,
            notes: false,
            pdf: true,
            pages: Pages::Slides,
        }
    }
}

/// What the window answered.
pub enum Answer {
    Pending,
    Cancelled,
    Export,
}

/// The two ways a deck can be put on paper.
fn pages_choice(ui: &mut egui::Ui, pages: &mut Pages) {
    ui.radio_value(pages, Pages::Slides, "A page for every slide, as it ends")
        .on_hover_text("The picture each slide rests on, as manim-slides makes a PDF.");
    ui.radio_value(
        pages,
        Pages::Steps,
        "A page for every step, as the slide builds up",
    )
    .on_hover_text(
        "A page whenever a step changes the picture: a handout that shows how each \
             slide is built. Empty screens are left out.",
    );
}

/// Why the deck cannot be exported as it is, if it cannot.
fn obstacle(session: &Session, folder: &Result<PathBuf, String>) -> Option<String> {
    if let Err(problem) = folder {
        return Some(problem.clone());
    }
    if session.checked.has_errors() {
        return Some("The deck has problems; fix the red ones before exporting.".to_owned());
    }
    None
}

/// Where Manim is with the deck, for a PDF, which is drawn from what it showed.
fn pdf_obstacle(session: &Session) -> Option<String> {
    match &session.preview {
        Preview::Failed(_) => Some("Manim could not run the deck; see Problems.".to_owned()),
        _ if session.shows_every_slide() => None,
        Preview::Stopped => {
            Some("The preview was stopped. Press Try again in the top bar.".to_owned())
        }
        _ => Some("Waiting for Manim to finish the slides…".to_owned()),
    }
}

fn window(title: &str) -> egui::Window<'_> {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
}

/// The buttons at the bottom of an export window.
fn buttons(ui: &mut egui::Ui, ready: bool) -> Answer {
    ui.add_space(8.0);
    let mut answer = Answer::Pending;
    ui.horizontal(|ui| {
        let export = egui::Button::new(RichText::new("Export").strong());
        if ui.add_enabled(ready, export).clicked() {
            answer = Answer::Export;
        }
        if ui.button("Cancel").clicked() {
            answer = Answer::Cancelled;
        }
    });
    if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
        answer = Answer::Cancelled;
    }
    answer
}

/// A size of the videos, written the way screens are: `1280×720`.
fn size_of(session: &Session, height: u32) -> String {
    let config = &session.deck.config;
    let exact = f64::from(height) * f64::from(config.width) / f64::from(config.height.max(1));
    let width = ((exact / 2.0).round() as u32) * 2;
    format!("{width}×{height}")
}

impl SuitApp {
    /// The window for a PDF. Returns what was answered.
    pub(crate) fn pdf_form(&mut self, ctx: &egui::Context, form: &mut PdfForm) -> Answer {
        let target = self.session.export_path(|stem| format!("{stem}.pdf"));
        let blocked = obstacle(&self.session, &target).or_else(|| pdf_obstacle(&self.session));
        let mut answer = Answer::Pending;
        window("Export PDF").show(ctx, |ui| {
            pages_choice(ui, &mut form.pages);
            ui.add_space(6.0);
            match (&target, &blocked) {
                (_, Some(problem)) => {
                    ui.colored_label(style::WARNING, problem);
                }
                (Ok(path), None) => {
                    ui.weak(format!("Writes {} next to the deck.", file_name(path)));
                }
                (Err(_), None) => {}
            }
            answer = buttons(ui, blocked.is_none());
        });
        if blocked.is_some() {
            // Manim may be about to finish.
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
        }
        if let Answer::Export = answer {
            self.notice = Some(match self.session.export_pdf(form.pages) {
                Ok(path) => format!("Exported to {}", path.display()),
                Err(problem) => problem,
            });
        }
        answer
    }

    /// The window for a website. Returns what was answered.
    pub(crate) fn website_form(&mut self, ctx: &egui::Context, form: &mut WebsiteForm) -> Answer {
        let folder = self.session.website_folder();
        let blocked = obstacle(&self.session, &folder);
        let deck_height = self.session.deck.config.height;
        let mut answer = Answer::Pending;
        window("Export website").show(ctx, |ui| {
            ui.label("Size of the videos");
            ui.horizontal(|ui| {
                let own = format!("{}, as the deck", size_of(&self.session, deck_height));
                ui.radio_value(&mut form.height, None, own);
                for height in [720, 480].into_iter().filter(|&h| h < deck_height) {
                    ui.radio_value(
                        &mut form.height,
                        Some(height),
                        size_of(&self.session, height),
                    );
                }
            });
            ui.weak("Smaller videos render sooner and load faster on phones.");
            ui.add_space(6.0);
            ui.checkbox(&mut form.pdf, "Add a PDF of the slides to download");
            if form.pdf {
                ui.indent("pages", |ui| pages_choice(ui, &mut form.pages));
            }
            ui.checkbox(&mut form.notes, "Put my notes on it, shown with N")
                .on_hover_text("Anyone who opens the website can read them.");
            ui.add_space(6.0);
            match (&folder, &blocked) {
                (_, Some(problem)) => {
                    ui.colored_label(style::WARNING, problem);
                }
                (Ok(path), None) => {
                    ui.weak(format!(
                        "Manim renders every slide into the folder {} next to the deck. \
                         That takes a while; you can go on editing meanwhile.",
                        file_name(path)
                    ));
                }
                (Err(_), None) => {}
            }
            answer = buttons(ui, blocked.is_none() && self.website.is_none());
        });
        if let (Answer::Export, Ok(folder)) = (&answer, folder) {
            self.start_website(folder, *form);
        }
        answer
    }

    fn start_website(&mut self, folder: PathBuf, form: WebsiteForm) {
        let Some(sidecar) = self.sidecar.clone() else {
            self.notice = Some("There is no Manim here to render the slides with.".to_owned());
            return;
        };
        let choices = WebsiteChoices {
            height: form.height,
            notes: form.notes,
            pdf: form.pdf.then_some(form.pages),
        };
        match self.session.website(folder, choices, sidecar) {
            Ok(request) => {
                self.website = Some(Job::start(request));
                self.made_website = None;
                self.notice = None;
            }
            Err(problem) => self.notice = Some(problem),
        }
    }

    /// The website being made, in the top bar: how far it has got, and a way to stop it.
    /// Once it is done, where it went and a way to look at it.
    pub(crate) fn website_progress(&mut self, ui: &mut egui::Ui) {
        if let Some(job) = &self.website
            && let Some(outcome) = job.poll()
        {
            self.website = None;
            match outcome {
                Ok(report) => {
                    let megabytes = report.bytes as f64 / 1e6;
                    let mut said = format!(
                        "Website written to {}, next to the deck ({megabytes:.1} MB).",
                        file_name(&report.folder)
                    );
                    if report.bytes > VERCEL_FREE {
                        said.push_str(
                            " That is more than Vercel's free plan takes in one upload; \
                                 smaller videos make it smaller.",
                        );
                    }
                    self.notice = Some(said);
                    self.made_website = Some(report.folder);
                }
                Err(problem) => self.notice = Some(format!("Website not written: {problem}")),
            }
        }
        if let Some(job) = &self.website {
            let progress = job.progress();
            ui.spinner();
            let doing = match progress.done {
                Some(done) => format!("Website: {} {:.0}%", progress.doing, done * 100.0),
                None => format!("Website: {}…", progress.doing),
            };
            ui.weak(doing);
            let stop = egui::Button::new("Stop").small();
            if ui
                .add_enabled(!job.is_stopping(), stop)
                .on_hover_text("Stop making the website. The folder stays as it was.")
                .clicked()
            {
                job.stop();
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
            ui.separator();
        }
        if let Some(folder) = self.made_website.clone()
            && self.notice.is_some()
        {
            let index = folder.join("index.html");
            if ui
                .small_button("Watch it")
                .on_hover_text("Open the website in your browser")
                .clicked()
                && let Err(problem) = open_in_browser(&index)
            {
                self.notice = Some(problem);
            }
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Opens `page` in the browser the system uses for web pages.
fn open_in_browser(page: &Path) -> Result<(), String> {
    let mut command = if cfg!(windows) {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", ""]);
        command
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else {
        std::process::Command::new("xdg-open")
    };
    let mut opened = command.arg(page).spawn().map_err(|e| {
        format!(
            "Could not open a browser ({e}). Open {} yourself.",
            page.display()
        )
    })?;
    // It hands the page on and leaves; somebody has to see it go.
    std::thread::spawn(move || opened.wait());
    Ok(())
}
