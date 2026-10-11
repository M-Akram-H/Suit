//! The steps of the selected slide, in the order they happen.

use eframe::egui::{self, Align2, FontId, Rect, Sense, pos2, vec2};
use suit_check::{ObjectState, State};
use suit_codegen::pretty::Pretty;
use suit_core::{
    Anim, Arg, Call, Command, Expr, Node, Place, Presence, Raw, RawIface, SlideContent, Sym, Time,
    Updater, VarId,
};
use suit_engine::Session;
use suit_sig::{AnimParam, AnimParamType, Role};

use crate::{
    SuitApp,
    editors::{self, Scope, Slot},
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

        let playing = self.playing_frame();
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
                    // While the slide plays, a bar marks the step the playback is in.
                    let span = row.and_then(|(_, node)| session.checked.notes.get(&node.id));
                    let span = span.map(|note| note.start_frame..note.start_frame + note.frames);
                    if let (Some(frame), Some(span)) = (playing, span)
                        && span.contains(&(frame as u32))
                    {
                        let bar = Rect::from_min_size(rect.min, vec2(3.0, rect.height()));
                        painter.rect_filled(bar, 1.0, style::ACCENT);
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
        let mut duplicate = false;
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
                // Every animation that suits the object as it is now, by what it is for.
                let choices = subject
                    .as_ref()
                    .map(|object| animations_for(&self.session, object))
                    .unwrap_or_default();
                let other = ui
                    .menu_button("▾", |ui| {
                        for (purpose, anims) in &choices {
                            ui.weak(*purpose);
                            for (name, anim) in anims {
                                if ui.button(name.as_str()).clicked() {
                                    add = Some(Command::Play {
                                        anims: vec![Node::unset(anim.clone())],
                                    });
                                    ui.close();
                                }
                            }
                        }
                        if choices.is_empty() {
                            ui.weak("There is no animation for this object as it is now.");
                        }
                    })
                    .response
                    .on_hover_text("Every animation that suits the selected object");
                other.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Other animations")
                });
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
            let more = ui
                .menu_button("More", |ui| {
                    ui.add_enabled_ui(target.is_some(), |ui| {
                        if ui
                            .button("Keep doing…")
                            .on_hover_text(
                                "Have the selected object do something again on every frame: \
                                 stay next to another one, follow a number, keep turning",
                            )
                            .clicked()
                            && let Some(object) = &subject
                        {
                            add = Some(Command::Attach {
                                target: Place::var(object.var),
                                updater: Updater::Always(following(&self.session, object.var)),
                            });
                            ui.close();
                        }
                        if ui
                            .button("Stop keeping on")
                            .on_hover_text("From here on, the selected object does that no more")
                            .clicked()
                            && let Some(target) = &target
                        {
                            add = Some(Command::Detach {
                                target: target.clone(),
                            });
                            ui.close();
                        }
                        if ui
                            .button("Duplicate")
                            .on_hover_text("Make a copy of the selected object, next to it")
                            .clicked()
                        {
                            duplicate = true;
                            ui.close();
                        }
                    });
                    if ui
                        .button("Python code")
                        .on_hover_text(
                            "A step written in Python, for what the editor has no step for. \
                             It is run as it stands; `self` is the scene.",
                        )
                        .clicked()
                    {
                        let python = "# self is the scene\n".to_owned();
                        add = Some(Command::Raw(Raw {
                            python,
                            iface: RawIface::default(),
                        }));
                        ui.close();
                    }
                    if ui
                        .button("Group…")
                        .on_hover_text("Put objects together, to move and animate them as one")
                        .clicked()
                    {
                        self.start_grouping();
                        ui.close();
                    }
                })
                .response;
            more.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "More steps")
            });
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
        if duplicate && let Some(object) = &subject {
            self.picked = self.session.duplicate(object.var).or(self.picked);
        }
    }

    /// Opens the dialog for making a group, with what is picked ticked already.
    pub(crate) fn start_grouping(&mut self) {
        let grouped = self.session.grouped();
        let picked = self.picked.iter().chain(&self.picked_too);
        let free = picked.filter(|var| !grouped.contains(var));
        self.grouping = Some(free.copied().collect());
    }

    /// Choosing what goes into a group.
    pub(crate) fn grouping(&mut self, ctx: &egui::Context) {
        let Some(mut chosen) = self.grouping.take() else {
            return;
        };
        let session = &self.session;
        let grouped = session.grouped();
        let free: Vec<&ObjectState> = session
            .objects()
            .iter()
            .filter(|object| !grouped.contains(&object.var))
            .collect();
        let mut states = free
            .iter()
            .filter(|object| chosen.contains(&object.var))
            .map(|object| &object.state);
        let first = states.next();
        let mixed = states.any(|state| Some(state) != first);
        let mut done = None;
        egui::Window::new("Group")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.label(
                    "Tick what belongs together. The group can then be moved, arranged and \
                     animated as one; its members keep their names.",
                );
                ui.add_space(4.0);
                if free.len() < 2 {
                    ui.weak("There are not two objects here that are free to be grouped.");
                }
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for object in &free {
                            let mut ticked = chosen.contains(&object.var);
                            let state = match object.state {
                                State::Is(Presence::On) => "",
                                State::Is(Presence::Off) => "  (not on the slide)",
                                State::Is(Presence::Spent) => "  (used up)",
                                State::Mixed => "  (partly on the slide)",
                            };
                            let label = format!("{}{state}", session.name(object.var));
                            if ui.checkbox(&mut ticked, label).changed() {
                                match ticked {
                                    true => chosen.push(object.var),
                                    false => chosen.retain(|var| *var != object.var),
                                }
                            }
                        }
                    });
                if mixed {
                    ui.colored_label(
                        style::WARNING,
                        "Some of these are on the slide and some are not. Show or hide them \
                         first, so that they match.",
                    );
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let ready = chosen.len() >= 2 && !mixed;
                    if ui
                        .add_enabled(ready, egui::Button::new("Group these"))
                        .clicked()
                    {
                        done = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        done = Some(false);
                    }
                });
            });
        match done {
            None => self.grouping = Some(chosen),
            Some(false) => {}
            Some(true) => {
                // In the order they were made, which is the order they are drawn in.
                chosen.sort();
                self.picked = self.session.add_group(&chosen).or(self.picked);
            }
        }
    }
}

