//! The judgments of spec §5, one method per judgment.
//!
//! Checking never stops at the first problem: after reporting one, the checker assumes the
//! intended outcome (an object that should have been introduced is treated as on scene) so
//! that one mistake does not bury the rest of the slide in follow-up errors.

use std::collections::BTreeMap;

use suit_core::{
    Anim, Arg, Args, Call, Command, Deck, Expr, Node, NodeId, Object, Opts, Place, Presence, Raw,
    Slide, SlideContent, Sym, Updater, VarId,
};
use suit_sig::{
    AnimParamType, Base, GROUP_KIND, Param, Pass, ROOT_KIND, Role, Signature, Type, VGROUP_KIND,
};

use crate::{
    diag::{Diagnostic, Severity},
    env::{Attached, Entry, Env, Member, Object as Obj, Shape, State, Tree},
    timing,
};

/// The result of checking a deck.
#[derive(Clone, Debug, Default)]
pub struct Checked {
    pub diagnostics: Vec<Diagnostic>,
    /// One report per slide, in deck order.
    pub slides: Vec<SlideReport>,
    /// Facts about individual nodes that export and the timeline need.
    pub notes: BTreeMap<NodeId, Note>,
}

impl Checked {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlideReport {
    /// Frames emitted by the blueprint, without the gap.
    pub frames: u32,
    /// Frames of the hold appended after the blueprint (spec §6.6); zero when there is none.
    pub gap_frames: u32,
    /// Updaters are running during that hold.
    pub gap_live: bool,
    /// False when the slide contains something whose length is only declared (`raw`, media).
    pub exact: bool,
    /// The slide's interface (spec §5.6): what it finds and what it leaves.
    pub entry: Vec<ObjectState>,
    pub exit: Vec<ObjectState>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectState {
    pub var: VarId,
    pub kind: Sym,
    pub state: State,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Note {
    /// Commands: frames emitted, and the frame of the slide they start on.
    pub frames: u32,
    pub start_frame: u32,
    /// Animations: run time in seconds, with defaults resolved.
    pub run_time: Option<f64>,
    /// `wait`: updaters are running, so every frame is rendered afresh.
    pub live: bool,
    /// `let g = group(…)` of objects that are on scene.
    pub group_on_scene: bool,
    /// `let x = …`: the layer (z-index) the new object is given; unique per object.
    pub layer: Option<u32>,
    /// `let x = copy(g)` of a group, whose members keep their relative order.
    pub copies_group: bool,
}

pub fn check(deck: &Deck, sig: &Signature) -> Checked {
    let mut checker = Checker {
        sig,
        deck,
        diagnostics: Vec::new(),
        notes: BTreeMap::new(),
        slide: 0,
        node: None,
        layers: 0,
    };
    if deck.slides.is_empty() {
        checker.error("empty-deck", "The deck has no slides.".into(), None);
    }
    let mut env = Env::default();
    let slides = deck
        .slides
        .iter()
        .enumerate()
        .map(|(index, slide)| checker.slide(&mut env, slide, index))
        .collect();
    Checked {
        diagnostics: checker.diagnostics,
        slides,
        notes: checker.notes,
    }
}

struct Checker<'a> {
    sig: &'a Signature,
    deck: &'a Deck,
    diagnostics: Vec<Diagnostic>,
    notes: BTreeMap<NodeId, Note>,
    slide: usize,
    node: Option<NodeId>,
    layers: u32,
}

/// A place that was looked up successfully.
struct Found {
    /// With aliases followed.
    place: Place,
    kind: Sym,
    shape: Shape,
    state: State,
}

/// The type of an expression or argument.
enum Ty {
    Base(Base),
    Mob(Sym),
    Rate,
    /// A lambda; checked once the expected parameter type is known.
    Fun,
    /// Already reported. Fits everything, so the error is not repeated.
    Unknown,
}

/// One presence transition of an animation's effect `Φ`.
struct Step {
    place: Place,
    role: Role,
    /// Kind of the place before the animation.
    kind: Sym,
    /// Kind of the place afterwards, when the animation reshapes it.
    weakened: Option<Sym>,
    /// Display name, for messages.
    name: String,
    /// From `p.animate(…)`, whose target is worked out when the `play` starts rather than
    /// when the animation begins (spec §6.4).
    eager: bool,
}

type Effect = Vec<Step>;

impl Checker<'_> {
    // ── Reporting ────────────────────────────────────────────────────────────

    fn report(
        &mut self,
        severity: Severity,
        code: &'static str,
        message: String,
        help: Option<&str>,
    ) {
        self.diagnostics.push(Diagnostic {
            severity,
            code,
            slide: self.slide,
            node: self.node,
            message,
            help: help.map(str::to_owned),
        });
    }

    fn error(&mut self, code: &'static str, message: String, help: Option<&str>) {
        self.report(Severity::Error, code, message, help);
    }

    fn warn(&mut self, code: &'static str, message: String, help: Option<&str>) {
        self.report(Severity::Warning, code, message, help);
    }

    /// `“circle”` or `“equation[0]”`.
    fn name(&self, place: &Place) -> String {
        let root = self.deck.names.get(&place.root).map_or("?", String::as_str);
        let path: String = place.path.iter().map(|i| format!("[{i}]")).collect();
        format!("“{root}{path}”")
    }

    fn note(&mut self) -> &mut Note {
        let node = self.node.expect("notes are attached to nodes");
        self.notes.entry(node).or_default()
    }

    // ── Slides (§5.6) ────────────────────────────────────────────────────────

