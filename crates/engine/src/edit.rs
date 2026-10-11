//! Every way the deck can change. Each public method here is one undo step.

use std::{collections::BTreeSet, path::Path, time::Instant};

use suit_check::ObjectState;
use suit_check::check;
use suit_core::{
    Anim, Arg, Blueprint, Call, Command, Deck, Expr, Node, NodeId, Object, Place, Slide,
    SlideConfig, SlideContent, SlideId, UNBOUND, VarId,
};
use suit_sig::{AnimParamType, ROOT_KIND, Role, Type, VGROUP_KIND};

use crate::{MERGE_WINDOW, Moment, Session, Target};

/// Methods that only say where an object is. Dragging an object replaces a run of these at
/// the end of how it is made.
const PLACEMENTS: [&str; 8] = [
    "move_to",
    "next_to",
    "shift",
    "to_edge",
    "to_corner",
    "align_to",
    "set_x",
    "set_y",
];

impl Session {
    // ── History ──────────────────────────────────────────────────────────────

    fn moment(&self) -> Moment {
        Moment {
            deck: self.deck.clone(),
            slide: self.slide,
            step: self.step,
        }
    }

    /// Runs `edit` as one undo step called `label`. Changes to the same `target` in quick
    /// succession join the step before them.
    fn change<R>(
        &mut self,
        label: &str,
        target: Option<Target>,
        edit: impl FnOnce(&mut Session) -> R,
    ) -> R {
        let joins = self
            .last_change
            .as_ref()
            .is_some_and(|(last, at)| Some(last) == target.as_ref() && at.elapsed() < MERGE_WINDOW);
        if !joins {
            self.undo.push((label.to_owned(), self.moment()));
        }
        self.redo.clear();
        let result = edit(self);
        self.last_change = target.map(|target| (target, Instant::now()));
        self.settle();
        result
    }

    /// Brings everything derived from the deck up to date after it changed.
    pub(crate) fn settle(&mut self) {
        self.checked = check(&self.deck, &self.sig);
        self.revision += 1;
        self.changed_at = Instant::now();
        self.unsettle();
        // Names may have changed, and what Manim remarked on is told by name.
        self.note_remarks();
        self.select(self.slide, self.step);
    }

    /// Changes the deck as a whole, as one undo step called `label`.
    pub(crate) fn change_deck(&mut self, label: &str, edit: impl FnOnce(&mut Deck)) {
        self.change(label, None, |session| edit(&mut session.deck));
    }

    fn restore(&mut self, moment: Moment) {
        self.deck = moment.deck;
        (self.slide, self.step) = (moment.slide, moment.step);
        self.last_change = None;
        self.settle();
    }

    /// What undoing would take back, if anything.
    pub fn can_undo(&self) -> Option<&str> {
        self.undo.last().map(|(label, _)| label.as_str())
    }

    pub fn can_redo(&self) -> Option<&str> {
        self.redo.last().map(|(label, _)| label.as_str())
    }

    pub fn undo(&mut self) {
        if let Some((label, moment)) = self.undo.pop() {
            self.redo.push((label, self.moment()));
            self.restore(moment);
        }
    }

    pub fn redo(&mut self) {
        if let Some((label, moment)) = self.redo.pop() {
            self.undo.push((label, self.moment()));
            self.restore(moment);
        }
    }

    // ── Slides ───────────────────────────────────────────────────────────────

    /// Adds an empty slide after the selected one and selects it.
    pub fn add_slide(&mut self) {
        self.change("Add slide", None, |session| {
            let id = SlideId(session.deck.fresh_id());
            let at = (session.slide + 1).min(session.deck.slides.len());
            let slide = Slide {
                id,
                title: format!("Slide {}", session.deck.slides.len() + 1),
                config: SlideConfig::default(),
                content: SlideContent::Blueprint(Blueprint::default()),
            };
            session.deck.slides.insert(at, slide);
            (session.slide, session.step) = (at, None);
        });
    }

