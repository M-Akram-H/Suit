//! Every way the deck can change. Each public method here is one undo step.

use std::time::Instant;

use suit_check::check;
use suit_core::{
    Anim, Arg, Blueprint, Call, Command, Expr, Node, NodeId, Object, Place, Slide, SlideConfig,
    SlideContent, SlideId, VarId,
};
use suit_sig::{AnimParamType, Role};

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
    fn settle(&mut self) {
        self.checked = check(&self.deck, &self.sig);
        self.revision += 1;
        self.changed_at = Instant::now();
        self.select(self.slide, self.step);
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
    /// animation that suits it; selects that animation.
    pub fn add_object(&mut self, ctor: &str) -> Option<VarId> {
        self.body()?;
        let kind = self.sig.ctors.get(ctor)?.kind.clone();
        let entrance = ["Write", "Create", "FadeIn"].into_iter().find_map(|name| {
            let anim = self.sig.anims.get(name)?;
            // The parameter through which the animation introduces an object of this kind.
            anim.params.iter().find_map(|param| match &param.ty {
                AnimParamType::Object {
                    role: Role::Intro,
                    kind: wanted,
                } => {
                    let fits =
                        self.sig.is_subkind(&kind, wanted) && (name != "Write" || writes(&kind));
                    fits.then(|| (name, param.name.clone()))
                }
                _ => None,
            })
        });
        let name = self.unused_name(&everyday_name(ctor));
        self.change("Add object", None, |session| {
            let var = session.deck.fresh_var(&name);
            let object = starting_arguments(ctor)
                .into_iter()
                .fold(Object::new(ctor), |object, (name, value)| {
                    object.arg(name, value)
                });
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
        let placed = Object::Chain {
            base: Box::new(object),
            call: move_to(to),
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

    /// Adds a step after the selected one that moves `var` to `to`: gliding there if
    /// `animated`, otherwise jumping.
    pub fn insert_move(&mut self, var: VarId, to: [f64; 2], animated: bool) -> Option<NodeId> {
        self.body()?;
        let target = Place::var(var);
        let command = match animated {
            true => {
                let anim = Anim::animate(&target, vec![move_to(to)]);
                Command::Play {
                    anims: vec![Node::unset(anim)],
                }
            }
            false => Command::Apply {
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
    match ctor {
        "MathTex" => "formula".to_owned(),
        "Tex" | "MarkupText" => "text".to_owned(),
        "NumberLine" => "number line".to_owned(),
        "ImageMobject" => "picture".to_owned(),
        _ => ctor.to_lowercase(),
    }
}

/// Kinds that are written rather than drawn.
fn writes(kind: &str) -> bool {
    matches!(kind, "Text" | "MarkupText" | "MathTex" | "Tex")
}

/// What a freshly added object needs to be given before it can exist.
fn starting_arguments(ctor: &str) -> Vec<(&'static str, Arg)> {
    match ctor {
        "Text" | "MarkupText" => vec![("text", "Text".into())],
        "MathTex" => vec![("tex_strings", "x^2".into())],
        "Tex" => vec![("tex_strings", "Text".into())],
        _ => Vec::new(),
    }
}