    fn slide(&mut self, env: &mut Env, slide: &Slide, index: usize) -> SlideReport {
        self.slide = index;
        self.node = None;
        let entry = summary(env);
        let config = &slide.config;

        if config.playback_rate <= 0.0 || config.reversed_playback_rate <= 0.0 {
            self.error(
                "playback-rate",
                "Playback rates must be greater than zero.".into(),
                None,
            );
        }
        if config.looping && config.auto_next {
            self.warn(
                "loop-auto-next",
                "This slide both loops and moves on by itself.".into(),
                Some("Which of the two wins depends on the player. Choose one."),
            );
        }
        if config.looping && (index == 0 || index + 1 == self.deck.slides.len()) {
            self.warn(
                "loop-edge",
                "A looping first or last slide does not work in the web (RevealJS) export.".into(),
                None,
            );
        }

        let mut report = SlideReport {
            exact: true,
            entry,
            ..Default::default()
        };
        match &slide.content {
            SlideContent::Media { path } => {
                report.exact = false;
                if path.is_empty() {
                    self.error("media", "This media slide has no file.".into(), None);
                }
            }
            SlideContent::Blueprint(blueprint) => {
                let mut shows = false;
                for node in &blueprint.body {
                    self.node = Some(node.id);
                    let frames = self.command(env, &node.item);
                    let note = self.note();
                    (note.frames, note.start_frame) = (frames, report.frames);
                    report.frames += frames;
                    shows |= matches!(node.item, Command::Play { .. } | Command::Wait { .. });
                    if let Command::Raw(raw) = &node.item {
                        report.exact = false;
                        shows |= raw.iface.duration.0 > 0;
                    }
                }
                self.node = None;
                if !shows {
                    self.error(
                        "empty-slide",
                        "This slide shows nothing: it has no animation and no pause.".into(),
                        Some("Add an animation or a pause, or delete the slide."),
                    );
                }
                let gap = self.deck.config.gap.secs();
                if gap > 0.0 && (!config.looping || self.deck.config.gap_on_loops) {
                    (report.gap_frames, report.gap_live) = self.wait_frames(env, gap);
                }
            }
        }

        report.exit = summary(env);
        if config.looping && on_scene(&report.entry) != on_scene(&report.exit) {
            self.warn(
                "loop-jump",
                "This looping slide does not end the way it starts, so the loop will jump.".into(),
                Some("Remove what the slide adds, and bring back what it removes, before it ends."),
            );
        }
        report
    }

    // ── Commands (§5.5) ──────────────────────────────────────────────────────

    /// Checks one command, updates `env`, and returns the frames it emits (§5.7).
    fn command(&mut self, env: &mut Env, command: &Command) -> u32 {
        match command {
            Command::LetObject { var, value } => self.let_object(env, *var, value),
            Command::LetValue { var, value } => {
                match self.expr(env, value) {
                    Ty::Base(base) => {
                        env.vars.insert(*var, Entry::Value(base));
                    }
                    Ty::Unknown => {}
                    _ => self.error(
                        "value",
                        "A value can be a number, a point, a colour or text, not an object.".into(),
                        None,
                    ),
                };
            }
            Command::Apply { target, call } => {
                if let Some(found) = self.place(env, target)
                    && let Some(kind) = self.call(env, &found, call)
                {
                    reshape(env, &found.place, kind);
                }
            }
            Command::Add { targets } => {
                self.set_presence(env, targets, Presence::Off, Presence::On)
            }
            Command::Remove { targets } => {
                self.set_presence(env, targets, Presence::On, Presence::Off)
            }
            Command::Play { anims } => return self.play(env, anims),
            Command::Wait { duration } => {
                if duration.0 <= 0 {
                    self.error("duration", "A pause must be longer than zero.".into(), None);
                }
                let (frames, live) = self.wait_frames(env, duration.secs());
                self.note().live = live;
                return frames;
            }
            Command::Attach { target, updater } => self.attach(env, target, updater),
            Command::Detach { target } => {
                if let Some(found) = self.place(env, target) {
                    let inside = |place: &Place| {
                        env.resolve(place)
                            .is_some_and(|resolved| found.place.contains(&resolved))
                    };
                    let kept = env
                        .updaters
                        .iter()
                        .filter(|u| !inside(&u.target))
                        .cloned()
                        .collect();
                    env.updaters = kept;
                }
            }
            Command::Raw(raw) => return self.raw(env, raw),
        }
        0
    }