/// What an object that is to keep doing something starts by doing: staying under the
/// newest other object there is, or failing that, where it is told.
fn following(session: &Session, var: VarId) -> Call {
    let other = session
        .objects()
        .iter()
        .rev()
        .find(|object| object.var != var && object.kind.as_str() != "ValueTracker");
    match other {
        Some(other) => Call::new("next_to")
            .arg("mobject_or_point", &Place::var(other.var))
            .arg("direction", Expr::konst("DOWN")),
        None => Call::new("move_to").arg("point_or_mobject", Expr::vec3(0.0, 0.0, 0.0)),
    }
}

/// Every animation of the catalogue that `object` can be given as it is now, with whatever
/// else it needs filled in, sorted by what it is for.
fn animations_for(
    session: &Session,
    object: &ObjectState,
) -> Vec<(&'static str, Vec<(Sym, Anim)>)> {
    let sig = &session.sig;
    let scope = Scope::new(sig, &session.deck, session.objects(), Vec::new());
    let target = Place::var(object.var);
    let presence = match object.state {
        State::Is(presence) => Some(presence),
        State::Mixed => None,
    };
    let takes = |role: Role, state: &State| match role.requires() {
        Some(needed) => *state == State::Is(needed),
        None => true,
    };
    let purposes = [
        "Bring it in",
        "Draw attention to it",
        "Change it",
        "Turn it into another",
        "Take it away",
    ];
    let mut sorted: Vec<(&str, Vec<(Sym, Anim)>)> = purposes
        .iter()
        .map(|purpose| (*purpose, Vec::new()))
        .collect();
    if presence == Some(Presence::On) {
        // Not one of the catalogue's animations but all of its changes, made smoothly.
        let glide = Call::new("shift").arg("vectors", Expr::konst("RIGHT"));
        sorted[2].1.push((
            "Move, turn, resize, recolour…".into(),
            Anim::animate(&target, vec![glide]),
        ));
    }
    'anims: for (name, anim) in &sig.anims {
        let objects: Vec<(&AnimParam, Role, &Sym)> = anim
            .params
            .iter()
            .filter_map(|param| match &param.ty {
                AnimParamType::Object { role, kind } => Some((param, *role, kind)),
                AnimParamType::Value(_) => None,
            })
            .collect();
        let Some(&(first, role, kind)) = objects.first() else {
            continue;
        };
        if !sig.is_subkind(&object.kind, kind) || !takes(role, &object.state) {
            continue;
        }
        let purpose = match (objects.len(), role) {
            (2.., _) => 3,
            (_, Role::Intro) => 0,
            (_, Role::Use | Role::Flash) => 1,
            (_, Role::Mut) => 2,
            (_, Role::Outro | Role::Consume) => 4,
        };
        let mut built = Anim::prim(name).arg(&first.name, &target);
        for param in anim.params.iter().filter(|param| param.required) {
            if param.name == first.name {
                continue;
            }
            let value = match &param.ty {
                // Another object that fits, as it is now: the newest there is.
                AnimParamType::Object { role, kind } => session
                    .objects()
                    .iter()
                    .rev()
                    .find(|other| {
                        other.var != object.var
                            && sig.is_subkind(&other.kind, kind)
                            && takes(*role, &other.state)
                    })
                    .map(|other| Arg::from(&Place::var(other.var))),
                AnimParamType::Value(_) => editors::starting_value(&scope, &Slot::from(param)),
            };
            match value {
                Some(value) => built = built.arg(&param.name, value),
                // Nothing to give it: the animation is not on offer.
                None => continue 'anims,
            }
        }
        sorted[purpose].1.push((name.clone(), built));
    }
    sorted.retain(|(_, anims)| !anims.is_empty());
    sorted
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
