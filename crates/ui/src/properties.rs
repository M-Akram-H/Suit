//! The properties of what is selected: a step, or the slide itself.
//!
//! A step is edited on a copy; if the copy ends up different, it replaces the step through
//! the engine. That keeps every editor free of bookkeeping and makes each change undoable.

use eframe::egui::{self, RichText};
use suit_check::{ObjectState, State};
use suit_codegen::pretty::Pretty;
use suit_core::{
    Anim, Args, Call, Command, Expr, Node, Object, Opts, Place, Presence, Sym, Time, UNBOUND,
    Updater, VarId,
};
use suit_sig::Inside;

use crate::{
    SuitApp,
    editors::{self, Scope, Slot},
    steps::starting_animation,
    style,
};

impl SuitApp {
    pub(crate) fn properties(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.strong("Properties");
        ui.add_space(2.0);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                match self.session.selected().map(|node| node.id) {
                    Some(_) => self.step_properties(ui),
                    None => self.slide_properties(ui),
                }
                if let Some(var) = self.picked {
                    ui.add_space(8.0);
                    ui.separator();
                    self.picked_object(ui, var);
                }
            });
    }

    fn step_properties(&mut self, ui: &mut egui::Ui) {
        let session = &self.session;
        let Some(node) = session.selected() else {
            return;
        };
        let pretty = Pretty {
            deck: &session.deck,
            sig: &session.sig,
        };
        let fps = f64::from(session.deck.config.fps);

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
        match note.frames {
            0 => ui.weak("Takes no time: it happens between two frames."),
            frames => ui.weak(format!(
                "{} · frames {}–{} of the slide",
                style::seconds(f64::from(frames) / fps),
                note.start_frame,
                note.start_frame + frames - 1
            )),
        };
        ui.add_space(6.0);

        let scope = Scope::new(
            &session.sig,
            &session.deck,
            session.objects_before(),
            session.values_before(),
        )
        .knowing(&session.snapshots.remarks);
        let subject = self.subject().cloned();
        let mut command = node.item.clone();
        let mut renamed = None;
        ui.push_id(node.id, |ui| {
            let run_time = |id| {
                session
                    .checked
                    .notes
                    .get(&id)
                    .and_then(|note| note.run_time)
            };
            edit_command(
                ui,
                &scope,
                &mut command,
                subject.as_ref(),
                &run_time,
                &mut renamed,
            );
        });

        let problems = session.problems_of(session.slide);
        for (_, problem) in problems.iter().filter(|(at, _)| *at == session.step) {
            ui.add_space(6.0);
            style::diagnostic(ui, problem);
        }
        ui.add_space(8.0);
        ui.collapsing("As text", |ui| {
            ui.add(egui::Label::new(RichText::new(pretty.command(&node.item)).monospace()).wrap());
        });

        let (id, changed) = (node.id, command != node.item);
        if changed {
            self.session.replace_step(id, command);
        }
        if let Some((var, name)) = renamed {
            self.session.rename(var, &name);
        }
    }

    fn slide_properties(&mut self, ui: &mut egui::Ui) {
        let session = &self.session;
        let Some(slide) = session.deck.slides.get(session.slide) else {
            return;
        };
        let Some(report) = session.checked.slides.get(session.slide) else {
            return;
        };
        let fps = f64::from(session.deck.config.fps);
        ui.label(
            RichText::new(format!("Slide {}", session.slide + 1))
                .size(15.0)
                .strong(),
        );
        let hold = match report.gap_frames {
            0 => String::new(),
            frames => format!(" + {} hold", style::seconds(f64::from(frames) / fps)),
        };
        ui.weak(format!(
            "{}{hold}",
            style::seconds(f64::from(report.frames) / fps)
        ));
        ui.add_space(6.0);

        let (mut title, mut config) = (slide.title.clone(), slide.config.clone());
        egui::Grid::new("slide")
            .num_columns(2)
            .spacing([8.0, 5.0])
            .show(ui, |ui| {
                ui.label("title");
                ui.add(egui::TextEdit::singleline(&mut title).desired_width(220.0));
                ui.end_row();
                ui.label("at the end");
                ui.vertical(|ui| {
                    ui.checkbox(&mut config.looping, "Loop until I move on");
                    ui.checkbox(&mut config.auto_next, "Move on by itself");
                });
                ui.end_row();
                ui.label("notes");
                ui.add(
                    egui::TextEdit::multiline(&mut config.notes)
                        .desired_rows(2)
                        .desired_width(220.0),
                );
                ui.end_row();
            });

        ui.add_space(6.0);
        let names = |states: &[ObjectState]| {
            let on = states
                .iter()
                .filter(|s| !matches!(s.state, State::Is(Presence::Off | Presence::Spent)));
            let names: Vec<&str> = on.map(|s| session.name(s.var)).collect();
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
            style::diagnostic(ui, problem);
        }
        ui.add_space(8.0);
        let only = session.deck.slides.len() < 2;
        let delete = ui
            .add_enabled(!only, egui::Button::new("Delete this slide"))
            .clicked();

        if title != slide.title || config != slide.config {
            self.session
                .edit_slide(|slide| (slide.title, slide.config) = (title, config));
        }
        if delete {
            self.session.delete_slide();
        }
    }

    /// The object picked on the canvas: what it is and how it is made, to be changed
    /// right here whichever step is selected. A text's words are found this way.
    fn picked_object(&mut self, ui: &mut egui::Ui, var: VarId) {
        let session = &self.session;
        ui.label(RichText::new(session.name(var)).size(15.0).strong());
        let Some((slide, step)) = session.creation(var) else {
            ui.weak("Made by Python code.");
            return;
        };
        let here = slide == session.slide && session.step == Some(step);
        let mut go = false;
        ui.horizontal(|ui| {
            ui.weak(format!("Made on slide {}, step {}", slide + 1, step + 1));
            go = !here
                && ui
                    .small_button("Go to that step")
                    .on_hover_text("Select the step that makes this object")
                    .clicked();
        });
        if go {
            self.session.select(slide, Some(step));
            return;
        }
        // With that step selected, its settings are what is shown above.
        if here {
            return;
        }
        ui.add_space(4.0);
        let node = &session.steps_of(slide)[step];
        let scope = Scope::new(
            &session.sig,
            &session.deck,
            session.objects_before_step(slide, Some(step)),
            session.values_before_step(slide, Some(step)),
        )
        .knowing(&session.snapshots.remarks);
        let mut command = node.item.clone();
        let mut renamed = None;
        ui.push_id(("picked", node.id), |ui| {
            edit_command(ui, &scope, &mut command, None, &|_| None, &mut renamed);
        });
        let (id, changed) = (node.id, command != node.item);
        if changed {
            self.session.replace_step(id, command);
        }
        if let Some((var, name)) = renamed {
            self.session.rename(var, &name);
        }
    }
}