    /// T-Let and T-LetGroup.
    fn let_object(&mut self, env: &mut Env, var: VarId, value: &Node<Object>) {
        // A group may only appear as the thing being named, possibly followed by method calls:
        // `let row = vgroup(a, b).arrange(RIGHT)`.
        let mut calls = Vec::new();
        let mut base = value;
        while let Object::Chain { base: inner, call } = &base.item {
            calls.push(call);
            base = inner;
        }
        calls.reverse();

        let Object::Group { vector, items } = &base.item else {
            let copies_group = match &value.item {
                Object::Copy(source) => self
                    .place_quiet(env, source)
                    .is_some_and(|found| matches!(found.shape, Shape::Group(_))),
                _ => false,
            };
            let (kind, shape) = self
                .object(env, value)
                .unwrap_or((ROOT_KIND.into(), Shape::Opaque));
            let tree = Tree::Uniform(Presence::Off);
            env.vars.insert(
                var,
                Entry::Object(Obj {
                    kind,
                    shape,
                    tree,
                    reshaped: BTreeMap::new(),
                }),
            );
            self.layers += 1;
            let layer = self.layers;
            let note = self.note();
            (note.layer, note.copies_group) = (Some(layer), copies_group);
            return;
        };

        let group_kind: Sym = if *vector { VGROUP_KIND } else { GROUP_KIND }.into();
        let member_kind = self.sig.elem(&group_kind);
        let mut members = Vec::new();
        let mut presence = None;
        let mut sound = true;
        for (index, item) in items.iter().enumerate() {
            let name = self.name(item);
            if items[..index].contains(item) {
                self.error(
                    "group",
                    format!("{name} is listed twice in this group."),
                    None,
                );
                sound = false;
                continue;
            }
            if !item.path.is_empty() || matches!(env.vars.get(&item.root), Some(Entry::Alias(_))) {
                self.error(
                    "group",
                    format!("{name} already belongs to a group, and an object can only be in one."),
                    Some("Group the whole objects, or make a copy and group that."),
                );
                sound = false;
                continue;
            }
            let Some(found) = self.place(env, item) else {
                sound = false;
                continue;
            };
            if !self.sig.is_subkind(&found.kind, &member_kind) {
                self.error(
                    "kind",
                    format!(
                        "{name} is a {}, which cannot go into a vector group.",
                        found.kind
                    ),
                    Some("Use a plain group instead."),
                );
                sound = false;
            }
            match (found.state, presence) {
                (State::Is(p), None) => presence = Some(p),
                (State::Is(p), Some(q)) if p == q => {}
                _ => {
                    self.error(
                        "group",
                        format!("{name} is not in the same state as the rest of the group: some are on the slide and some are not."),
                        Some("Add or remove objects first so that all of them match."),
                    );
                    sound = false;
                }
            }
            members.push(Member {
                kind: found.kind,
                shape: found.shape,
            });
        }

        let presence = presence.unwrap_or(Presence::Off);
        self.note().group_on_scene = presence == Presence::On;
        if sound {
            // The operands move into the group; their names now mean places inside it.
            for (index, item) in items.iter().enumerate() {
                let alias = Place {
                    root: var,
                    path: vec![index as u32],
                };
                env.vars.insert(item.root, Entry::Alias(alias));
            }
        }
        let shape = if sound {
            Shape::Group(members)
        } else {
            Shape::Opaque
        };
        let object = Obj {
            kind: group_kind.clone(),
            shape: shape.clone(),
            tree: Tree::Uniform(presence),
            reshaped: BTreeMap::new(),
        };
        env.vars.insert(var, Entry::Object(object));
        let found = Found {
            place: Place::var(var),
            kind: group_kind,
            shape,
            state: State::Is(presence),
        };
        for call in calls {
            self.call(env, &found, call);
        }
    }

    /// T-Add and T-Remove.
    fn set_presence(&mut self, env: &mut Env, targets: &[Place], from: Presence, to: Presence) {
        let mut done: Vec<Place> = Vec::new();
        for target in targets {
            let Some(found) = self.place(env, target) else {
                continue;
            };
            let name = self.name(target);
            if done.iter().any(|earlier| !earlier.disjoint(&found.place)) {
                self.error("overlap", format!("{name} is listed more than once."), None);
                continue;
            }
            if found.state != State::Is(from) {
                let (message, help) = match (to, found.state) {
                    (Presence::On, State::Is(Presence::On)) => {
                        (format!("{name} is already on the slide."), None)
                    }
                    (Presence::Off, State::Is(Presence::Off)) => (
                        format!("{name} is not on the slide, so it cannot be removed."),
                        None,
                    ),
                    (_, State::Mixed) => (
                        format!("Only part of {name} is on the slide."),
                        Some("Add or remove the individual parts instead."),
                    ),
                    _ => (format!("{name} cannot be used here."), None),
                };
                self.error("presence", message, help);
            }
            env.set(&found.place, to);
            done.push(found.place);
        }
    }

    fn attach(&mut self, env: &mut Env, target: &Place, updater: &Updater) {
        let Some(found) = self.place(env, target) else {
            return;
        };
        let reshaped = match updater {
            Updater::Always(call) => self.call(env, &found, call),
            Updater::Every { dt, call } => {
                let mut scope = env.clone();
                scope.vars.insert(*dt, Entry::Value(Base::Real));
                self.call(&scope, &found, call)
            }
        };
        if let Some(kind) = reshaped {
            reshape(env, &found.place, kind);
        }
        let mut reads = Vec::new();
        reads_of_args(&updater.call().args, &mut reads);
        env.updaters.push(Attached {
            target: target.clone(),
            reads,
        });
    }

    /// T-Raw: the interface is a promise, not something the checker can verify.
    fn raw(&mut self, env: &mut Env, raw: &Raw) -> u32 {
        for (place, presence) in &raw.iface.requires {
            let Some(found) = self.place(env, place) else {
                continue;
            };
            if found.state != State::Is(*presence) {
                let name = self.name(place);
                self.error(
                    "presence",
                    format!("This code expects {name} to be {}.", describe(*presence)),
                    None,
                );
            }
        }
        for (var, kind, presence) in &raw.iface.defines {
            if !self.sig.kinds.contains_key(kind) {
                self.error(
                    "unknown",
                    format!("There is no kind of object called {kind}."),
                    None,
                );
                continue;
            }
            let object = Obj {
                kind: kind.clone(),
                shape: Shape::Opaque,
                tree: Tree::Uniform(*presence),
                reshaped: BTreeMap::new(),
            };
            env.vars.insert(*var, Entry::Object(object));
        }
        for (place, presence) in &raw.iface.ensures {
            if let Some(found) = self.place(env, place) {
                env.set(&found.place, *presence);
            }
        }
        match raw.iface.duration.secs() {
            secs if secs > 0.0 => timing::play_frames(secs, self.deck.config.fps),
            _ => 0,
        }
    }

    /// Frames of a `wait`, and whether updaters keep the scene live meanwhile (§5.7).
    fn wait_frames(&self, env: &Env, secs: f64) -> (u32, bool) {
        let fps = self.deck.config.fps;
        match env.updaters_live() {
            true => (timing::play_frames(secs, fps), true),
            false => (timing::still_frames(secs, fps), false),
        }
    }

    // ── Animations (§5.4) ────────────────────────────────────────────────────

    /// T-Play: the animations of one `play` form a `par`.
    fn play(&mut self, env: &mut Env, anims: &[Node<Anim>]) -> u32 {
        if anims.is_empty() {
            self.error("empty-play", "This step has no animation.".into(), None);
            return 0;
        }
        let (_, run_times) = self.parallel(env, anims);
        let longest = run_times.into_iter().fold(0.0, f64::max);
        timing::play_frames(longest, self.deck.config.fps)
    }

