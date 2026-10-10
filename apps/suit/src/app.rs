//! The window: slides on the left, the canvas in the middle, the selected slide's steps and
//! their properties on the right, problems along the bottom.

use std::path::PathBuf;

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, Rect, RichText, Sense, Stroke, StrokeKind, pos2,
    vec2,
};
use suit_check::{Diagnostic, Severity, State};
use suit_codegen::pretty::Pretty;
use suit_core::{Anim, Arg, Args, Command, Expr, Node, Place, Presence, Updater, VarId};
use suit_render::Snapshot;

use crate::{
    Launch,
    canvas::{Cached, Frame, rasterize},
    session::{Preview, Session},
};

const ERROR: Color32 = Color32::from_rgb(0xf0, 0x6a, 0x6a);
const WARNING: Color32 = Color32::from_rgb(0xe8, 0xb8, 0x4a);
const ACCENT: Color32 = Color32::from_rgb(0x58, 0xc4, 0xdd);
const ROW: f32 = 26.0;

/// What a texture of the stage shows: preview generation, slide, step, pixel size.
type StageKey = (u64, usize, Option<usize>, [u32; 2]);

pub struct SuitApp {
    session: Session,
    stage: Cached<StageKey>,
    thumbnails: Vec<Cached<(u64, [u32; 2])>>,
    /// The object last clicked on the canvas.
    picked: Option<VarId>,
    /// The outcome of the last action in the bar.
    notice: Option<String>,
    screenshot: Option<PathBuf>,
    /// Frames drawn since the preview arrived; a screenshot waits for a few.
    settled: u32,
}

impl SuitApp {
    pub fn new(creation: &eframe::CreationContext<'_>, launch: Launch) -> Result<Self, String> {
        let mut session = Session::open(launch.deck)?;
        if let Some((slide, step)) = launch.select {
            session.select(slide.saturating_sub(1), step.checked_sub(1));
        }
        session.refresh_preview(&creation.egui_ctx);

        // The monospace font has the arrows and symbols the proportional one lacks.
        let mut fonts = egui::FontDefinitions::default();
        if let Some(proportional) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            proportional.push("Hack".to_owned());
        }
        creation.egui_ctx.set_fonts(fonts);
        Ok(SuitApp {
            session,
            stage: Cached::default(),
            thumbnails: Vec::new(),
            picked: None,
            notice: None,
            screenshot: launch.screenshot,
            settled: 0,
        })
    }
}

impl eframe::App for SuitApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.session.poll();
        self.keys(ui.ctx());
        self.take_screenshot(ui.ctx());

        egui::Panel::top("bar").show(ui, |ui| self.bar(ui));
        egui::Panel::bottom("problems")
            .resizable(true)
            .default_size(118.0)
            .show(ui, |ui| self.problems(ui));
        egui::Panel::left("slides")
            .resizable(true)
            .default_size(208.0)
            .show(ui, |ui| self.slides(ui));
        egui::Panel::right("steps")
            .resizable(true)
            .default_size(350.0)
            .show(ui, |ui| {
                egui::Panel::bottom("properties")
                    .resizable(true)
                    .default_size(300.0)
                    .show(ui, |ui| self.properties(ui));
                egui::CentralPanel::default().show(ui, |ui| self.steps(ui));
            });
        egui::CentralPanel::default().show(ui, |ui| self.stage(ui));
    }
}

impl SuitApp {
    fn keys(&mut self, ctx: &egui::Context) {
        let pressed = |key| ctx.input(|input| input.key_pressed(key));
        if pressed(egui::Key::ArrowDown) {
            self.session.move_step(true);
        }
        if pressed(egui::Key::ArrowUp) {
            self.session.move_step(false);
        }
        if pressed(egui::Key::PageDown) {
            self.session.select(self.session.slide + 1, None);
        }
        if pressed(egui::Key::PageUp) {
            self.session
                .select(self.session.slide.saturating_sub(1), None);
        }
    }

    // ── Bar ──────────────────────────────────────────────────────────────────