/// Edits one step in place.
fn edit_command(
    ui: &mut egui::Ui,
    scope: &Scope,
    command: &mut Command,
    subject: Option<&ObjectState>,
    run_time: &dyn Fn(suit_core::NodeId) -> Option<f64>,
    renamed: &mut Option<(VarId, String)>,
) {
    let any: Sym = suit_sig::ROOT_KIND.into();
    match command {
        Command::LetObject { var, value } => {
            let mut name = scope.deck.names.get(&*var).cloned().unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label("name");
                if ui
                    .add(egui::TextEdit::singleline(&mut name).desired_width(180.0))
                    .changed()
                {
                    *renamed = Some((*var, name.clone()));
                }
            });
            ui.add_space(4.0);
            edit_object(ui, scope, &mut value.item, true, Some(*var));
        }
        Command::LetValue { var, value } => {
            let mut name = scope.deck.names.get(&*var).cloned().unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label("name");
                if ui
                    .add(egui::TextEdit::singleline(&mut name).desired_width(180.0))
                    .changed()
                {
                    *renamed = Some((*var, name.clone()));
                }
            });
            ui.horizontal(|ui| {
                ui.label("is");
                editors::formula(ui, scope, value);
            });
            ui.weak("Steps after this one can use it by its name, in any formula.");
        }
        Command::Apply { target, call } => {
            ui.horizontal(|ui| {
                ui.label("object");
                edit_place(ui, scope, &any, target);
            });
            let kind = scope.kind_of(target).cloned().unwrap_or(any);
            edit_call(ui, scope, &kind, call);
        }
        Command::Add { targets } | Command::Remove { targets } => {
            let mut removed = None;
            for (index, target) in targets.iter_mut().enumerate() {
                ui.push_id(index, |ui| {
                    ui.horizontal(|ui| {
                        edit_place(ui, scope, &any, target);
                        if ui.small_button("×").clicked() {
                            removed = Some(index);
                        }
                    });
                });
            }
            if let Some(index) = removed.filter(|_| targets.len() > 1) {
                targets.remove(index);
            }
            if let Some(object) = subject
                && ui.button("Add another object").clicked()
            {
                targets.push(Place::var(object.var));
            }
        }
        Command::Play { anims } => {
            let mut removed = None;
            for (index, anim) in anims.iter_mut().enumerate() {
                ui.push_id(index, |ui| {
                    if edit_anim(ui, scope, anim, subject, run_time) {
                        removed = Some(index);
                    }
                });
                ui.add_space(4.0);
            }
            if let Some(index) = removed.filter(|_| anims.len() > 1) {
                anims.remove(index);
            }
            if let Some(object) = subject
                && ui
                    .button("Add animation")
                    .on_hover_text("Another animation that plays at the same time")
                    .clicked()
            {
                let on = object.state == State::Is(Presence::On);
                anims.push(Node::unset(starting_animation(&Place::var(object.var), on)));
            }
            // Several animations play together unless they are told to take turns.
            let in_turn =
                matches!(anims.as_slice(), [only] if matches!(only.item, Anim::Seq { .. }));
            if anims.len() >= 2
                && ui
                    .button("Play these one after another")
                    .on_hover_text("Each starts when the one before it has finished")
                    .clicked()
            {
                let items = std::mem::take(anims);
                let opts = Opts::default();
                anims.push(Node::unset(Anim::Seq { items, opts }));
            } else if in_turn
                && ui
                    .button("Play these together")
                    .on_hover_text("All start at the same moment")
                    .clicked()
                && let Some(Anim::Seq { items, .. }) = anims.pop().map(|only| only.item)
            {
                *anims = items;
            }
        }
        Command::Wait { duration } => {
            ui.horizontal(|ui| {
                ui.label("for");
                edit_time(ui, duration);
            });
        }
        Command::Attach { target, updater } => {
            ui.horizontal(|ui| {
                ui.label("object");
                edit_place(ui, scope, &any, target);
            });
            let kind = scope.kind_of(target).cloned().unwrap_or(any);
            ui.weak("On every frame, it does this again:");
            let mut timed = matches!(updater, Updater::Every { .. });
            match updater {
                Updater::Always(call) => edit_call(ui, scope, &kind, call),
                Updater::Every { dt, call } => edit_call(ui, &scope.inside(*dt), &kind, call),
            }
            let asked = ui
                .checkbox(&mut timed, "by an amount that depends on the time passed")
                .on_hover_text(
                    "Formulas in this step can then use dt: the seconds since the frame \
                     before. Turning by 90 * DEGREES * dt is a quarter turn every second.",
                );
            if asked.changed() {
                // The time step gets a variable of its own when the step goes into the deck.
                *updater = match (timed, updater.call().clone()) {
                    (true, call) => Updater::Every { dt: UNBOUND, call },
                    (false, call) => Updater::Always(call),
                };
            }
        }
        Command::Detach { target } => {
            ui.horizontal(|ui| {
                ui.label("object");
                edit_place(ui, scope, &any, target);
            });
        }
        Command::Raw(raw) => {
            ui.add(
                egui::TextEdit::multiline(&mut raw.python)
                    .code_editor()
                    .desired_rows(6)
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                ui.label("takes");
                let mut secs = raw.iface.duration.secs();
                let field = egui::DragValue::new(&mut secs)
                    .speed(0.02)
                    .range(0.0..=600.0)
                    .max_decimals(2)
                    .suffix(" s");
                if ui.add(field).changed() {
                    raw.iface.duration = Time::from_secs(secs);
                }
            });
            ui.weak(
                "The editor cannot read Python. If this code plays or waits, say how long \
                 for, so that the times of the slide stay right; and objects it makes are \
                 not known to the steps after it.",
            );
        }
    }
}