    /// Deletes the selected slide. A deck keeps at least one.
    pub fn delete_slide(&mut self) {
        if self.deck.slides.len() < 2 {
            return;
        }
        self.change("Delete slide", None, |session| {
            session.deck.slides.remove(session.slide);
            (session.slide, session.step) = (session.slide.saturating_sub(1), None);
        });
    }

    /// Moves the selected slide one place earlier or later.
    pub fn move_slide(&mut self, later: bool) {
        let from = self.slide;
        let Some(to) = neighbour(from, later, self.deck.slides.len()) else {
            return;
        };
        self.change("Move slide", None, |session| {
            session.deck.slides.swap(from, to);
            session.slide = to;
        });
    }

    /// Changes the selected slide's title or settings.
    pub fn edit_slide(&mut self, edit: impl FnOnce(&mut Slide)) {
        let Some(id) = self.deck.slides.get(self.slide).map(|slide| slide.id) else {
            return;
        };
        self.change("Change slide", Some(Target::Slide(id)), |session| {
            edit(&mut session.deck.slides[session.slide]);
        });
    }

    // ── Steps ────────────────────────────────────────────────────────────────

    fn body(&mut self) -> Option<&mut Vec<Node<Command>>> {
        match &mut self.deck.slides.get_mut(self.slide)?.content {
            SlideContent::Blueprint(blueprint) => Some(&mut blueprint.body),
            SlideContent::Media { .. } => None,
        }
    }

    /// Puts `commands` after the selected step, without touching the history.
    fn insert(&mut self, commands: Vec<Command>) -> Option<NodeId> {
        let first = self.step.map_or(0, |step| step + 1);
        let mut last = None;
        for (at, command) in (first..).zip(commands) {
            let node = self.deck.adopt(command);
            last = Some(node.id);
            self.body()?.insert(at, node);
            self.step = Some(at);
        }
        last
    }

    /// Adds a step after the selected one and selects it.
    pub fn insert_step(&mut self, command: Command) -> Option<NodeId> {
        self.body()?;
        self.change("Add step", None, |session| session.insert(vec![command]))
    }

    pub fn delete_step(&mut self) {
        let Some(step) = self.step else { return };
        self.change("Delete step", None, |session| {
            if let Some(body) = session.body() {
                body.remove(step);
            }
            session.step = step.checked_sub(1);
        });
    }

    /// Moves the selected step one place earlier or later within its slide.
    pub fn move_step(&mut self, later: bool) {
        let Some(from) = self.step else { return };
        let Some(to) = neighbour(from, later, self.steps().len()) else {
            return;
        };
        self.change("Move step", None, |session| {
            if let Some(body) = session.body() {
                body.swap(from, to);
            }
            session.step = Some(to);
        });
    }

    /// Replaces what the step `id` does, keeping its identity and its place.
    pub fn replace_step(&mut self, id: NodeId, command: Command) {
        let Some(&(slide, step)) = self.locations().get(&id) else {
            return;
        };
        self.change("Change step", Some(Target::Step(id)), |session| {
            let mut node = Node { id, item: command };
            session.deck.assign_ids(&mut node);
            if let SlideContent::Blueprint(blueprint) = &mut session.deck.slides[slide].content {
                blueprint.body[step] = node;
            }
        });
    }

    pub fn rename(&mut self, var: VarId, name: &str) {
        self.change("Rename", Some(Target::Name(var)), |session| {
            session.deck.names.insert(var, name.to_owned());
        });
    }

    // ── Objects ──────────────────────────────────────────────────────────────

    /// A name nothing else has: `circle`, then `circle 2`, `circle 3`, …
    fn unused_name(&self, base: &str) -> String {
        let taken = |name: &str| self.deck.names.values().any(|used| used == name);
        std::iter::once(base.to_owned())
            .chain((2..).map(|n| format!("{base} {n}")))
            .find(|name| !taken(name))
            .expect("there are always more numbers")
    }