    fn bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Suit").strong().size(16.0));
            ui.label(self.session.title());
            ui.separator();
            let busy = matches!(self.session.preview, Preview::Running);
            if ui
                .add_enabled(!busy, egui::Button::new("Refresh preview"))
                .clicked()
            {
                self.session.refresh_preview(ui.ctx());
            }
            if ui.button("Export Python").clicked() {
                self.notice = Some(match self.session.export() {
                    Ok(path) => format!("Exported to {}", path.display()),
                    Err(problem) => problem,
                });
            }
            ui.separator();
            match &self.session.preview {
                Preview::Running => {
                    ui.spinner();
                    ui.label("Asking Manim what this looks like…");
                }
                Preview::Failed(reason) => {
                    ui.colored_label(WARNING, format!("Preview stopped: {reason}"));
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
                ui.weak(format!(
                    "{} slides · {} · {}×{} at {} fps",
                    self.session.deck.slides.len(),
                    seconds(f64::from(frames) / f64::from(config.fps)),
                    config.width,
                    config.height,
                    config.fps
                ));
            });
        });
        ui.add_space(4.0);
    }

    // ── Slides ───────────────────────────────────────────────────────────────

    fn slides(&mut self, ui: &mut egui::Ui) {
        let Self {
            session,
            thumbnails,
            ..
        } = self;
        thumbnails.resize_with(session.deck.slides.len(), Cached::default);
        let frame = Frame::of(&session.deck.config);
        let fps = f64::from(session.deck.config.fps);

        ui.add_space(6.0);
        ui.strong("Slides");
        ui.add_space(2.0);
        let mut chosen = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let slides = session.deck.slides.iter().zip(thumbnails.iter_mut());
                for (index, (slide, thumbnail)) in slides.enumerate() {
                    let width = ui.available_width();
                    let picture = vec2(width - 12.0, (width - 12.0) / frame.aspect());
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(width, picture.y + 34.0), Sense::click());
                    let painter = ui.painter_at(rect);
                    let selected = index == session.slide;
                    if selected {
                        painter.rect_filled(
                            rect,
                            6.0,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.45),
                        );
                    } else if response.hovered() {
                        painter.rect_filled(rect, 6.0, ui.visuals().widgets.hovered.weak_bg_fill);
                    }

                    // The picture is what the slide rests on once it has played.
                    let picture_rect = Rect::from_min_size(rect.min + vec2(6.0, 6.0), picture);
                    let pixels = ui.ctx().pixels_per_point();
                    let size =
                        [picture.x * pixels, picture.y * pixels].map(|side| side.round() as u32);
                    let texture = thumbnail.get(
                        ui.ctx(),
                        &format!("slide-{index}"),
                        (session.generation, size),
                        || {
                            rasterize(
                                session.snapshots.last_of(index),
                                frame,
                                session.deck.config.background,
                                size,
                            )
                        },
                    );
                    painter.image(texture, picture_rect, UV, Color32::WHITE);
                    let outline = match selected {
                        true => Stroke::new(1.5, ACCENT),
                        false => {
                            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
                        }
                    };
                    painter.rect_stroke(picture_rect, 2.0, outline, StrokeKind::Outside);

                    let baseline = picture_rect.bottom() + 14.0;
                    let report = &session.checked.slides[index];
                    let mut caption = format!("{}  {}", index + 1, slide.title);
                    if slide.config.looping {
                        caption.push_str("  ⟳");
                    }
                    let text = ui.visuals().text_color();
                    painter.text(
                        pos2(rect.left() + 8.0, baseline),
                        Align2::LEFT_CENTER,
                        caption,
                        FontId::proportional(13.0),
                        text,
                    );
                    let length = seconds(f64::from(report.frames + report.gap_frames) / fps);
                    let weak = ui.visuals().weak_text_color();
                    let right = painter.text(
                        pos2(rect.right() - 8.0, baseline),
                        Align2::RIGHT_CENTER,
                        length,
                        FontId::proportional(12.0),
                        weak,
                    );
                    if let Some(color) = worst(
                        session
                            .checked
                            .diagnostics
                            .iter()
                            .filter(|d| d.slide == index),
                    ) {
                        painter.text(
                            pos2(right.left() - 6.0, baseline),
                            Align2::RIGHT_CENTER,
                            "⚠",
                            FontId::proportional(13.0),
                            color,
                        );
                    }
                    if response.clicked() {
                        chosen = Some(index);
                    }
                    ui.add_space(2.0);
                }
            });
        if let Some(index) = chosen {
            session.select(index, None);
            self.picked = None;
        }
    }

    // ── Steps ────────────────────────────────────────────────────────────────

    fn steps(&mut self, ui: &mut egui::Ui) {
        let session = &self.session;
        let pretty = Pretty {
            deck: &session.deck,
            sig: &session.sig,
        };
        let fps = f64::from(session.deck.config.fps);
        let Some(slide) = session.deck.slides.get(session.slide) else {
            return;
        };

        ui.add_space(6.0);
        ui.strong(format!(
            "Steps · slide {} “{}”",
            session.slide + 1,
            slide.title
        ));
        ui.add_space(2.0);

        let problems = session.problems_of(session.slide);
        let mut chosen = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                // Row 0 is the slide as it starts; rows 1… are its steps.
                let rows =
                    std::iter::once(None).chain(session.steps().iter().enumerate().map(Some));
                for row in rows {
                    let step = row.map(|(index, _)| index);
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
                    let painter = ui.painter_at(rect);
                    if step == session.step {
                        painter.rect_filled(
                            rect,
                            4.0,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.6),
                        );
                    } else if response.hovered() {
                        painter.rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
                    }
                    let middle = rect.center().y;
                    let weak = ui.visuals().weak_text_color();
                    let strong = ui.visuals().strong_text_color();

                    let (number, icon, headline, frames) = match row {
                        None => (String::new(), "⏵", "Start of the slide".to_owned(), 0),
                        Some((index, node)) => {
                            let frames = session
                                .checked
                                .notes
                                .get(&node.id)
                                .map_or(0, |note| note.frames);
                            (
                                (index + 1).to_string(),
                                icon(&node.item),
                                pretty.headline(&node.item),
                                frames,
                            )
                        }
                    };
                    // Steps that take time carry the slide; the rest is preparation.
                    let color = if frames > 0 {
                        strong
                    } else {
                        ui.visuals().text_color()
                    };
                    painter.text(
                        pos2(rect.left() + 22.0, middle),
                        Align2::RIGHT_CENTER,
                        number,
                        FontId::proportional(11.5),
                        weak,
                    );
                    painter.text(
                        pos2(rect.left() + 30.0, middle),
                        Align2::LEFT_CENTER,
                        icon,
                        FontId::proportional(13.0),
                        color,
                    );

                    let mut right = rect.right() - 8.0;
                    if frames > 0 {
                        let length = seconds(f64::from(frames) / fps);
                        right = painter
                            .text(
                                pos2(right, middle),
                                Align2::RIGHT_CENTER,
                                length,
                                FontId::proportional(12.0),
                                weak,
                            )
                            .left()
                            - 6.0;
                    }
                    let own = problems
                        .iter()
                        .filter(|(at, _)| *at == step && step.is_some())
                        .map(|(_, d)| *d);
                    if let Some(color) = worst(own) {
                        right = painter
                            .text(
                                pos2(right, middle),
                                Align2::RIGHT_CENTER,
                                "⚠",
                                FontId::proportional(13.0),
                                color,
                            )
                            .left()
                            - 6.0;
                    }
                    let room = Rect::from_min_max(
                        pos2(rect.left() + 52.0, rect.top()),
                        pos2(right, rect.bottom()),
                    );
                    ui.painter_at(room).text(
                        room.left_center(),
                        Align2::LEFT_CENTER,
                        headline,
                        FontId::proportional(13.5),
                        color,
                    );

                    if response.clicked() {
                        chosen = Some(step);
                    }
                }
                if matches!(slide.content, suit_core::SlideContent::Media { .. }) {
                    ui.weak("This slide plays a media file.");
                }
            });
        if let Some(step) = chosen {
            self.session.step = step;
            self.picked = None;
        }
    }

    // ── Properties ───────────────────────────────────────────────────────────

    fn properties(&mut self, ui: &mut egui::Ui) {
        let session = &self.session;
        let pretty = Pretty {
            deck: &session.deck,
            sig: &session.sig,
        };
        let fps = f64::from(session.deck.config.fps);
        ui.add_space(6.0);
        ui.strong("Properties");
        ui.add_space(2.0);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                match session.selected() {
                    Some(node) => {
                        ui.label(
                            RichText::new(pretty.headline(&node.item))
                                .size(15.0)
                                .strong(),
                        );
                        let note = session
                            .checked
                            .notes
                            .get(&node.id)
                            .cloned()
                            .unwrap_or_default();
                        if note.frames > 0 {
                            ui.weak(format!(
                                "{} · frames {}–{} of the slide",
                                seconds(f64::from(note.frames) / fps),
                                note.start_frame,
                                note.start_frame + note.frames - 1
                            ));
                        } else {
                            ui.weak("Takes no time: it happens between two frames.");
                        }
                        ui.add_space(6.0);
                        ui.add(
                            egui::Label::new(RichText::new(pretty.command(&node.item)).monospace())
                                .wrap(),
                        );
                        let problems = session.problems_of(session.slide);
                        for (_, problem) in problems.iter().filter(|(at, _)| *at == session.step) {
                            ui.add_space(6.0);
                            diagnostic(ui, problem);
                        }
                    }
                    None => self.slide_properties(ui),
                }
                if let Some(var) = self.picked {
                    ui.add_space(8.0);
                    ui.separator();
                    self.object_properties(ui, var);
                }
            });
    }

    fn slide_properties(&self, ui: &mut egui::Ui) {
        let session = &self.session;
        let Some(slide) = session.deck.slides.get(session.slide) else {
            return;
        };
        let report = &session.checked.slides[session.slide];
        let fps = f64::from(session.deck.config.fps);
        ui.label(
            RichText::new(format!("Slide {} “{}”", session.slide + 1, slide.title))
                .size(15.0)
                .strong(),
        );
        let hold = match report.gap_frames {
            0 => String::new(),
            frames => format!(" + {} hold", seconds(f64::from(frames) / fps)),
        };
        ui.weak(format!("{}{hold}", seconds(f64::from(report.frames) / fps)));
        ui.add_space(6.0);
        let config = &slide.config;
        let behaviour = match (config.looping, config.auto_next) {
            (true, _) => "Loops until you move on.",
            (false, true) => "Moves on by itself when it has played.",
            (false, false) => "Waits for you at the end.",
        };
        ui.label(behaviour);
        if !config.notes.is_empty() {
            ui.add_space(4.0);
            ui.weak("Speaker notes");
            ui.label(&config.notes);
        }
        ui.add_space(6.0);
        let names = |states: &[suit_check::ObjectState]| {
            let on = states
                .iter()
                .filter(|s| !matches!(s.state, State::Is(Presence::Off | Presence::Spent)));
            let names: Vec<&str> = on
                .filter_map(|s| session.deck.names.get(&s.var).map(String::as_str))
                .collect();
            if names.is_empty() {
                "nothing".to_owned()
            } else {
                names.join(", ")
            }
        };
        ui.weak("On the slide when it starts");
        ui.label(names(&report.entry));
        ui.add_space(4.0);
        ui.weak("On the slide when it ends");
        ui.label(names(&report.exit));
        for (_, problem) in session
            .problems_of(session.slide)
            .iter()
            .filter(|(at, _)| at.is_none())
        {
            ui.add_space(6.0);
            diagnostic(ui, problem);
        }
    }

    fn object_properties(&self, ui: &mut egui::Ui, var: VarId) {
        let session = &self.session;
        let pretty = Pretty {
            deck: &session.deck,
            sig: &session.sig,
        };
        ui.label(RichText::new(pretty.var(var)).size(15.0).strong());
        // Where the object is made, wherever in the deck that is.
        for slide in 0..session.deck.slides.len() {
            for (step, node) in session.steps_of(slide).iter().enumerate() {
                if matches!(&node.item, Command::LetObject { var: made, .. } if *made == var) {
                    ui.weak(format!("Made on slide {}, step {}", slide + 1, step + 1));
                    ui.add_space(4.0);
                    ui.add(
                        egui::Label::new(RichText::new(pretty.command(&node.item)).monospace())
                            .wrap(),
                    );
                }
            }
        }
    }

    // ── Problems ─────────────────────────────────────────────────────────────

    fn problems(&mut self, ui: &mut egui::Ui) {
        let session = &self.session;
        let diagnostics = &session.checked.diagnostics;
        ui.add_space(6.0);
        ui.strong(match diagnostics.len() {
            0 => "Problems".to_owned(),
            count => format!("Problems ({count})"),
        });
        let locations = session.locations();
        let mut chosen = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                if diagnostics.is_empty() {
                    ui.weak("None. The deck is ready to export.");
                }
                for problem in diagnostics {
                    let step = problem
                        .node
                        .and_then(|id| locations.get(&id))
                        .map(|&(_, step)| step);
                    let place = match step {
                        Some(step) => format!("Slide {}, step {}", problem.slide + 1, step + 1),
                        None => format!("Slide {}", problem.slide + 1),
                    };
                    let response = ui
                        .horizontal(|ui| {
                            ui.colored_label(severity_color(problem.severity), "⚠");
                            ui.weak(place);
                            ui.label(&problem.message)
                        })
                        .response
                        .interact(Sense::click());
                    if let Some(help) = &problem.help {
                        ui.horizontal(|ui| {
                            ui.add_space(22.0);
                            ui.weak(help);
                        });
                    }
                    if response.clicked() {
                        chosen = Some((problem.slide, step));
                    }
                }
            });
        if let Some((slide, step)) = chosen {
            self.session.select(slide, step);
        }
    }

    // ── Canvas ───────────────────────────────────────────────────────────────

    fn stage(&mut self, ui: &mut egui::Ui) {
        let whole = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(whole, Sense::click());
        let session = &self.session;
        let frame = Frame::of(&session.deck.config);
        let rect = frame.fit(whole.shrink(18.0));
        let painter = ui.painter_at(whole);

        let snapshot = session.snapshot();
        let pixels = ui.ctx().pixels_per_point();
        let size = [rect.width() * pixels, rect.height() * pixels].map(|side| side.round() as u32);
        let key = (session.generation, session.slide, session.step, size);
        let background = session.deck.config.background;
        let texture = self.stage.get(ui.ctx(), "stage", key, || {
            rasterize(snapshot, frame, background, size)
        });
        painter.image(texture, rect, UV, Color32::WHITE);
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
            StrokeKind::Outside,
        );

        let Some(snapshot) = snapshot else {
            let message = match session.preview {
                Preview::Running => "Waiting for Manim…",
                _ => "Manim did not get as far as this step.",
            };
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                message,
                FontId::proportional(15.0),
                ui.visuals().weak_text_color(),
            );
            return;
        };

        // Outline what the selected step works on, and whatever was clicked.
        let touched = session.selected().map(touched).unwrap_or_default();
        let outline = |var: VarId, color: Color32| {
            let Some(bounds) = snapshot.boxes.get(&var) else {
                return;
            };
            let [left, bottom, right, top] = bounds.rect;
            let area = Rect::from_two_pos(
                frame.to_screen(rect, [left, bottom]),
                frame.to_screen(rect, [right, top]),
            )
            .expand(4.0);
            // An object that exists but is not shown gets a fainter outline.
            let color = if bounds.on {
                color
            } else {
                color.gamma_multiply(0.4)
            };
            painter.rect_stroke(area, 3.0, Stroke::new(1.5, color), StrokeKind::Outside);
            let name = session.deck.names.get(&var).map_or("?", String::as_str);
            painter.text(
                area.left_top() - vec2(0.0, 3.0),
                Align2::LEFT_BOTTOM,
                name,
                FontId::proportional(12.0),
                color,
            );
        };
        for var in &touched {
            outline(*var, ACCENT);
        }
        if let Some(var) = self.picked.filter(|var| !touched.contains(var)) {
            outline(var, WARNING);
        }

        if response.clicked()
            && let Some(position) = response.interact_pointer_pos()
        {
            self.picked = pick(snapshot, frame.to_scene(rect, position));
        }
    }

    // ── Screenshot ───────────────────────────────────────────────────────────

    /// With `--screenshot`: once the preview has arrived and been drawn, saves a picture of
    /// the window and closes it.
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
        if !matches!(self.session.preview, Preview::Running) {
            self.settled += 1;
            if self.settled == 4 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
        }
        ctx.request_repaint();
    }
}