    /// T-Par. Each child is checked against the state before any of them ran; then their
    /// effects must be separate, and all of them are applied.
    fn parallel(&mut self, env: &mut Env, items: &[Node<Anim>]) -> (Effect, Vec<f64>) {
        let mut effects: Vec<Effect> = Vec::new();
        let mut run_times = Vec::new();
        for item in items {
            let mut scratch = env.clone();
            let (effect, run_time) = self.anim(&mut scratch, item);
            for earlier in &effects {
                if let Some((a, b)) = clash(earlier, &effect).or_else(|| clash(&effect, earlier)) {
                    let previous = self.node.replace(item.id);
                    let message = match a.name == b.name {
                        true => format!("{} is animated twice at the same time.", a.name),
                        false => format!(
                            "{} and {} overlap, and both are animated at the same time.",
                            a.name, b.name
                        ),
                    };
                    self.error(
                        "overlap",
                        message,
                        Some("Manim would silently keep only one of the two. Put them in separate steps, or combine them into one animation."),
                    );
                    self.node = previous;
                }
            }
            effects.push(effect);
            run_times.push(run_time);
        }
        let effect: Effect = effects.into_iter().flatten().collect();
        apply(env, &effect);
        (effect, run_times)
    }

    /// `Γ ⊢ a ▷ Φ`: checks `node`, applies its effect to `env`, returns it with the run time.
    fn anim(&mut self, env: &mut Env, node: &Node<Anim>) -> (Effect, f64) {
        let outer = self.node.replace(node.id);
        self.opts(node.item.opts(), matches!(node.item, Anim::Seq { .. }));
        let given = node.item.opts().run_time.map(|t| t.secs());

        let (effect, run_time) = match &node.item {
            Anim::Prim { ctor, args, .. } => {
                let (effect, default) = self.prim(env, ctor, args);
                apply(env, &effect);
                (effect, given.unwrap_or(default))
            }
            Anim::Animate { target, calls, .. } => {
                let effect = self.animate(env, target, calls);
                apply(env, &effect);
                (effect, given.unwrap_or(1.0))
            }
            Anim::Par { items, opts } => {
                if items.is_empty() {
                    self.error(
                        "empty-play",
                        "This group of animations is empty.".into(),
                        None,
                    );
                }
                let (effect, run_times) = self.parallel(env, items);
                let length = timing::group_run_time(&run_times, opts.lag.unwrap_or(0.0));
                (effect, given.unwrap_or(length))
            }
            Anim::Seq { items, .. } => {
                if items.is_empty() {
                    self.error(
                        "empty-play",
                        "This sequence of animations is empty.".into(),
                        None,
                    );
                }
                // T-Seq: each child sees the state its predecessors left.
                let mut effect = Effect::new();
                let mut run_times = Vec::new();
                for item in items {
                    let (steps, run_time) = self.anim(env, item);
                    // An eager target ignores what earlier children did to the same object,
                    // so playing it would undo them.
                    let undone = steps.iter().filter(|step| step.eager).find(|step| {
                        let changed = |earlier: &&Step| {
                            earlier.role == Role::Mut && !earlier.place.disjoint(&step.place)
                        };
                        effect.iter().any(|earlier| changed(&earlier))
                    });
                    if let Some(step) = undone {
                        let outer = self.node.replace(item.id);
                        self.error(
                            "animate-order",
                            format!("{} is changed earlier in this sequence, but this change starts from where it was before the sequence, so the earlier one would be undone.", step.name),
                            Some("Put the two changes in separate steps."),
                        );
                        self.node = outer;
                    }
                    effect.extend(steps);
                    run_times.push(run_time);
                }
                (
                    effect,
                    given.unwrap_or(timing::group_run_time(&run_times, 1.0)),
                )
            }
        };
        self.note().run_time = Some(run_time);
        self.node = outer;
        (effect, run_time)
    }

    fn opts(&mut self, opts: &Opts, is_seq: bool) {
        if opts.run_time.is_some_and(|t| t.0 <= 0) {
            self.error(
                "duration",
                "An animation must last longer than zero.".into(),
                None,
            );
        }
        if opts.lag.is_some_and(|lag| lag < 0.0) {
            self.error("duration", "The stagger cannot be negative.".into(), None);
        }
        if is_seq && opts.lag.is_some() {
            self.error(
                "duration",
                "A sequence plays one after another; it has no stagger.".into(),
                None,
            );
        }
        if let Some(rate) = &opts.rate
            && !self.sig.rates.contains_key(rate)
        {
            self.error(
                "unknown",
                format!("There is no easing called {rate}."),
                None,
            );
        }
    }