    /// Makes a new object after the selected step and brings it onto the slide with the
    /// animation that suits it; selects that animation. Some objects are made around
    /// another one, a brace for instance: that is `about`, or failing that the newest
    /// object there is. `None` if such an object is needed and there is none.
    pub fn add_object(&mut self, ctor: &str) -> Option<VarId> {
        self.add_object_about(ctor, None)
    }

    pub fn add_object_about(&mut self, ctor: &str, about: Option<VarId>) -> Option<VarId> {
        let made = self.sig.ctors.get(ctor)?;
        let kind = made.kind.clone();
        // What it is made around has to be of the right kind: a tangent wants a graph.
        let wanted = made.params.iter().find_map(|param| match &param.ty {
            Type::Mob(kind) => Some(kind.clone()),
            _ => None,
        });
        let fits = |object: &&ObjectState| {
            let kind = wanted.as_deref().unwrap_or(ROOT_KIND);
            self.sig.is_subkind(&object.kind, kind)
        };
        let objects = self.objects();
        let chosen = objects
            .iter()
            .filter(fits)
            .find(|object| Some(object.var) == about);
        let about = chosen
            .or(objects.iter().rev().find(fits))
            .map(|object| object.var);
        let mut args = starting_arguments(ctor, about.map(Place::var).as_ref())?;
        if let (true, Some(graph)) = (ctor == "Tangent", about) {
            // In the middle of the longest stretch the graph is drawn on, if that is
            // known; a tangent put where there is no graph touches nothing.
            let longest = self.found_by_graph(graph).and_then(|found| {
                let length = |piece: &&[f64; 2]| piece[1] - piece[0];
                found
                    .pieces
                    .iter()
                    .max_by(|a, b| length(a).total_cmp(&length(b)))
            });
            let middle = longest.map_or(1.0, |[from, to]| ((from + to) * 5.0).round() / 10.0);
            args.push(("at", middle.into()));
        }
        self.add_made(&everyday_name(ctor), &kind, |_| {
            args.into_iter()
                .fold(Object::new(ctor), |object, (name, value)| {
                    object.arg(name, value)
                })
        })
    }

    /// Puts the picture in `file` on the slide, four units high.
    pub fn add_picture(&mut self, file: &Path) -> Option<VarId> {
        let file = self.name_of_picture(file);
        self.add_made("picture", "ImageMobject", |_| {
            Object::new("ImageMobject")
                .arg("filename_or_array", file.as_str())
                .then(Call::new("scale_to_fit_height").arg("height", 4.0))
        })
    }

    /// Makes a copy of `var` next to it and brings the copy onto the slide.
    pub fn duplicate(&mut self, var: VarId) -> Option<VarId> {
        let source = self.objects().iter().find(|object| object.var == var)?;
        let kind = source.kind.clone();
        let name = format!("{} copy", self.name(var));
        let aside = Call::new("shift").arg("vectors", Expr::vec3(0.5, -0.5, 0.0));
        self.add_made(&name, &kind, |_| Object::copy(&Place::var(var)).then(aside))
    }

    /// Makes a group of `members`, which from then on can be moved and animated as one.
    /// The members keep their names. Selects the step that makes the group.
    pub fn add_group(&mut self, members: &[VarId]) -> Option<VarId> {
        self.body()?;
        if members.is_empty() {
            return None;
        }
        // A group of shapes can do more than a group of anything, so it is one if it can be.
        let shape = self.sig.elem(VGROUP_KIND);
        let kind_of = |var: &VarId| self.objects().iter().find(|object| object.var == *var);
        let vector = members.iter().all(|var| {
            kind_of(var).is_some_and(|object| self.sig.is_subkind(&object.kind, &shape))
        });
        let items = members.iter().map(|var| Place::var(*var)).collect();
        let name = self.unused_name("group");
        self.change("Group", None, |session| {
            let var = session.deck.fresh_var(&name);
            let value = Node::unset(Object::Group { vector, items });
            session.insert(vec![Command::LetObject { var, value }]);
            Some(var)
        })
    }

