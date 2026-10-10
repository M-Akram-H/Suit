//! The canvas: what the slide looks like after the selected step, the bar of things that
//! can be put on it, and picking and dragging what is there.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, vec2};
use suit_core::{Anim, Arg, Args, Command, Expr, Node, Place, Updater, VarId};
use suit_engine::{Preview, Session};
use suit_render::Snapshot;

use crate::{
    SuitApp,
    canvas::{Cached, Frame, rasterize},
    style::{ACCENT, UV, WARNING},
};

/// What the picture of the stage shows: preview generation, slide, step, pixel size, and
/// how far something is being dragged (in thousandths of a scene unit).
type StageKey = (
    u64,
    usize,
    Option<usize>,
    [u32; 2],
    Option<(VarId, [i32; 2])>,
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

#[derive(Default)]
pub struct Stage {
    picture: Cached<StageKey>,
    /// An object following the pointer, and how far it has come, in scene units.
    drag: Option<(VarId, [f32; 2])>,
    /// A finished drag that needs a decision, or whose result Manim has yet to show.
    dropped: Option<Dropped>,
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
                }
            }
        });
        ui.add_space(2.0);

        let whole = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(whole, Sense::click_and_drag());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Canvas"));
        let frame = Frame::of(&self.session.deck.config);
        let rect = frame.fit(whole.shrink(18.0));
        let painter = ui.painter_at(whole);

        // A dropped object stays where it was dropped until Manim has caught up with it.
        if self
            .stage
            .dropped
            .as_ref()
            .is_some_and(|dropped| dropped.ask_at.is_none())
            && (self.session.preview_is_current()
                || matches!(self.session.preview, Preview::Failed(_)))
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
        let pixels = ui.ctx().pixels_per_point();
        let size = [rect.width() * pixels, rect.height() * pixels].map(|side| side.round() as u32);
        let thousandths =
            |(var, offset): (VarId, [f32; 2])| (var, offset.map(|c| (c * 1000.0) as i32));
        let key = (
            session.generation,
            session.slide,
            session.step,
            size,
            moved.map(thousandths),
        );
        let background = session.deck.config.background;
        let texture = self.stage.picture.get(ui.ctx(), "stage", key, || {
            rasterize(snapshot, frame, background, size, moved)
        });
        painter.image(texture, rect, UV, Color32::WHITE);
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
            StrokeKind::Outside,
        );

        let Some(snapshot) = snapshot else {
            let message = match (&session.preview, session.steps().is_empty()) {
                (Preview::Running, _) => "Waiting for Manim…",
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
            self.picked = under_pointer(position);
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