    /// T-Prim. Returns the effect and the animation's default run time.
    fn prim(&mut self, env: &Env, ctor: &Sym, args: &Args) -> (Effect, f64) {
        let Some(anim) = self.sig.anims.get(ctor) else {
            self.error(
                "unknown",
                format!("There is no animation called {ctor}."),
                None,
            );
            return (Effect::new(), 1.0);
        };
        self.arity(
            ctor,
            args,
            anim.params.iter().map(|p| (&p.name, p.required, p.pass)),
        );

        let mut effect = Effect::new();
        // Kind of the object read by a `use` parameter: what a Transform turns its source into.
        let mut template = None;
        for param in &anim.params {
            for arg in args
                .iter()
                .filter(|(name, _)| *name == param.name)
                .map(|(_, arg)| arg)
            {
                match &param.ty {
                    AnimParamType::Value(ty) => self.arg(env, ctor, &param.name, ty, arg),
                    AnimParamType::Object { role, kind } => {
                        let (step, found) = self.role(env, ctor, *role, kind, arg);
                        if *role == Role::Use {
                            template = found;
                        }
                        effect.extend(step);
                    }
                }
            }
        }

        // Within one animation, too, an object can only be changed in one way.
        for (index, step) in effect.iter().enumerate() {
            let earlier = &effect[..index];
            if step.role != Role::Use
                && earlier
                    .iter()
                    .any(|e| e.role != Role::Use && !e.place.disjoint(&step.place))
            {
                let message = format!("{} is given to {ctor} twice.", step.name);
                self.error("overlap", message, None);
            }
        }
        // An updater that reads a consumed object keeps reading its leftover geometry.
        for step in effect.iter().filter(|s| s.role == Role::Consume) {
            let reads_it = |updater: &&Attached| {
                updater
                    .reads
                    .iter()
                    .filter_map(|read| env.resolve(read))
                    .any(|read| !read.disjoint(&step.place))
            };
            if let Some(updater) = env.updaters.iter().find(reads_it) {
                let follower = self.name(&updater.target);
                self.warn(
                    "stale-updater",
                    format!(
                        "{follower} keeps following {}, which {ctor} uses up.",
                        step.name
                    ),
                    Some("Detach it first, then make it follow the new object."),
                );
            }
        }
        if anim.weakens {
            for step in effect.iter_mut().filter(|s| s.role == Role::Mut) {
                step.weakened = template
                    .as_ref()
                    .map(|other| self.sig.join(&step.kind, other));
            }
        }
        (effect, anim.run_time)
    }

    /// One object argument of a primitive animation against its role. Returns the step it
    /// contributes to the effect, and the kind of the object it was given.
    fn role(
        &mut self,
        env: &Env,
        ctor: &Sym,
        role: Role,
        kind: &Sym,
        arg: &Arg,
    ) -> (Option<Step>, Option<Sym>) {
        let place = match arg {
            Arg::Expr(Expr::Place(place)) => place.clone(),
            Arg::Expr(Expr::Var(var))
                if matches!(env.vars.get(var), Some(Entry::Object(_) | Entry::Alias(_))) =>
            {
                Place::var(*var)
            }
            Arg::Object(inline) if role == Role::Use => {
                // An unnamed target: read once, never on scene, so it has no effect.
                let found = self.object(env, inline).map(|(found, _)| found);
                if let Some(found) = &found
                    && !self.sig.is_subkind(found, kind)
                {
                    self.error(
                        "kind",
                        format!("{ctor} needs a {kind} here, not a {found}."),
                        None,
                    );
                }
                return (None, found);
            }
            Arg::Object(_) => {
                self.error(
                    "unnamed",
                    format!("{ctor} puts this object on the slide, so it needs a name."),
                    Some("Create the object first, then use it here."),
                );
                return (None, None);
            }
            _ => {
                self.error("kind", format!("{ctor} needs an object here."), None);
                return (None, None);
            }
        };
        let Some(found) = self.place(env, &place) else {
            return (None, None);
        };
        let name = self.name(&place);
        if !self.sig.is_subkind(&found.kind, kind) {
            self.error(
                "kind",
                format!("{ctor} works on a {kind}, but {name} is a {}.", found.kind),
                None,
            );
        }
        if let Some(required) = role.requires()
            && found.state != State::Is(required)
        {
            let (message, help) = presence_problem(ctor, &name, role, found.state);
            self.error("presence", message, help);
        }
        let step = Step {
            place: found.place,
            role,
            kind: found.kind.clone(),
            weakened: None,
            name,
            eager: false,
        };
        (Some(step), Some(found.kind))
    }

    /// T-Animate.
    fn animate(&mut self, env: &Env, target: &Place, calls: &[Call]) -> Effect {
        let Some(found) = self.place(env, target) else {
            return Effect::new();
        };
        let name = self.name(target);
        if found.state != State::Is(Presence::On) {
            let (message, help) =
                presence_problem(&"This change".into(), &name, Role::Mut, found.state);
            self.error("presence", message, help);
        }
        if calls.is_empty() {
            self.error(
                "empty-play",
                format!("This animation of {name} changes nothing."),
                None,
            );
        }
        let mut weakened = None;
        for call in calls {
            if let Some(kind) = self.call(env, &found, call) {
                weakened = Some(kind);
            }
            if &*call.method == "rotate" {
                self.warn(
                    "animate-rotate",
                    format!("{name} will not turn: it slides in a straight line to its rotated position."),
                    Some("Use the Rotate animation to turn it."),
                );
            }
        }
        vec![Step {
            place: found.place,
            role: Role::Mut,
            kind: found.kind,
            weakened,
            name,
            eager: true,
        }]
    }

    // ── Objects (§5.3) ───────────────────────────────────────────────────────

    /// `Γ ⊢ m : Mob κ`; `None` once an error makes the kind unknowable.
    fn object(&mut self, env: &Env, node: &Node<Object>) -> Option<(Sym, Shape)> {
        match &node.item {
            Object::New { ctor, args } => {
                let Some(sig) = self.sig.ctors.get(ctor) else {
                    self.error(
                        "unknown",
                        format!("There is no object called {ctor}."),
                        None,
                    );
                    return None;
                };
                let params = self.sig.ctor_params(sig);
                self.args(env, ctor, &params, args);
                Some((sig.kind.clone(), Shape::Opaque))
            }
            Object::Copy(source) => {
                let found = self.place(env, source)?;
                Some((found.kind, found.shape))
            }
            Object::Chain { base, call } => {
                let (kind, shape) = self.object(env, base)?;
                // The object under construction has no place yet; only its kind matters.
                let found = Found {
                    place: Place::var(VarId(u64::MAX)),
                    kind: kind.clone(),
                    shape: shape.clone(),
                    state: State::Is(Presence::Off),
                };
                match self.call(env, &found, call) {
                    Some(weakened) => Some((weakened, Shape::Opaque)),
                    None => Some((kind, shape)),
                }
            }
            Object::Group { .. } => {
                self.error(
                    "group",
                    "A group has to be given a name of its own.".into(),
                    Some("Create the group as a separate object first."),
                );
                None
            }
        }
    }