    /// The objects that are members of a group at the selected point, and so cannot go
    /// into another.
    pub fn grouped(&self) -> BTreeSet<VarId> {
        fn members(object: &Object) -> &[Place] {
            match object {
                Object::Group { items, .. } => items,
                Object::Chain { base, .. } => members(&base.item),
                _ => &[],
            }
        }
        let mut grouped = BTreeSet::new();
        for node in self.steps_until_selection(true) {
            if let Command::LetObject { value, .. } = &node.item {
                grouped.extend(members(&value.item).iter().map(|place| place.root));
            }
        }
        grouped
    }

    /// Adds a named value after the selected step: a number other steps can use by name.
    pub fn add_value(&mut self) -> Option<VarId> {
        self.body()?;
        let name = self.unused_name("value");
        self.change("Add value", None, |session| {
            let var = session.deck.fresh_var(&name);
            let value = Expr::Real(1.0);
            session.insert(vec![Command::LetValue { var, value }]);
            Some(var)
        })
    }

    /// The named values that can be read by the selected step: those made before it.
    pub fn values_before(&self) -> Vec<VarId> {
        self.values_before_step(self.slide, self.step)
    }

    /// The named values made before step `step` of slide `slide`.
    pub fn values_before_step(&self, slide: usize, step: Option<usize>) -> Vec<VarId> {
        let made = |node: &Node<Command>| match &node.item {
            Command::LetValue { var, .. } => Some(*var),
            _ => None,
        };
        self.steps_until(slide, step.unwrap_or(0))
            .filter_map(made)
            .collect()
    }

    /// The steps of the deck from its start up to the selected one, with or without it.
    fn steps_until_selection(&self, inclusive: bool) -> impl Iterator<Item = &Node<Command>> {
        let count = match (self.step, inclusive) {
            (Some(step), true) => step + 1,
            (Some(step), false) => step,
            (None, _) => 0,
        };
        self.steps_until(self.slide, count)
    }

    /// The steps of the deck from its start up to the first `count` of slide `slide`.
    fn steps_until(&self, slide: usize, count: usize) -> impl Iterator<Item = &Node<Command>> {
        let earlier = (0..slide).flat_map(|slide| self.steps_of(slide));
        earlier.chain(self.steps_of(slide).iter().take(count))
    }

    /// Names what `make` builds, which is of `kind`, and brings it onto the slide.
    fn add_made(
        &mut self,
        name: &str,
        kind: &str,
        make: impl FnOnce(&mut Deck) -> Object,
    ) -> Option<VarId> {
        self.body()?;
        let entrance = ["Write", "Create", "FadeIn"].into_iter().find_map(|name| {
            let anim = self.sig.anims.get(name)?;
            // The parameter through which the animation introduces an object of this kind.
            anim.params.iter().find_map(|param| match &param.ty {
                AnimParamType::Object {
                    role: Role::Intro,
                    kind: wanted,
                } => {
                    let fits =
                        self.sig.is_subkind(kind, wanted) && (name != "Write" || writes(kind));
                    fits.then(|| (name, param.name.clone()))
                }
                _ => None,
            })
        });
        // What cannot be seen is not brought in with a flourish.
        let entrance = entrance.filter(|_| kind != "ValueTracker");
        let name = self.unused_name(name);
        self.change("Add object", None, |session| {
            let object = make(&mut session.deck);
            let var = session.deck.fresh_var(&name);
            let mut commands = vec![Command::LetObject {
                var,
                value: Node::unset(object),
            }];
            let place = Place::var(var);
            commands.push(match entrance {
                Some((anim, param)) => {
                    let anim = Anim::prim(anim).arg(&param, &place);
                    Command::Play {
                        anims: vec![Node::unset(anim)],
                    }
                }
                None => Command::Add {
                    targets: vec![place],
                },
            });
            session.insert(commands);
            Some(var)
        })
    }