/// A duration in seconds, never less than a hundredth.
fn edit_time(ui: &mut egui::Ui, time: &mut Time) -> bool {
    let mut secs = time.secs();
    let changed = ui
        .add(
            egui::DragValue::new(&mut secs)
                .speed(0.02)
                .range(0.01..=600.0)
                .max_decimals(2)
                .suffix(" s"),
        )
        .changed();
    if changed {
        *time = Time::from_secs(secs);
    }
    changed
}

fn edit_place(ui: &mut egui::Ui, scope: &Scope, kind: &Sym, place: &mut Place) -> bool {
    let mut expr = Expr::Place(place.clone());
    let changed = editors::object(ui, scope, kind, &mut expr);
    if let Expr::Place(chosen) = expr
        && changed
    {
        *place = chosen;
    }
    changed
}

/// The kind an object expression has, for offering the methods it understands.
fn kind_of_object(scope: &Scope, object: &Object) -> Sym {
    match object {
        Object::New { ctor, .. } => match scope.sig.ctors.get(ctor) {
            Some(sig) => sig.kind.clone(),
            None => suit_sig::ROOT_KIND.into(),
        },
        Object::Group { vector: true, .. } => suit_sig::VGROUP_KIND.into(),
        Object::Group { vector: false, .. } => suit_sig::GROUP_KIND.into(),
        Object::Copy(source) => scope
            .kind_of(source)
            .cloned()
            .unwrap_or_else(|| suit_sig::ROOT_KIND.into()),
        Object::Chain { base, .. } => kind_of_object(scope, &base.item),
    }
}

