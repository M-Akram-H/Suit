//! The canvas: what the slide looks like after the selected step, the bar of things that
//! can be put on it, and picking and dragging what is there.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, vec2};
use suit_core::{Anim, Arg, Args, Command, Expr, Node, Place, Updater, VarId};
use suit_engine::{Preview, Session};
use suit_render::Snapshot;

use crate::{
    SuitApp,
    bar::Dialog,
    canvas::{Cached, Frame, rasterize},
    editors,
    files::{Browser, Purpose},
    style::{ACCENT, UV, WARNING},
};

/// What the picture of the stage shows: preview generation, slide, step, pixel size, how
/// far something is being dragged (in thousandths of a scene unit), and while the slide is
/// being played, the frame.
type StageKey = (
    u64,
    usize,
    Option<usize>,
    [u32; 2],
    Option<(VarId, [i32; 2])>,
    Option<usize>,
);

/// What can be put on a slide from the bar above the canvas: the label, and the object.
const PALETTE: [(&str, &str); 12] = [
    ("Text", "Text"),
    ("Formula", "MathTex"),
    ("Circle", "Circle"),
    ("Square", "Square"),
    ("Rectangle", "Rectangle"),
    ("Triangle", "Triangle"),
    ("Star", "Star"),
    ("Dot", "Dot"),
    ("Line", "Line"),
    ("Arrow", "Arrow"),
    ("Axes", "Axes"),
    ("Number line", "NumberLine"),
];

/// The rest of the catalogue, in a menu.
const MORE: [(&str, &str); 18] = [
    ("Ellipse", "Ellipse"),
    ("Rounded rectangle", "RoundedRectangle"),
    ("Regular polygon", "RegularPolygon"),
    ("Polygon", "Polygon"),
    ("Arc", "Arc"),
    ("Dashed line", "DashedLine"),
    ("Double arrow", "DoubleArrow"),
    ("Brace under…", "Brace"),
    ("Frame around…", "SurroundingRectangle"),
    ("Number", "DecimalNumber"),
    ("Whole number", "Integer"),
    ("LaTeX text", "Tex"),
    ("Styled text", "MarkupText"),
    ("Grid", "NumberPlane"),
    ("Graph of a function", "Plot"),
    ("Tangent to a graph", "Tangent"),
    ("Curve", "ParametricFunction"),
    ("Number that can change", "ValueTracker"),
];

/// Whether what `ctor` makes is made to be typed: a text, whose words are its point, or
/// a graph, whose rule is.
fn written(ctor: &str) -> bool {
    matches!(ctor, "Text" | "MarkupText" | "MathTex" | "Tex" | "Plot")
}

#[derive(Default)]
pub struct Stage {
    picture: Cached<StageKey>,
    /// An object following the pointer, and how far it has come, in scene units.
    drag: Option<(VarId, [f32; 2])>,
    /// A finished drag that needs a decision, or whose result Manim has yet to show.
    dropped: Option<Dropped>,
    /// Set while the slide is being played rather than edited.
    playback: Option<Playback>,
}

/// Playing the selected slide as Manim animates it.
struct Playback {
    /// The selection and the state of the deck this began with; any change to them ends it.
    began: (usize, Option<usize>, u64),
    /// The frame shown.
    frame: usize,
    /// While it runs: the time at which frame 0 was, or would have been, shown.
    origin: Option<f64>,
    /// Start running as soon as there is a recording to run.
    eager: bool,
}

struct Dropped {
    var: VarId,
    /// How far the object was moved, and where its centre ended up, in scene units.
    offset: [f32; 2],
    to: [f64; 2],
    /// Where on screen to ask what the move should be; `None` once that is decided.
    ask_at: Option<egui::Pos2>,
}