    /// Puts the object where it is made at `to`, by replacing whatever placed it there
    /// before. Returns false if `var` is not made by a constructor.
    pub fn place_at_creation(&mut self, var: VarId, to: [f64; 2]) -> bool {
        let Some((slide, step)) = self.creation(var) else {
            return false;
        };
        let node = &self.steps_of(slide)[step];
        let Command::LetObject { value, .. } = &node.item else {
            return false;
        };
        let id = node.id;

        // Peel the placement calls off the end, keep the rest.
        let mut object = value.clone();
        loop {
            let inner = match &object.item {
                Object::Chain { base, call } if PLACEMENTS.contains(&call.method.as_str()) => {
                    (**base).clone()
                }
                _ => break,
            };
            object = inner;
        }
        // A tangent is put somewhere by saying where along its graph it touches.
        let placed = match &mut object.item {
            Object::New { ctor, args } if &**ctor == "Tangent" => {
                let x = (to[0] * 100.0).round() / 100.0;
                args.retain(|(name, _)| &**name != "at");
                args.push(("at".into(), x.into()));
                object.item
            }
            _ => Object::Chain {
                base: Box::new(object),
                call: move_to(to),
            },
        };
        let command = Command::LetObject {
            var,
            value: Node::unset(placed),
        };
        self.change("Move object", Some(Target::Step(id)), |session| {
            let mut node = Node { id, item: command };
            session.deck.assign_ids(&mut node);
            if let SlideContent::Blueprint(blueprint) = &mut session.deck.slides[slide].content {
                blueprint.body[step] = node;
            }
        });
        true
    }

    /// The kind of the object `var` at the selected point, if it exists there.
    fn kind_of(&self, var: VarId) -> Option<&str> {
        let object = self.objects().iter().find(|object| object.var == var)?;
        Some(object.kind.as_str())
    }

    /// Adds a step after the selected one that moves `var` to `to`: gliding there if
    /// `animated`, otherwise jumping.
    pub fn insert_move(&mut self, var: VarId, to: [f64; 2], animated: bool) -> Option<NodeId> {
        self.body()?;
        let target = Place::var(var);
        // A tangent does not go to a point: it goes along its graph, to an x.
        let tangent = self
            .kind_of(var)
            .is_some_and(|kind| self.sig.is_subkind(kind, "Tangent"));
        let x = (to[0] * 100.0).round() / 100.0;
        let command = match (animated, tangent) {
            (true, true) => {
                let glide = Anim::prim("Glide").arg("mobject", &target).arg("to", x);
                Command::Play {
                    anims: vec![Node::unset(glide)],
                }
            }
            (false, true) => Command::Apply {
                target,
                call: Call::new("touch").arg("x", x),
            },
            (true, false) => {
                let anim = Anim::animate(&target, vec![move_to(to)]);
                Command::Play {
                    anims: vec![Node::unset(anim)],
                }
            }
            (false, false) => Command::Apply {
                target,
                call: move_to(to),
            },
        };
        self.change("Move object", None, |session| session.insert(vec![command]))
    }
}

fn neighbour(index: usize, later: bool, count: usize) -> Option<usize> {
    match later {
        true => Some(index + 1).filter(|&next| next < count),
        false => index.checked_sub(1),
    }
}

fn move_to([x, y]: [f64; 2]) -> Call {
    // Whole hundredths: nobody places things more finely by hand, and the numbers stay readable.
    let round = |value: f64| (value * 100.0).round() / 100.0;
    Call::new("move_to").arg("point_or_mobject", Expr::vec3(round(x), round(y), 0.0))
}