/// How an object is made: its constructor's settings, then what is done to it straight
/// away. `outermost` is true for the whole expression, where further calls can be added.
fn edit_object(
    ui: &mut egui::Ui,
    scope: &Scope,
    object: &mut Object,
    outermost: bool,
    var: Option<VarId>,
) {
    let kind = kind_of_object(scope, object);
    let mut unwrap = false;
    let mut moved = Vec::new();
    match object {
        Object::New { ctor, args } => {
            ui.weak(format!("A {ctor}"));
            match scope.sig.ctors.get(ctor) {
                Some(sig) => {
                    // A graph's range is where its rule is worked out, in fine steps.
                    let sampled = matches!(&**ctor, "FunctionGraph" | "ParametricFunction");
                    let mut slots: Vec<Slot> = scope
                        .sig
                        .ctor_params(sig)
                        .into_iter()
                        .map(|param| Slot {
                            sampled,
                            ..Slot::from(param)
                        })
                        .collect();
                    colour_first(&mut slots);
                    let inside = scope.sig.inside(&sig.kind);
                    let apart = scope.sig.unseen_params(sig);
                    let apart: Vec<Sym> = apart.iter().map(|param| param.name.clone()).collect();
                    // Nothing to fill, or parts that a setting does not reach: it is not
                    // offered, and one that was given stays only to be taken away.
                    let open = inside == Some(Inside::Open);
                    slots.retain(|slot| {
                        let fill = slot.name.starts_with("fill_");
                        let shows = !(open && fill) && !apart.contains(&slot.name);
                        shows || given(args, &slot.name)
                    });
                    let filled = given(args, "fill_color");
                    if &**ctor == "Plot" {
                        // Its rule first, then where and its asymptotes, which have
                        // editors of their own; then what it shares with every shape.
                        let own =
                            |slot: &Slot| ["function", "on", "asymptotes"].contains(&&*slot.name);
                        let (mut rule, rest): (Vec<Slot>, Vec<Slot>) =
                            slots.into_iter().partition(own);
                        rule.retain(|slot| &*slot.name == "function");
                        ui.push_id("rule", |ui| editors::args(ui, scope, &rule, args));
                        ui.push_id("graph", |ui| editors::graph(ui, scope, args, var));
                        ui.push_id("made", |ui| editors::args(ui, scope, &rest, args));
                    } else {
                        ui.push_id("made", |ui| editors::args(ui, scope, &slots, args));
                    }
                    // Manim fills nothing unless told how much: a fill colour that has just
                    // been chosen comes with the opacity that shows it.
                    let unseen = given(args, "fill_color") && !given(args, "fill_opacity");
                    if !apart.is_empty() {
                        moved = unseen_by_parts(ui, scope, &sig.kind, &apart, args);
                    } else if unseen && !filled && inside == Some(Inside::Clear) {
                        args.push(("fill_opacity".into(), 1.0_f64.into()));
                    } else if unseen {
                        unseen_fill(ui, inside, args);
                    }
                }
                None => {
                    ui.colored_label(style::ERROR, "The catalogue has no such object.");
                }
            }
        }
        Object::Group { items, .. } => {
            let pretty = Pretty {
                deck: scope.deck,
                sig: scope.sig,
            };
            let names: Vec<String> = items.iter().map(|item| pretty.place(item)).collect();
            ui.weak(format!("A group of {}", names.join(", ")));
        }
        Object::Copy(source) => {
            ui.horizontal(|ui| {
                ui.label("copy of");
                edit_place(ui, scope, &suit_sig::ROOT_KIND.into(), source);
            });
        }
        Object::Chain { base, call } => {
            edit_object(ui, scope, &mut base.item, false, var);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.weak("then");
                if ui.small_button("×").on_hover_text("Remove this").clicked() {
                    unwrap = true;
                }
            });
            // Calls are numbered from the inside out, so their widgets do not collide.
            let depth = chain_depth(&base.item);
            ui.push_id(("then", depth), |ui| edit_call(ui, scope, &kind, call));
        }
    }
    // What was a setting is done to the new object first, before whatever else is.
    for call in moved {
        let inner = std::mem::replace(object, Object::Copy(Place::var(VarId(0))));
        *object = inner.then(call);
    }
    if unwrap && let Object::Chain { base, .. } = object {
        *object = std::mem::replace(&mut base.item, Object::Copy(Place::var(VarId(0))));
        return;
    }
    if outermost {
        ui.add_space(4.0);
        ui.menu_button("Then also…", |ui| {
            for (name, _) in scope.sig.methods_of(&kind) {
                if ui.button(editors::label(name)).clicked() {
                    let inner = std::mem::replace(object, Object::Copy(Place::var(VarId(0))));
                    *object = inner.then(starting_call(scope, &kind, name));
                    ui.close();
                }
            }
        });
    }
}