    /// A method call on an object of `found.kind`. Returns the receiver's new kind when the
    /// method reshapes it.
    fn call(&mut self, env: &Env, found: &Found, call: &Call) -> Option<Sym> {
        let Some(method) = self.sig.method(&found.kind, &call.method) else {
            let message = match self.sig.methods.contains_key(&call.method) {
                true => format!("A {} cannot do “{}”.", found.kind, call.method),
                false => format!("There is no action called “{}”.", call.method),
            };
            self.error("unknown", message, None);
            return None;
        };
        let params: Vec<&Param> = method.params.iter().collect();
        let kinds = self.args(env, &call.method, &params, &call.args);
        match method.reshapes {
            true => kinds.first().map(|other| self.sig.join(&found.kind, other)),
            false => None,
        }
    }

    // ── Arguments ────────────────────────────────────────────────────────────

    /// Checks `args` against `params`. Returns the kinds of the object arguments.
    fn args(&mut self, env: &Env, owner: &Sym, params: &[&Param], args: &Args) -> Vec<Sym> {
        self.arity(
            owner,
            args,
            params.iter().map(|p| (&p.name, p.required, p.pass)),
        );
        let mut kinds = Vec::new();
        for (name, arg) in args {
            let Some(param) = params.iter().find(|p| p.name == *name) else {
                continue;
            };
            if let Some(kind) = self.arg_kind(env, owner, name, &param.ty, arg) {
                kinds.push(kind);
            }
        }
        kinds
    }

    /// Names, repetition and required parameters; the types are checked separately.
    fn arity<'p>(
        &mut self,
        owner: &Sym,
        args: &Args,
        params: impl Iterator<Item = (&'p Sym, bool, Pass)> + Clone,
    ) {
        for (index, (name, _)) in args.iter().enumerate() {
            match params.clone().find(|(param, ..)| *param == name) {
                None => self.error(
                    "unknown",
                    format!("{owner} has no setting called “{name}”."),
                    None,
                ),
                Some((_, _, pass)) => {
                    let repeated = args[..index].iter().any(|(earlier, _)| earlier == name);
                    if repeated && pass != Pass::Variadic {
                        self.error(
                            "arguments",
                            format!("{owner} is given “{name}” twice."),
                            None,
                        );
                    }
                }
            }
        }
        for (name, required, _) in params {
            if required && !args.iter().any(|(given, _)| given == name) {
                self.error("arguments", format!("{owner} needs “{name}”."), None);
            }
        }
    }

    fn arg(&mut self, env: &Env, owner: &Sym, name: &Sym, expected: &Type, arg: &Arg) {
        self.arg_kind(env, owner, name, expected, arg);
    }

    /// Checks one argument; returns its kind if it is an object.
    fn arg_kind(
        &mut self,
        env: &Env,
        owner: &Sym,
        name: &Sym,
        expected: &Type,
        arg: &Arg,
    ) -> Option<Sym> {
        let ty = match arg {
            Arg::Expr(expr) => self.expr(env, expr),
            Arg::Object(inline) => match self.object(env, inline) {
                Some((kind, _)) => Ty::Mob(kind),
                None => Ty::Unknown,
            },
            Arg::Rate(rate) => {
                if !self.sig.rates.contains_key(rate) {
                    self.error(
                        "unknown",
                        format!("There is no easing called {rate}."),
                        None,
                    );
                }
                Ty::Rate
            }
            Arg::Fun { .. } => Ty::Fun,
        };
        if let (Ty::Fun, Arg::Fun { param, body }) = (&ty, arg) {
            let Some((input, output)) = function_type(expected) else {
                let message = format!(
                    "“{name}” of {owner} should be {}, not a formula.",
                    plain(expected)
                );
                self.error("type", message, None);
                return None;
            };
            let mut scope = env.clone();
            scope.vars.insert(*param, Entry::Value(input));
            match self.expr(&scope, body) {
                Ty::Base(found) if !found.fits(output) => {
                    let message = format!(
                        "The formula for “{name}” of {owner} gives {}, but it should give {}.",
                        plain_base(found),
                        plain_base(output)
                    );
                    self.error("type", message, None);
                }
                Ty::Mob(_) | Ty::Rate | Ty::Fun => self.error(
                    "type",
                    format!("The formula for “{name}” of {owner} must give a value."),
                    None,
                ),
                _ => {}
            }
            return None;
        }
        if !self.fits(&ty, expected) {
            let message = format!(
                "“{name}” of {owner} should be {}, but it is {}.",
                plain(expected),
                plain_ty(&ty)
            );
            self.error("type", message, None);
        }
        match ty {
            Ty::Mob(kind) => Some(kind),
            _ => None,
        }
    }

    fn fits(&self, ty: &Ty, expected: &Type) -> bool {
        match (ty, expected) {
            (Ty::Unknown, _) => true,
            (_, Type::Union(options)) => options.iter().any(|option| self.fits(ty, option)),
            (Ty::Base(found), Type::Base(wanted)) => found.fits(*wanted),
            (Ty::Mob(found), Type::Mob(wanted)) => self.sig.is_subkind(found, wanted),
            (Ty::Rate, Type::Rate) => true,
            _ => false,
        }
    }

    // ── Expressions (§5.2) ───────────────────────────────────────────────────