/// What a new object of this kind is called until someone names it.
fn everyday_name(ctor: &str) -> String {
    let name = match ctor {
        "MathTex" => "formula",
        "Tex" | "MarkupText" => "text",
        "NumberLine" => "number line",
        "NumberPlane" => "grid",
        "ImageMobject" => "picture",
        "RoundedRectangle" => "rounded rectangle",
        "RegularPolygon" | "Polygon" => "polygon",
        "DashedLine" => "dashed line",
        "DoubleArrow" => "double arrow",
        "DecimalNumber" | "Integer" => "number",
        "FunctionGraph" | "Plot" => "graph",
        "Tangent" => "tangent",
        "ParametricFunction" => "curve",
        "ValueTracker" => "tracker",
        "SurroundingRectangle" => "frame",
        _ => return ctor.to_lowercase(),
    };
    name.to_owned()
}

/// Kinds that are written rather than drawn.
fn writes(kind: &str) -> bool {
    matches!(
        kind,
        "Text" | "MarkupText" | "MathTex" | "Tex" | "DecimalNumber" | "Integer"
    )
}

/// What a freshly added object is given to start with: what it cannot exist without, and
/// where the bare object would be too small or too plain to see what it is. `about` is the
/// object it is made around, for those that are. `None` if it needs one and there is none.
fn starting_arguments(ctor: &str, about: Option<&Place>) -> Option<Vec<(&'static str, Arg)>> {
    let point = |x: f64, y: f64| Arg::from(Expr::vec3(x, y, 0.0));
    let unbound = || Expr::Var(UNBOUND);
    let call = |fun: &str| Expr::call(fun, vec![unbound()]);
    Some(match ctor {
        "Text" => vec![("text", "Text".into())],
        "MarkupText" => vec![("text", "<b>Bold</b> and <i>slanted</i>".into())],
        "MathTex" => vec![("tex_strings", "x^2".into())],
        "Tex" => vec![("tex_strings", "Text".into())],
        "Ellipse" => vec![("width", 3.0.into()), ("height", 1.5.into())],
        "RoundedRectangle" => vec![
            ("corner_radius", 0.3.into()),
            ("width", 3.0.into()),
            ("height", 1.6.into()),
        ],
        "RegularPolygon" => vec![("n", 6_i64.into())],
        "Polygon" => vec![
            ("vertices", point(-1.5, -1.0)),
            ("vertices", point(1.5, -1.0)),
            ("vertices", point(2.0, 1.0)),
            ("vertices", point(-1.0, 1.0)),
        ],
        "DecimalNumber" => vec![("number", 1.5.into())],
        "Integer" => vec![("number", 42.0.into())],
        "ValueTracker" => vec![("value", 0.0.into())],
        // A picture is of a file, which has to be chosen: see `add_picture`.
        "ImageMobject" => return None,
        "Brace" => vec![("mobject", about?.into())],
        "SurroundingRectangle" => vec![("mobjects", about?.into())],
        "Plot" => vec![("function", Arg::fun(UNBOUND, call("sin")))],
        // Where it touches is added by the caller, who knows where the graph is drawn.
        "Tangent" => vec![("graph", about?.into()), ("dot", true.into())],
        // A wave, and a circle: something to see, and to change into what is wanted.
        "FunctionGraph" => vec![
            ("function", Arg::fun(UNBOUND, call("sin"))),
            ("x_range", Expr::vec3(-7.0, 7.0, 0.05).into()),
        ],
        "ParametricFunction" => {
            let twice = |fun: &str| Expr::call("*", vec![Expr::Real(2.0), call(fun)]);
            let circle = Expr::Vec3(Box::new([twice("cos"), twice("sin"), Expr::Real(0.0)]));
            let whole_turn = [Expr::Real(0.0), Expr::konst("TAU"), Expr::Real(0.05)];
            vec![
                ("function", Arg::fun(UNBOUND, circle)),
                ("t_range", Expr::Vec3(Box::new(whole_turn)).into()),
            ]
        }
        _ => Vec::new(),
    })
}