fn given(args: &Args, name: &str) -> bool {
    args.iter().any(|(given, _)| &**given == name)
}

/// The colour of the whole object before the colours of its inside and of its outline: it
/// is the one mostly meant, and the one that shows on everything.
fn colour_first(slots: &mut Vec<Slot>) {
    let position = |name: &str| slots.iter().position(|slot| &*slot.name == name);
    if let (Some(from), Some(to)) = (position("color"), position("fill_color"))
        && to < from
    {
        let slot = slots.remove(from);
        slots.insert(to, slot);
    }
}

/// Says that the fill colour among `args` shows nothing, and offers what was meant.
fn unseen_fill(ui: &mut egui::Ui, inside: Option<Inside>, args: &mut Args) {
    let (why, offer) = match inside {
        Some(Inside::Open) => ("There is no inside to fill here.", "Use color instead"),
        Some(Inside::Clear) => ("The inside is fully transparent.", "Add fill opacity"),
        _ => return,
    };
    let open = inside == Some(Inside::Open);
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(style::WARNING, "⚠");
        ui.weak(why);
        // The line's own colour cannot be offered where there is one already.
        let offered = !open || !given(args, "color");
        if !offered || !ui.button(offer).clicked() {
            return;
        }
        match args.iter_mut().find(|(name, _)| &**name == "fill_color") {
            Some(fill) if open => fill.0 = "color".into(),
            _ => args.push(("fill_opacity".into(), 1.0_f64.into())),
        }
    });
}