/// The whole of a texture.
const UV: Rect = Rect {
    min: pos2(0.0, 0.0),
    max: pos2(1.0, 1.0),
};

fn icon(command: &Command) -> &'static str {
    match command {
        Command::LetObject { .. } | Command::LetValue { .. } => "+",
        Command::Apply { .. } => "✏",
        Command::Add { .. } => "👁",
        Command::Remove { .. } => "🚫",
        Command::Play { .. } => "▶",
        Command::Wait { .. } => "⏸",
        Command::Attach { .. } => "🔁",
        Command::Detach { .. } => "⏹",
        Command::Raw(_) => "🐍",
    }
}

/// A length of time for lists: at most two decimals, none if it is whole.
fn seconds(secs: f64) -> String {
    let text = format!("{secs:.2}");
    format!("{} s", text.trim_end_matches('0').trim_end_matches('.'))
}

fn severity_color(severity: Severity) -> Color32 {
    match severity {
        Severity::Error => ERROR,
        Severity::Warning => WARNING,
    }
}

/// The colour for the most serious of `diagnostics`; `None` if there are none.
fn worst<'d>(diagnostics: impl Iterator<Item = &'d Diagnostic>) -> Option<Color32> {
    let mut found = None;
    for diagnostic in diagnostics {
        if diagnostic.severity == Severity::Error {
            return Some(ERROR);
        }
        found = Some(WARNING);
    }
    found
}

