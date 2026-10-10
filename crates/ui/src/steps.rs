//! The steps of the selected slide, in the order they happen.

use eframe::egui::{self, Align2, FontId, Rect, Sense, pos2, vec2};
use suit_check::{ObjectState, State};
use suit_codegen::pretty::Pretty;
use suit_core::{Anim, Call, Command, Expr, Node, Place, Presence, SlideContent, Time};

use crate::{
    SuitApp,
    style::{self, ROW},
};

impl SuitApp {
    /// The object a new step is about: the one picked on the canvas, else the newest.
    pub(crate) fn subject(&self) -> Option<&ObjectState> {
        let objects = self.session.objects();
        let picked = self
            .picked
            .and_then(|var| objects.iter().find(|object| object.var == var));
        picked.or_else(|| objects.last())
    }

    pub(crate) fn steps(&mut self, ui: &mut egui::Ui) {
        let Some(slide) = self.session.deck.slides.get(self.session.slide) else {
            return;
        };
        ui.add_space(6.0);
        ui.strong(format!(
            "Steps · slide {} “{}”",
            self.session.slide + 1,
            slide.title
        ));
        let is_media = matches!(slide.content, SlideContent::Media { .. });
        self.step_buttons(ui);
        ui.add_space(2.0);

        let session = &self.session;
        let pretty = Pretty {
            deck: &session.deck,
            sig: &session.sig,
        };
        let fps = f64::from(session.deck.config.fps);
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
                    let selected = step == session.step;
                    style::as_button(&response, selected, &headline);

                    let painter = ui.painter_at(rect);
                    if selected {
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
                    // Steps that take time carry the slide; the rest is preparation.
                    let color = match frames {
                        0 => ui.visuals().text_color(),
                        _ => ui.visuals().strong_text_color(),
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
                        let length = style::seconds(f64::from(frames) / fps);
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
                    if let Some(color) = style::worst(own) {
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
                if is_media {
                    ui.weak("This slide plays a media file.");
                }
            });
        if let Some(step) = chosen {
            self.session.step = step;
        }
    }

    /// The buttons that add a step after the selected one, and move or delete that one.
    fn step_buttons(&mut self, ui: &mut egui::Ui) {
        let subject = self.subject().cloned();
        let target = subject.as_ref().map(|object| Place::var(object.var));
        let on_slide = subject
            .as_ref()
            .is_some_and(|object| object.state == State::Is(Presence::On));
        let mut add = None;
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(target.is_some(), |ui| {
                let Some(target) = &target else {
                    for label in ["Animate", "Change", "Show", "Hide"] {
                        let _ = ui
                            .button(label)
                            .on_disabled_hover_text("Add an object first.");
                    }
                    return;
                };
                if ui
                    .button("Animate")
                    .on_hover_text("Add an animation of the selected object")
                    .clicked()
                {
                    add = Some(Command::Play {
                        anims: vec![Node::unset(starting_animation(target, on_slide))],
                    });
                }
                if ui
                    .button("Change")
                    .on_hover_text("Change the selected object instantly, between two frames")
                    .clicked()
                {
                    let call = Call::new("shift").arg("vectors", Expr::vec3(0.0, 0.0, 0.0));
                    add = Some(Command::Apply {
                        target: target.clone(),
                        call,
                    });
                }
                if ui
                    .button("Show")
                    .on_hover_text("Put the selected object on the slide without an animation")
                    .clicked()
                {
                    add = Some(Command::Add {
                        targets: vec![target.clone()],
                    });
                }
                if ui
                    .button("Hide")
                    .on_hover_text("Take the selected object off the slide without an animation")
                    .clicked()
                {
                    add = Some(Command::Remove {
                        targets: vec![target.clone()],
                    });
                }
            });
            if ui
                .button("Pause")
                .on_hover_text("Hold still for a moment")
                .clicked()
            {
                add = Some(Command::Wait {
                    duration: Time::from_secs(1.0),
                });
            }
            ui.separator();
            let step = self.session.step;
            let count = self.session.steps().len();
            let has_step = step.is_some();
            if style::arrow(
                ui,
                step.is_some_and(|step| step > 0),
                "↑",
                "Move step earlier",
            ) {
                self.session.move_step(false);
            }
            if style::arrow(
                ui,
                step.is_some_and(|step| step + 1 < count),
                "↓",
                "Move step later",
            ) {
                self.session.move_step(true);
            }
            if ui
                .add_enabled(has_step, egui::Button::new("Delete"))
                .on_hover_text("Delete the selected step")
                .clicked()
            {
                self.session.delete_step();
            }
        });
        if let Some(command) = add {
            self.session.insert_step(command);
        }
    }
}

/// The animation a new step starts with: bring the object in if it is not on the slide,
/// draw attention to it if it is.
pub(crate) fn starting_animation(target: &Place, on_slide: bool) -> Anim {
    match on_slide {
        true => Anim::prim("Indicate").arg("mobject", target),
        false => Anim::prim("FadeIn").arg("mobject", target),
    }
}

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