impl SuitApp {
    pub(crate) fn stage(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.weak("Add");
            for (label, ctor) in PALETTE {
                if ui
                    .button(label)
                    .on_hover_text(format!("Put a {} on the slide", label.to_lowercase()))
                    .clicked()
                {
                    self.picked = self.session.add_object(ctor);
                    // What a text says is the first thing anyone wants to change.
                    if written(ctor) {
                        editors::type_into_the_text(ui.ctx());
                    }
                }
            }
            if ui
                .button("Picture")
                .on_hover_text("Put a picture from a file on the slide")
                .clicked()
            {
                let browser = Browser::new(Purpose::Picture, &self.deck_folder());
                self.dialog = Some(Dialog::Picture(browser));
            }
            let more = ui
                .menu_button("More", |ui| {
                    for (label, ctor) in MORE {
                        if ui.button(label).clicked() {
                            // Some go around another object: the picked one, or the newest.
                            match self.session.add_object_about(ctor, self.picked) {
                                Some(var) => {
                                    self.picked = Some(var);
                                    if written(ctor) {
                                        editors::type_into_the_text(ui.ctx());
                                    }
                                }
                                None => {
                                    let what = label.trim_end_matches('…').to_lowercase();
                                    self.notice = Some(match ctor {
                                        "Tangent" => "A tangent needs a graph to touch: add a \
                                                      graph of a function first."
                                            .to_owned(),
                                        _ => format!("A {what} needs an object: add one first."),
                                    });
                                }
                            }
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui
                        .button("Named value")
                        .on_hover_text("A number with a name, for formulas to use")
                        .clicked()
                    {
                        self.session.add_value();
                        ui.close();
                    }
                })
                .response
                .on_hover_text("The rest of what can be put on a slide");
            more.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "More objects")
            });
        });
        ui.add_space(2.0);
        // What was picked along with an object goes when another object is picked.
        if self.picked_with != self.picked {
            self.picked_too.clear();
            self.picked_with = self.picked;
        }
        // Move the playback on before drawing its controls, so that they show the frame
        // that is on the canvas; and again after them, so that a press on Play counts from
        // this frame, not the next.
        self.advance_playback(ui.ctx());
        egui::Panel::bottom("transport")
            .show_separator_line(false)
            .show(ui, |ui| self.transport(ui));
        let playing = self.advance_playback(ui.ctx());

        let whole = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(whole, Sense::click_and_drag());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Canvas"));
        let frame = Frame::of(&self.session.deck.config);
        let rect = frame.fit(whole.shrink(18.0));
        let painter = ui.painter_at(whole);
        let pixels = ui.ctx().pixels_per_point();
        let size = [rect.width() * pixels, rect.height() * pixels].map(|side| side.round() as u32);
        let border = Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);

        // While the slide plays, the canvas shows Manim's frames and nothing can be grabbed.
        if let Some(index) = playing
            && let Some(clip) = self.session.clip()
        {
            let session = &self.session;
            let key = (
                session.generation,
                session.slide,
                session.step,
                size,
                None,
                Some(index),
            );
            let background = session.deck.config.background;
            let texture = self.stage.picture.get(ui.ctx(), "stage", key, || {
                rasterize(clip.frame(index), frame, background, size, None)
            });
            painter.image(texture, rect, UV, Color32::WHITE);
            painter.rect_stroke(rect, 0.0, border, StrokeKind::Outside);
            return;
        }

        // A dropped object stays where it was dropped until Manim has caught up with it.
        if self
            .stage
            .dropped
            .as_ref()
            .is_some_and(|dropped| dropped.ask_at.is_none())
            && (self.session.preview_is_current()
                || matches!(self.session.preview, Preview::Failed(_) | Preview::Stopped))
        {
            self.stage.dropped = None;
        }
        let moved = self.stage.drag.or(self
            .stage
            .dropped
            .as_ref()
            .map(|dropped| (dropped.var, dropped.offset)));

        let session = &self.session;
        let snapshot = session.snapshot();
        let thousandths =
            |(var, offset): (VarId, [f32; 2])| (var, offset.map(|c| (c * 1000.0) as i32));
        let key = (
            session.generation,
            session.slide,
            session.step,
            size,
            moved.map(thousandths),
            None,
        );
        let background = session.deck.config.background;
        let texture = self.stage.picture.get(ui.ctx(), "stage", key, || {
            let drawn = snapshot.into_iter().flat_map(|snapshot| &snapshot.drawn);
            rasterize(drawn, frame, background, size, moved)
        });
        painter.image(texture, rect, UV, Color32::WHITE);
        painter.rect_stroke(rect, 0.0, border, StrokeKind::Outside);

        let Some(snapshot) = snapshot else {
            let nothing_at_all = session.snapshots.steps.is_empty();
            let message = match (&session.preview, session.steps().is_empty()) {
                (Preview::Running, _) => "Waiting for Manim…",
                // Not one slide was shown: it is Manim that is the matter, not the step.
                (Preview::Failed(_), _) if nothing_at_all => {
                    "Manim is not running, so there is nothing to show here.\n\
                     The bar at the top says why."
                }
                (_, true) => "An empty slide. Pick something from the bar above to put on it.",
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

        // What is shown may be older than the deck, or an earlier step than the selected
        // one: where Manim stopped, the canvas has nothing newer to show, and says so.
        let own = snapshot.node == session.selected().map(|node| node.id);
        let behind = !session.preview_is_current() || !own;
        if behind && matches!(session.preview, Preview::Failed(_) | Preview::Stopped) {
            painter.text(
                rect.left_top() + vec2(10.0, 8.0),
                Align2::LEFT_TOP,
                "Manim stopped before this, so the picture is an earlier one. \
                 The bar at the top says why.",
                FontId::proportional(13.0),
                WARNING,
            );
        }

        if session.steps().is_empty() {
            let hint = "An empty slide. Pick something from the bar above to put on it.";
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                hint,
                FontId::proportional(15.0),
                ui.visuals().weak_text_color(),
            );
        }

        // Outline what the selected step works on, and whatever was picked.
        let touched = session.selected().map(touched).unwrap_or_default();
        let outline = |var: VarId, color: Color32| {
            let Some(bounds) = snapshot.boxes.get(&var) else {
                return;
            };
            let [left, bottom, right, top] = bounds.rect;
            let shift = moved
                .filter(|(dragged, _)| *dragged == var)
                .map_or([0.0; 2], |(_, offset)| offset);
            let corner = |x: f32, y: f32| frame.to_screen(rect, [x + shift[0], y + shift[1]]);
            let area = Rect::from_two_pos(corner(left, bottom), corner(right, top)).expand(4.0);
            // A graph may reach far beyond the picture; its outline stays with the picture.
            let area = area.intersect(rect.expand(6.0));
            if !area.is_positive() {
                return;
            }
            // An object that exists but is not shown gets a fainter outline.
            let color = if bounds.on {
                color
            } else {
                color.gamma_multiply(0.4)
            };
            painter.rect_stroke(area, 3.0, Stroke::new(1.5, color), StrokeKind::Outside);
            painter.text(
                area.left_top() - vec2(0.0, 3.0),
                Align2::LEFT_BOTTOM,
                session.name(var),
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
        for var in &self.picked_too {
            outline(*var, WARNING);
        }

        // Picking and dragging. What is outlined can be grabbed by its outline, which is
        // the only way to get hold of an object that is not shown yet.
        let scale = frame.height / rect.height();
        let outlined: Vec<VarId> = touched.iter().copied().chain(self.picked).collect();
        let under_pointer =
            |position: egui::Pos2| pick(snapshot, frame.to_scene(rect, position), &outlined);
        let pressed_at = ui.input(|input| input.pointer.press_origin());
        if response
            .hover_pos()
            .is_some_and(|position| under_pointer(position).is_some())
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if response.drag_started()
            && let Some(var) = pressed_at.and_then(under_pointer)
        {
            self.stage.drag = Some((var, [0.0, 0.0]));
            self.picked = Some(var);
        }
        if let Some((_, offset)) = &mut self.stage.drag
            && let (Some(from), Some(to)) = (pressed_at, response.interact_pointer_pos())
        {
            let moved = to - from;
            *offset = [moved.x * scale, -moved.y * scale];
        }
        let mut finished = None;
        if response.drag_stopped()
            && let Some((var, offset)) = self.stage.drag.take()
            && let Some(bounds) = snapshot.boxes.get(&var)
            && offset[0].hypot(offset[1]) > 0.02
        {
            let [left, bottom, right, top] = bounds.rect;
            let to = [
                f64::from((left + right) / 2.0 + offset[0]),
                f64::from((bottom + top) / 2.0 + offset[1]),
            ];
            let ask_at = response
                .interact_pointer_pos()
                .or(response.hover_pos())
                .unwrap_or(rect.center());
            finished = Some(Dropped {
                var,
                offset,
                to,
                ask_at: Some(ask_at),
            });
        }
        if response.clicked()
            && let Some(position) = response.interact_pointer_pos()
        {
            let hit = under_pointer(position);
            let adding = ui.input(|input| input.modifiers.shift);
            match (adding, hit, self.picked) {
                // With Shift, a click adds to what is picked, or takes away from it.
                (true, Some(hit), Some(first)) if hit != first => {
                    match self.picked_too.contains(&hit) {
                        true => self.picked_too.retain(|var| *var != hit),
                        false => self.picked_too.push(hit),
                    }
                }
                (true, None, _) | (true, _, Some(_)) => {}
                _ => self.picked = hit,
            }
        }

        // A double click on a text is a wish to change what it says.
        if response.double_clicked()
            && let Some(var) = response.interact_pointer_pos().and_then(under_pointer)
        {
            self.picked = Some(var);
            editors::type_into_the_text(ui.ctx());
        }

        if let Some(mut dropped) = finished {
            // While an object is still where it was made, dragging it says where that is.
            // Once something has moved it, the drag could mean several things, so ask.
            let settled = still_where_made(&self.session, dropped.var, snapshot);
            if settled && self.session.place_at_creation(dropped.var, dropped.to) {
                dropped.ask_at = None;
            }
            self.stage.dropped = Some(dropped);
        }
        self.ask_about_move(ui.ctx());
    }

    // ── Playing ──────────────────────────────────────────────────────────────

    /// The frame being shown, while the slide is being played.
    pub fn playing_frame(&self) -> Option<usize> {
        let playback = self.stage.playback.as_ref()?;
        self.session.clip().map(|_| playback.frame)
    }

    /// Whether the slide is running, or will as soon as Manim has recorded it.
    pub fn is_playing(&self) -> bool {
        let running = |playback: &Playback| playback.origin.is_some() || playback.eager;
        self.stage.playback.as_ref().is_some_and(running)
    }

    /// Starts playing the selected slide, or pauses it, or carries on from where it paused.
    pub fn toggle_playing(&mut self) {
        let frames = self.session.clip().map_or(0, |clip| clip.frames.len());
        match &mut self.stage.playback {
            Some(playback) if playback.origin.is_some() || playback.eager => {
                (playback.origin, playback.eager) = (None, false);
            }
            Some(playback) => {
                // From the top again if it had reached the end.
                if playback.frame + 1 >= frames {
                    playback.frame = 0;
                }
                playback.eager = true;
            }
            None => {
                let began = (
                    self.session.slide,
                    self.session.step,
                    self.session.revision(),
                );
                self.stage.playback = Some(Playback {
                    began,
                    frame: 0,
                    origin: None,
                    eager: true,
                });
                self.session.record_slide();
            }
        }
    }

    pub fn stop_playing(&mut self) {
        self.stage.playback = None;
    }

    /// Moves the playback on to the frame that is due. Returns it if there is a recording
    /// to show; `None` means the canvas shows the steps as usual.
    fn advance_playback(&mut self, ctx: &egui::Context) -> Option<usize> {
        let session = &self.session;
        let now = (session.slide, session.step, session.revision());
        let playback = self
            .stage
            .playback
            .as_mut()
            .filter(|playback| playback.began == now);
        let Some(playback) = playback else {
            // Something else was selected or the deck changed: back to editing.
            self.stage.playback = None;
            return None;
        };
        let Some(clip) = session.clip() else {
            if session.recording_problem().is_some() {
                self.stage.playback = None;
            }
            return None;
        };
        let frames = clip.frames.len();
        if frames == 0 {
            self.stage.playback = None;
            return None;
        }
        let slide = &session.deck.slides[session.slide];
        let per_second = f64::from(session.deck.config.fps) * slide.config.playback_rate;
        let time = ctx.input(|input| input.time);
        if playback.eager {
            playback.eager = false;
            playback.origin = Some(time - playback.frame as f64 / per_second);
        }
        if let Some(origin) = playback.origin {
            let due = ((time - origin) * per_second).floor().max(0.0) as usize;
            playback.frame = match (due < frames, slide.config.looping) {
                (true, _) => due,
                (false, true) => due % frames,
                (false, false) => {
                    playback.origin = None;
                    frames - 1
                }
            };
            ctx.request_repaint();
        }
        Some(playback.frame)
    }

    /// Play and pause, and a slider through the frames, under the canvas.
    fn transport(&mut self, ui: &mut egui::Ui) {
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            let label = if self.is_playing() { "Pause" } else { "Play" };
            let has_steps = !self.session.steps().is_empty();
            let play = ui
                .add_enabled(has_steps, egui::Button::new(label))
                .on_hover_text("Play this slide the way Manim animates it (Space)");
            if play.clicked() {
                self.toggle_playing();
            }
            if self.stage.playback.is_some() && ui.button("Back to editing").clicked() {
                self.stop_playing();
            }
            if !self.picked_too.is_empty()
                && ui
                    .button(format!("Group these {}…", self.picked_too.len() + 1))
                    .on_hover_text("Put the picked objects together, to move and animate as one")
                    .clicked()
            {
                self.start_grouping();
            }
            if self.stage.playback.is_some() && self.session.is_recording() {
                ui.spinner();
                ui.weak("Manim is playing the slide through…");
            }
            if let Some(problem) = self.session.recording_problem() {
                ui.colored_label(WARNING, format!("Could not play the slide: {problem}"));
            }
            let fps = f64::from(self.session.deck.config.fps);
            if let (Some(playback), Some(clip)) = (&mut self.stage.playback, self.session.clip()) {
                let last = clip.frames.len().saturating_sub(1);
                let mut frame = playback.frame;
                ui.spacing_mut().slider_width = (ui.available_width() - 150.0).max(80.0);
                let slider = ui.add(egui::Slider::new(&mut frame, 0..=last).show_value(false));
                if slider.changed() {
                    // Dragging the slider takes over from the clock.
                    (playback.frame, playback.origin, playback.eager) = (frame, None, false);
                }
                let at = |frame: usize| crate::style::seconds(frame as f64 / fps);
                ui.weak(format!("{} / {}", at(frame), at(clip.frames.len())));
            }
        });
        ui.add_space(3.0);
    }

    /// After a drag on a later step: is the move an animation, a jump, or a new start?
    fn ask_about_move(&mut self, ctx: &egui::Context) {
        let Some(dropped) = &self.stage.dropped else {
            return;
        };
        let Some(at) = dropped.ask_at else { return };
        let (var, to) = (dropped.var, dropped.to);
        let mut choice = None;
        egui::Window::new("Move")
            .title_bar(false)
            .resizable(false)
            .fixed_pos(at + vec2(12.0, 12.0))
            .show(ctx, |ui| {
                ui.label(format!("Move {} here…", self.session.name(var)));
                if ui
                    .button("Glide there")
                    .on_hover_text("Add an animation that moves it")
                    .clicked()
                {
                    choice = Some(Choice::Glide);
                }
                if ui
                    .button("Jump there")
                    .on_hover_text("Add a step that moves it instantly")
                    .clicked()
                {
                    choice = Some(Choice::Jump);
                }
                if ui
                    .button("Start from here")
                    .on_hover_text("Change where it is made, on every slide")
                    .clicked()
                {
                    choice = Some(Choice::Start);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(Choice::Cancel);
                }
            });
        let Some(choice) = choice else { return };
        let done = match choice {
            Choice::Glide => self.session.insert_move(var, to, true).is_some(),
            Choice::Jump => self.session.insert_move(var, to, false).is_some(),
            Choice::Start => self.session.place_at_creation(var, to),
            Choice::Cancel => false,
        };
        match (done, &mut self.stage.dropped) {
            // Keep showing the object where it was dropped until the preview arrives.
            (true, Some(dropped)) => dropped.ask_at = None,
            _ => self.stage.dropped = None,
        }
    }
}