fn diagnostic(ui: &mut egui::Ui, problem: &Diagnostic) {
    ui.colored_label(
        severity_color(problem.severity),
        format!("⚠ {}", problem.message),
    );
    if let Some(help) = &problem.help {
        ui.weak(help);
    }
}

/// The topmost named object under a point of the scene.
fn pick(snapshot: &Snapshot, [x, y]: [f32; 2]) -> Option<VarId> {
    let inside = |var: &VarId| {
        snapshot.boxes.get(var).is_some_and(|bounds| {
            let [left, bottom, right, top] = bounds.rect;
            (left..=right).contains(&x) && (bottom..=top).contains(&y)
        })
    };
    let owners = snapshot
        .drawn
        .iter()
        .rev()
        .flat_map(|drawn| drawn.owners.iter().map(|(var, _)| var));
    owners.copied().find(|var| inside(var))
}

/// The objects a step mentions, for outlining them on the canvas.
fn touched(node: &Node<Command>) -> Vec<VarId> {
    let mut vars = Vec::new();
    let mut add = |place: &Place| {
        if !vars.contains(&place.root) {
            vars.push(place.root);
        }
    };
    fn in_args(args: &Args, add: &mut impl FnMut(&Place)) {
        for (_, arg) in args {
            match arg {
                Arg::Expr(Expr::Place(place)) => add(place),
                Arg::Expr(Expr::Var(var)) => add(&Place::var(*var)),
                _ => {}
            }
        }
    }
    fn in_anim(anim: &Anim, add: &mut impl FnMut(&Place)) {
        match anim {
            Anim::Prim { args, .. } => in_args(args, add),
            Anim::Animate { target, .. } => add(target),
            Anim::Par { items, .. } | Anim::Seq { items, .. } => {
                items.iter().for_each(|item| in_anim(&item.item, add))
            }
        }
    }
    match &node.item {
        Command::LetObject { var, .. } => add(&Place::var(*var)),
        Command::Apply { target, .. } | Command::Detach { target } => add(target),
        Command::Add { targets } | Command::Remove { targets } => targets.iter().for_each(&mut add),
        Command::Play { anims } => anims.iter().for_each(|anim| in_anim(&anim.item, &mut add)),
        Command::Attach { target, updater } => {
            add(target);
            let (Updater::Always(call) | Updater::Every { call, .. }) = updater;
            in_args(&call.args, &mut add);
        }
        Command::LetValue { .. } | Command::Wait { .. } | Command::Raw(_) => {}
    }
    vars
}