    fn expr(&mut self, env: &Env, expr: &Expr) -> Ty {
        match expr {
            Expr::Real(_) => Ty::Base(Base::Real),
            Expr::Int(_) => Ty::Base(Base::Int),
            Expr::Bool(_) => Ty::Base(Base::Bool),
            Expr::Str(_) => Ty::Base(Base::Str),
            Expr::Color(_) => Ty::Base(Base::Color),
            Expr::Vec3(parts) => {
                for part in parts.iter() {
                    let ty = self.expr(env, part);
                    if !self.fits(&ty, &Type::Base(Base::Real)) {
                        self.error(
                            "type",
                            "The coordinates of a point must be numbers.".into(),
                            None,
                        );
                    }
                }
                Ty::Base(Base::Vec)
            }
            Expr::Const(name) => match self.sig.consts.get(name) {
                Some(constant) => Ty::Base(constant.ty()),
                None => {
                    self.error(
                        "unknown",
                        format!("There is no constant called {name}."),
                        None,
                    );
                    Ty::Unknown
                }
            },
            Expr::Var(var) => match env.vars.get(var) {
                Some(Entry::Value(base)) => Ty::Base(*base),
                Some(_) => self.expr(env, &Expr::Place(Place::var(*var))),
                None => {
                    let name = self.name(&Place::var(*var));
                    self.error(
                        "unknown",
                        format!("{name} does not exist yet at this point."),
                        None,
                    );
                    Ty::Unknown
                }
            },
            Expr::Place(place) => match self.place(env, place) {
                Some(found) => Ty::Mob(found.kind),
                None => Ty::Unknown,
            },
            Expr::Call { fun, args } => {
                let types: Vec<Ty> = args.iter().map(|arg| self.expr(env, arg)).collect();
                let Some(overloads) = self.sig.fns.get(fun) else {
                    self.error(
                        "unknown",
                        format!("There is no function called {fun}."),
                        None,
                    );
                    return Ty::Unknown;
                };
                let applies = |params: &[Base]| {
                    params.len() == types.len()
                        && types
                            .iter()
                            .zip(params)
                            .all(|(ty, param)| self.fits(ty, &Type::Base(*param)))
                };
                match overloads.iter().find(|overload| applies(&overload.params)) {
                    Some(overload) => Ty::Base(overload.result),
                    None => {
                        let given: Vec<&str> = types.iter().map(plain_ty).collect();
                        self.error(
                            "type",
                            format!("{fun} cannot be applied to {}.", given.join(" and ")),
                            None,
                        );
                        Ty::Unknown
                    }
                }
            }
            Expr::If {
                cond,
                then,
                otherwise,
            } => {
                let cond = self.expr(env, cond);
                if !self.fits(&cond, &Type::Base(Base::Bool)) {
                    self.error("type", "A condition must be yes or no.".into(), None);
                }
                match (self.expr(env, then), self.expr(env, otherwise)) {
                    (Ty::Base(a), Ty::Base(b)) if a.fits(b) => Ty::Base(b),
                    (Ty::Base(a), Ty::Base(b)) if b.fits(a) => Ty::Base(a),
                    (Ty::Unknown, other) | (other, Ty::Unknown) => other,
                    _ => {
                        self.error(
                            "type",
                            "Both outcomes of a choice must be the same sort of value.".into(),
                            None,
                        );
                        Ty::Unknown
                    }
                }
            }
            Expr::Query {
                target,
                query,
                args,
            } => {
                let Some(found) = self.place(env, target) else {
                    return Ty::Unknown;
                };
                let Some(sig) = self.sig.query(&found.kind, query) else {
                    self.error(
                        "unknown",
                        format!("A {} has no property called “{query}”.", found.kind),
                        None,
                    );
                    return Ty::Unknown;
                };
                let params: Vec<&Param> = sig.params.iter().collect();
                self.args(env, query, &params, args);
                Ty::Base(sig.result)
            }
        }
    }

    // ── Places ───────────────────────────────────────────────────────────────

    /// T-Place: the object at `place`, which must exist and not be spent.
    fn place(&mut self, env: &Env, place: &Place) -> Option<Found> {
        match self.lookup(env, place) {
            Ok(found) if found.state == State::Is(Presence::Spent) => {
                let name = self.name(place);
                self.error(
                    "spent",
                    format!("{name} was used up by an earlier animation and cannot be used again."),
                    Some("Make a copy before that animation and use the copy here."),
                );
                None
            }
            Ok(found) => Some(found),
            Err((code, message)) => {
                self.error(code, message, None);
                None
            }
        }
    }

    fn place_quiet(&self, env: &Env, place: &Place) -> Option<Found> {
        self.lookup(env, place).ok()
    }