/// Says which of the settings among `args` reach no part of an object of `kind`, which is
/// made of parts, and offers the calls that do. `unseen` are the settings that reach none.
/// Returns the calls if they are asked for, which have then taken their settings with them.
fn unseen_by_parts(
    ui: &mut egui::Ui,
    scope: &Scope,
    kind: &Sym,
    unseen: &[Sym],
    args: &mut Args,
) -> Vec<Call> {
    // In the order they were given, and each once: as Problems names them.
    let mut given: Vec<&str> = Vec::new();
    for (name, _) in args.iter() {
        if unseen.contains(name) && !given.contains(&&**name) {
            given.push(name);
        }
    }
    if given.is_empty() {
        return Vec::new();
    }
    let methods = scope.sig.calls_for(kind, given.iter().copied());
    let said = |names: &[&str], quoted: bool| {
        let said: Vec<String> = names
            .iter()
            .map(|name| match quoted {
                true => format!("“{}”", editors::label(name)),
                false => editors::label(name),
            })
            .collect();
        match said.split_last() {
            Some((last, [])) => last.clone(),
            Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
            None => String::new(),
        }
    };
    let shows = if given.len() == 1 { "shows" } else { "show" };
    let why = format!(
        "{} {shows} nothing here: the parts are made separately.",
        said(&given, true)
    );
    let offer = format!("Use {} instead", said(&methods, false));

    let mut asked = Vec::new();
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(style::WARNING, "⚠");
        ui.weak(why);
        // A fill has no call that would show: the parts are lines.
        if methods.is_empty() || !ui.button(offer).clicked() {
            return;
        }
        asked = methods.iter().map(|method| Call::new(method)).collect();
        for (name, value) in std::mem::take(args) {
            let call = scope.sig.call_for(kind, &name).and_then(|(method, param)| {
                let call = asked.iter_mut().find(|call| &*call.method == method)?;
                unseen.contains(&name).then_some((call, param))
            });
            match call {
                Some((call, param)) => call.args.push((param.into(), value)),
                None => args.push((name, value)),
            }
        }
    });
    asked
}

fn chain_depth(object: &Object) -> usize {
    match object {
        Object::Chain { base, .. } => 1 + chain_depth(&base.item),
        _ => 0,
    }
}

/// A call of `method` with whatever it cannot do without already filled in.
fn starting_call(scope: &Scope, kind: &Sym, method: &Sym) -> Call {
    let mut call = Call::new(method);
    if let Some(sig) = scope.sig.method(kind, method) {
        let slots: Vec<Slot> = sig.params.iter().map(Slot::from).collect();
        fill_required(scope, &slots, &mut call.args);
    }
    // To fill is to say with what and how much: an inside mostly starts fully transparent,
    // and a colour alone would leave it so.
    if &**method == "set_fill" {
        call = call.arg("color", Expr::konst("WHITE")).arg("opacity", 1.0);
    }
    call
}

/// Gives every required parameter that has no argument a starting one.
fn fill_required(scope: &Scope, slots: &[Slot], args: &mut Args) {
    for slot in slots.iter().filter(|slot| slot.required || slot.variadic) {
        if !args.iter().any(|(name, _)| *name == slot.name)
            && let Some(start) = editors::starting_value(scope, slot)
        {
            args.push((slot.name.clone(), start));
        }
    }
}