enum Choice {
    Glide,
    Jump,
    Start,
    Cancel,
}

/// Whether `var` is made on the selected slide and has not been moved since, judging by
/// what Manim reported after the step that makes it and for the step shown `now`.
fn still_where_made(session: &Session, var: VarId, now: &Snapshot) -> bool {
    let Some((slide, step)) = session.creation(var) else {
        return false;
    };
    if slide != session.slide {
        return false;
    }
    if session.step == Some(step) {
        return true;
    }
    let made_by = session.steps_of(slide)[step].id;
    let when_made = session
        .snapshots
        .steps
        .iter()
        .find(|snapshot| snapshot.node == Some(made_by));
    match (
        when_made.and_then(|snapshot| snapshot.boxes.get(&var)),
        now.boxes.get(&var),
    ) {
        (Some(then), Some(now)) => then
            .rect
            .iter()
            .zip(&now.rect)
            .all(|(a, b)| (a - b).abs() < 1e-3),
        _ => false,
    }
}

/// The named object under a point of the scene: the topmost one drawn there, and of the
/// names that reach it the most specific (a member rather than its group). Failing that,
/// one of the `outlined` objects whose outline the point is in.
fn pick(snapshot: &Snapshot, [x, y]: [f32; 2], outlined: &[VarId]) -> Option<VarId> {
    let inside = |var: &VarId| {
        snapshot.boxes.get(var).is_some_and(|bounds| {
            let [left, bottom, right, top] = bounds.rect;
            (left..=right).contains(&x) && (bottom..=top).contains(&y)
        })
    };
    let drawn = snapshot.drawn.iter().rev().find_map(|drawn| {
        let mut owners: Vec<&(VarId, Vec<u32>)> = drawn.owners.iter().collect();
        owners.sort_by_key(|(_, path)| path.len());
        owners.into_iter().map(|(var, _)| *var).find(inside)
    });
    drawn.or_else(|| outlined.iter().copied().find(inside))
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