    fn lookup(&self, env: &Env, place: &Place) -> Result<Found, (&'static str, String)> {
        let name = self.name(place);
        let missing = || {
            (
                "unknown",
                format!("{name} does not exist yet at this point."),
            )
        };
        if matches!(env.vars.get(&place.root), Some(Entry::Value(_))) {
            return Err(("type", format!("{name} is a value, not an object.")));
        }
        let resolved = env.resolve(place).ok_or_else(missing)?;
        let object = env.object(resolved.root).ok_or_else(missing)?;

        let reshaped = |path: &[u32]| object.reshaped.get(path);
        let mut kind = reshaped(&[]).unwrap_or(&object.kind).clone();
        let mut shape = &object.shape;
        for (depth, &index) in resolved.path.iter().enumerate() {
            if reshaped(&resolved.path[..depth]).is_some() {
                let message = format!(
                    "{name} cannot be picked out: an object that was morphed into another shape no longer has separate parts."
                );
                return Err(("sealed", message));
            }
            match shape {
                Shape::Opaque => kind = self.sig.elem(&kind),
                Shape::Group(members) => {
                    let member = members.get(index as usize).ok_or_else(|| {
                        let message = format!(
                            "{name} does not exist: the group has only {} members.",
                            members.len()
                        );
                        ("unknown", message)
                    })?;
                    kind = member.kind.clone();
                }
            }
            shape = shape.child(index).unwrap_or(&Shape::Opaque);
            if let Some(weakened) = reshaped(&resolved.path[..=depth]) {
                kind = weakened.clone();
            }
        }
        let state = object.tree.at(&resolved.path).state(shape);
        Ok(Found {
            place: resolved,
            kind,
            shape: shape.clone(),
            state,
        })
    }
}

/// The places the arguments of a call mention, wherever they are nested.
fn reads_of_args(args: &Args, out: &mut Vec<Place>) {
    for (_, arg) in args {
        match arg {
            Arg::Expr(expr) => reads_of_expr(expr, out),
            Arg::Fun { body, .. } => reads_of_expr(body, out),
            Arg::Object(object) => reads_of_object(object, out),
            Arg::Rate(_) => {}
        }
    }
}

fn reads_of_object(node: &Node<Object>, out: &mut Vec<Place>) {
    match &node.item {
        Object::New { args, .. } => reads_of_args(args, out),
        Object::Group { items, .. } => out.extend(items.iter().cloned()),
        Object::Copy(source) => out.push(source.clone()),
        Object::Chain { base, call } => {
            reads_of_object(base, out);
            reads_of_args(&call.args, out);
        }
    }
}

fn reads_of_expr(expr: &Expr, out: &mut Vec<Place>) {
    match expr {
        Expr::Place(place) => out.push(place.clone()),
        Expr::Var(var) => out.push(Place::var(*var)),
        Expr::Vec3(parts) => parts.iter().for_each(|part| reads_of_expr(part, out)),
        Expr::Call { args, .. } => args.iter().for_each(|arg| reads_of_expr(arg, out)),
        Expr::If {
            cond,
            then,
            otherwise,
        } => [cond, then, otherwise]
            .into_iter()
            .for_each(|part| reads_of_expr(part, out)),
        Expr::Query { target, args, .. } => {
            out.push(target.clone());
            reads_of_args(args, out);
        }
        Expr::Real(_)
        | Expr::Int(_)
        | Expr::Bool(_)
        | Expr::Str(_)
        | Expr::Color(_)
        | Expr::Const(_) => {}
    }
}

/// `Γ·Φ`: the presence each step ensures, and the kinds it weakens.
fn apply(env: &mut Env, effect: &Effect) {
    for step in effect {
        if let Some(presence) = step.role.ensures() {
            env.set(&step.place, presence);
        }
        if let Some(kind) = &step.weakened {
            reshape(env, &step.place, kind.clone());
        }
    }
}

/// The object at `place` took on another object's geometry: it now has the weakened `kind`
/// and its parts can no longer be told apart (spec §5.4).
fn reshape(env: &mut Env, place: &Place, kind: Sym) {
    if let Some(Entry::Object(object)) = env.vars.get_mut(&place.root) {
        object.reshaped.insert(place.path.clone(), kind);
    }
}

/// `Φ # Φ′` fails: a place `a` writes overlaps a place `b` touches.
fn clash<'e>(writes: &'e Effect, touches: &'e Effect) -> Option<(&'e Step, &'e Step)> {
    writes.iter().filter(|a| a.role != Role::Use).find_map(|a| {
        touches
            .iter()
            .find(|b| !a.place.disjoint(&b.place))
            .map(|b| (a, b))
    })
}

fn summary(env: &Env) -> Vec<ObjectState> {
    env.vars
        .iter()
        .filter_map(|(var, entry)| match entry {
            Entry::Object(object) => Some(ObjectState {
                var: *var,
                kind: object.reshaped.get(&[][..]).unwrap_or(&object.kind).clone(),
                state: object.tree.state(&object.shape),
            }),
            _ => None,
        })
        .collect()
}

/// The objects at least partly on scene, for comparing a loop's entry with its exit.
fn on_scene(states: &[ObjectState]) -> Vec<(VarId, State)> {
    states
        .iter()
        .filter(|s| !matches!(s.state, State::Is(Presence::Off | Presence::Spent)))
        .map(|s| (s.var, s.state))
        .collect()
}

fn function_type(expected: &Type) -> Option<(Base, Base)> {
    match expected {
        Type::Fun(input, output) => Some((*input, *output)),
        _ => None,
    }
}

fn presence_problem(
    what: &Sym,
    name: &str,
    role: Role,
    state: State,
) -> (String, Option<&'static str>) {
    match (role, state) {
        (Role::Intro | Role::Flash, State::Is(Presence::On)) => (
            format!("{name} is already on the slide, so {what} would make it vanish and reappear."),
            Some("Remove it first, or pick an animation that changes it in place."),
        ),
        (_, State::Is(Presence::Off)) => (
            format!(
                "{name} is not on the slide yet, so {what} would make it pop in out of nowhere."
            ),
            Some("Bring it in first, for example with FadeIn or Create."),
        ),
        (_, State::Mixed) => (
            format!("Only part of {name} is on the slide."),
            Some("Animate the parts that are on the slide individually."),
        ),
        _ => (format!("{name} cannot be used by {what} here."), None),
    }
}

fn describe(presence: Presence) -> &'static str {
    match presence {
        Presence::On => "on the slide",
        Presence::Off => "off the slide",
        Presence::Spent => "used up",
    }
}

fn plain_base(base: Base) -> &'static str {
    match base {
        Base::Real => "a number",
        Base::Int => "a whole number",
        Base::Bool => "yes or no",
        Base::Str => "text",
        Base::Vec => "a point",
        Base::Color => "a colour",
    }
}

fn plain(ty: &Type) -> String {
    match ty {
        Type::Base(base) => plain_base(*base).to_owned(),
        Type::Mob(kind) if &**kind == ROOT_KIND => "an object".to_owned(),
        Type::Mob(kind) => format!("a {kind}"),
        Type::Fun(..) => "a formula".to_owned(),
        Type::Rate => "an easing".to_owned(),
        Type::Union(options) => options.iter().map(plain).collect::<Vec<_>>().join(" or "),
    }
}

fn plain_ty(ty: &Ty) -> &'static str {
    match ty {
        Ty::Base(base) => plain_base(*base),
        Ty::Mob(_) => "an object",
        Ty::Rate => "an easing",
        Ty::Fun => "a formula",
        Ty::Unknown => "unknown",
    }
}