/// A method call on an object of `kind`: which method, and its arguments.
fn edit_call(ui: &mut egui::Ui, scope: &Scope, kind: &Sym, call: &mut Call) {
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.label("do");
        egui::ComboBox::from_id_salt("method")
            .selected_text(editors::label(&call.method))
            .show_ui(ui, |ui| {
                for (name, _) in scope.sig.methods_of(kind) {
                    if ui
                        .selectable_label(call.method == *name, editors::label(name))
                        .clicked()
                    {
                        chosen = Some(name.clone());
                    }
                }
            });
    });
    if let Some(method) = chosen.filter(|method| *method != call.method) {
        *call = starting_call(scope, kind, &method);
    }
    match scope.sig.method(kind, &call.method) {
        Some(sig) => {
            let slots: Vec<Slot> = sig.params.iter().map(Slot::from).collect();
            editors::args(ui, scope, &slots, &mut call.args);
        }
        None => {
            ui.colored_label(style::ERROR, format!("A {kind} cannot do this."));
        }
    }
}

/// One animation of a step, in a frame of its own. Returns true if it should be removed.
fn edit_anim(
    ui: &mut egui::Ui,
    scope: &Scope,
    node: &mut Node<Anim>,
    subject: Option<&ObjectState>,
    run_time: &dyn Fn(suit_core::NodeId) -> Option<f64>,
) -> bool {
    let mut remove = false;
    let length = run_time(node.id);
    let together = matches!(node.item, Anim::Par { .. });
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        match &mut node.item {
            Anim::Prim { ctor, args, opts } => {
                ui.horizontal(|ui| {
                    let mut chosen = None;
                    egui::ComboBox::from_id_salt("animation")
                        .selected_text(ctor.as_str())
                        .show_ui(ui, |ui| {
                            for name in scope.sig.anims.keys() {
                                if ui.selectable_label(*ctor == *name, name.as_str()).clicked() {
                                    chosen = Some(name.clone());
                                }
                            }
                        });
                    if let Some(name) = chosen.filter(|name| *name != *ctor) {
                        retarget(scope, args, &name);
                        *ctor = name;
                    }
                    remove = ui
                        .small_button("×")
                        .on_hover_text("Remove this animation")
                        .clicked();
                });
                match scope.sig.anims.get(ctor.as_str()) {
                    Some(sig) => {
                        let slots: Vec<Slot> = sig.params.iter().map(Slot::from).collect();
                        editors::args(ui, scope, &slots, args);
                        edit_opts(ui, scope, opts, length, Some(sig.rate.as_str()), false);
                    }
                    None => {
                        ui.colored_label(style::ERROR, "The catalogue has no such animation.");
                    }
                }
            }
            Anim::Animate {
                target,
                calls,
                opts,
            } => {
                ui.horizontal(|ui| {
                    ui.strong("Change");
                    edit_place(ui, scope, &suit_sig::ROOT_KIND.into(), target);
                    remove = ui
                        .small_button("×")
                        .on_hover_text("Remove this animation")
                        .clicked();
                });
                let kind = scope
                    .kind_of(target)
                    .cloned()
                    .unwrap_or_else(|| suit_sig::ROOT_KIND.into());
                let mut dropped = None;
                for (index, call) in calls.iter_mut().enumerate() {
                    ui.push_id(index, |ui| {
                        edit_call(ui, scope, &kind, call);
                        if ui.small_button("Remove this change").clicked() {
                            dropped = Some(index);
                        }
                    });
                }
                if let Some(index) = dropped.filter(|_| calls.len() > 1) {
                    calls.remove(index);
                }
                if ui
                    .button("And also…")
                    .on_hover_text("Another change, made in the same movement")
                    .clicked()
                {
                    calls.push(starting_call(scope, &kind, &"shift".into()));
                }
                edit_opts(ui, scope, opts, length, Some("smooth"), false);
            }
            Anim::Par { items, opts } | Anim::Seq { items, opts } => {
                ui.horizontal(|ui| {
                    ui.strong(if together {
                        "Together"
                    } else {
                        "One after another"
                    });
                    remove = ui
                        .small_button("×")
                        .on_hover_text("Remove these animations")
                        .clicked();
                });
                let mut dropped = None;
                for (index, item) in items.iter_mut().enumerate() {
                    ui.push_id(index, |ui| {
                        if edit_anim(ui, scope, item, subject, run_time) {
                            dropped = Some(index);
                        }
                    });
                }
                if let Some(index) = dropped.filter(|_| items.len() > 1) {
                    items.remove(index);
                }
                if let Some(object) = subject
                    && ui.button("Add animation").clicked()
                {
                    let on = object.state == State::Is(Presence::On);
                    items.push(Node::unset(starting_animation(&Place::var(object.var), on)));
                }
                edit_opts(ui, scope, opts, length, Some("linear"), together);
            }
        }
    });
    remove
}

/// Carries the arguments of an animation over to another one: objects keep their order,
/// settings are kept where the new animation has one of the same name.
fn retarget(scope: &Scope, args: &mut Args, to: &Sym) {
    let Some(sig) = scope.sig.anims.get(to) else {
        return;
    };
    let is_object =
        |arg: &suit_core::Arg| matches!(arg, suit_core::Arg::Expr(Expr::Place(_) | Expr::Var(_)));
    let old = std::mem::take(args);
    let mut objects = old
        .iter()
        .filter(|(_, arg)| is_object(arg))
        .map(|(_, arg)| arg.clone());
    for param in &sig.params {
        match &param.ty {
            suit_sig::AnimParamType::Object { .. } => {
                if let Some(object) = objects.next() {
                    args.push((param.name.clone(), object));
                }
            }
            suit_sig::AnimParamType::Value(_) => {
                let same = old
                    .iter()
                    .find(|(name, arg)| *name == param.name && !is_object(arg));
                args.extend(same.cloned());
            }
        }
    }
    let slots: Vec<Slot> = sig.params.iter().map(Slot::from).collect();
    fill_required(scope, &slots, args);
}

/// How long an animation takes, how it eases, and for a group how far apart its parts start.
fn edit_opts(
    ui: &mut egui::Ui,
    scope: &Scope,
    opts: &mut Opts,
    length: Option<f64>,
    default_rate: Option<&str>,
    stagger: bool,
) {
    egui::Grid::new("options").num_columns(3).spacing([8.0, 5.0]).show(ui, |ui| {
        ui.label("takes");
        let mut time = opts.run_time.unwrap_or_else(|| Time::from_secs(length.unwrap_or(1.0)));
        if edit_time(ui, &mut time) {
            opts.run_time = Some(time);
        }
        if opts.run_time.is_some() && ui.small_button("×").on_hover_text("Back to the usual length").clicked() {
            opts.run_time = None;
        } else if opts.run_time.is_none() {
            ui.weak("usual");
        }
        ui.end_row();

        ui.label("easing");
        let usual = default_rate.unwrap_or("smooth");
        let shown = opts.rate.as_ref().map_or(format!("{usual} (usual)"), |rate| rate.to_string());
        egui::ComboBox::from_id_salt("easing").selected_text(shown).show_ui(ui, |ui| {
            if ui.selectable_label(opts.rate.is_none(), format!("{usual} (usual)")).clicked() {
                opts.rate = None;
            }
            for name in scope.sig.rates.keys() {
                if ui.selectable_label(opts.rate.as_ref() == Some(name), name.as_str()).clicked() {
                    opts.rate = Some(name.clone());
                }
            }
        });
        ui.label("");
        ui.end_row();

        if stagger {
            ui.label("stagger");
            let mut lag = opts.lag.unwrap_or(0.0);
            let dragged = ui
                .add(egui::DragValue::new(&mut lag).speed(0.01).range(0.0..=1.0).max_decimals(2))
                .on_hover_text("How far into one animation the next one starts: 0 is all at once, 1 is one after another.")
                .changed();
            if dragged {
                opts.lag = Some(lag).filter(|lag| *lag > 0.0);
            }
            ui.label("");
            ui.end_row();
        }
    });
}
