//! Constructing decks in code: used by tests, samples and, later, editor commands.
//!
//! Trees are built with placeholder node ids; inserting one into a deck gives every node
//! in it a fresh id.

use crate::{
    Anim, Arg, Args, Blueprint, Call, Command, Deck, DeckConfig, Expr, Node, NodeId, Object, Opts,
    Place, Rgba, Slide, SlideConfig, SlideContent, SlideId, Sym, Time, Updater, VarId,
};

const UNSET: NodeId = NodeId(0);

impl<T> Node<T> {
    pub fn unset(item: T) -> Node<T> {
        Node { id: UNSET, item }
    }
}

impl Deck {
    pub fn new(config: DeckConfig) -> Deck {
        Deck {
            config,
            slides: Vec::new(),
            names: Default::default(),
            next_id: 0,
        }
    }

    pub fn fresh_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn fresh_var(&mut self, name: &str) -> VarId {
        let var = VarId(self.fresh_id());
        self.names.insert(var, name.to_owned());
        var
    }

    /// Appends an empty slide and returns a handle for filling it.
    pub fn add_slide(&mut self, title: &str) -> SlideBuilder<'_> {
        let id = SlideId(self.fresh_id());
        self.slides.push(Slide {
            id,
            title: title.to_owned(),
            config: SlideConfig::default(),
            content: SlideContent::Blueprint(Blueprint::default()),
        });
        let index = self.slides.len() - 1;
        SlideBuilder { deck: self, index }
    }

    /// Gives `command` and everything inside it fresh node ids.
    pub fn adopt(&mut self, command: Command) -> Node<Command> {
        let mut node = Node::unset(command);
        node.id = NodeId(self.fresh_id());
        let mut fresh = || NodeId(self.fresh_id());
        renumber_command(&mut node.item, &mut fresh);
        node
    }
}

pub struct SlideBuilder<'a> {
    deck: &'a mut Deck,
    index: usize,
}

impl SlideBuilder<'_> {
    pub fn config(&mut self, edit: impl FnOnce(&mut SlideConfig)) -> &mut Self {
        edit(&mut self.deck.slides[self.index].config);
        self
    }

    pub fn push(&mut self, command: Command) -> NodeId {
        let node = self.deck.adopt(command);
        let id = node.id;
        match &mut self.deck.slides[self.index].content {
            SlideContent::Blueprint(blueprint) => blueprint.body.push(node),
            SlideContent::Media { .. } => panic!("a media slide has no blueprint"),
        }
        id
    }

    /// `let name = object`
    pub fn object(&mut self, name: &str, object: Object) -> Place {
        let var = self.deck.fresh_var(name);
        self.push(Command::LetObject {
            var,
            value: Node::unset(object),
        });
        Place::var(var)
    }

    /// A fresh variable that no command binds: the parameter of a formula.
    pub fn var(&mut self, name: &str) -> VarId {
        self.deck.fresh_var(name)
    }

    /// `let name = expr`
    pub fn value(&mut self, name: &str, value: Expr) -> VarId {
        let var = self.deck.fresh_var(name);
        self.push(Command::LetValue { var, value });
        var
    }

    pub fn apply(&mut self, target: &Place, call: Call) -> &mut Self {
        self.push(Command::Apply {
            target: target.clone(),
            call,
        });
        self
    }

    pub fn add(&mut self, targets: &[&Place]) -> &mut Self {
        self.push(Command::Add {
            targets: targets.iter().map(|&p| p.clone()).collect(),
        });
        self
    }

    pub fn remove(&mut self, targets: &[&Place]) -> &mut Self {
        self.push(Command::Remove {
            targets: targets.iter().map(|&p| p.clone()).collect(),
        });
        self
    }

    pub fn play(&mut self, anims: Vec<Anim>) -> &mut Self {
        self.push(Command::Play {
            anims: anims.into_iter().map(Node::unset).collect(),
        });
        self
    }

    pub fn wait(&mut self, secs: f64) -> &mut Self {
        self.push(Command::Wait {
            duration: Time::from_secs(secs),
        });
        self
    }

    /// `attach(target, always call)`
    pub fn always(&mut self, target: &Place, call: Call) -> &mut Self {
        self.push(Command::Attach {
            target: target.clone(),
            updater: Updater::Always(call),
        });
        self
    }

    /// `attach(target, every dt. call)`; `call` receives the variable bound to `dt`.
    pub fn every(&mut self, target: &Place, call: impl FnOnce(VarId) -> Call) -> &mut Self {
        let dt = self.deck.fresh_var("dt");
        let updater = Updater::Every { dt, call: call(dt) };
        self.push(Command::Attach {
            target: target.clone(),
            updater,
        });
        self
    }

    pub fn detach(&mut self, target: &Place) -> &mut Self {
        self.push(Command::Detach {
            target: target.clone(),
        });
        self
    }
}

// ── Fresh ids ────────────────────────────────────────────────────────────────

fn renumber_command(command: &mut Command, fresh: &mut impl FnMut() -> NodeId) {
    match command {
        Command::LetObject { value, .. } => renumber_object(value, fresh),
        Command::Apply { call, .. } => renumber_args(&mut call.args, fresh),
        Command::Play { anims } => anims.iter_mut().for_each(|a| renumber_anim(a, fresh)),
        Command::Attach { updater, .. } => match updater {
            Updater::Always(call) | Updater::Every { call, .. } => {
                renumber_args(&mut call.args, fresh)
            }
        },
        Command::LetValue { .. }
        | Command::Add { .. }
        | Command::Remove { .. }
        | Command::Wait { .. }
        | Command::Detach { .. }
        | Command::Raw(_) => {}
    }
}

fn renumber_object(node: &mut Node<Object>, fresh: &mut impl FnMut() -> NodeId) {
    node.id = fresh();
    match &mut node.item {
        Object::New { args, .. } => renumber_args(args, fresh),
        Object::Chain { base, call } => {
            renumber_object(base, fresh);
            renumber_args(&mut call.args, fresh);
        }
        Object::Group { .. } | Object::Copy(_) => {}
    }
}

fn renumber_args(args: &mut Args, fresh: &mut impl FnMut() -> NodeId) {
    for (_, arg) in args {
        if let Arg::Object(object) = arg {
            renumber_object(object, fresh);
        }
    }
}

fn renumber_anim(node: &mut Node<Anim>, fresh: &mut impl FnMut() -> NodeId) {
    node.id = fresh();
    match &mut node.item {
        Anim::Prim { args, .. } => renumber_args(args, fresh),
        Anim::Animate { calls, .. } => calls
            .iter_mut()
            .for_each(|call| renumber_args(&mut call.args, fresh)),
        Anim::Par { items, .. } | Anim::Seq { items, .. } => {
            items.iter_mut().for_each(|item| renumber_anim(item, fresh))
        }
    }
}

// ── Tree shorthand ───────────────────────────────────────────────────────────

impl Object {
    /// `C()`; add arguments with [`Object::arg`].
    pub fn new(ctor: &str) -> Object {
        Object::New {
            ctor: ctor.into(),
            args: Vec::new(),
        }
    }

    pub fn group(items: &[&Place]) -> Object {
        Object::Group {
            vector: false,
            items: items.iter().map(|&p| p.clone()).collect(),
        }
    }

    pub fn vgroup(items: &[&Place]) -> Object {
        Object::Group {
            vector: true,
            items: items.iter().map(|&p| p.clone()).collect(),
        }
    }

    pub fn copy(source: &Place) -> Object {
        Object::Copy(source.clone())
    }

    /// Adds a constructor argument. Only meaningful on [`Object::New`].
    pub fn arg(mut self, name: &str, value: impl Into<Arg>) -> Object {
        match &mut self {
            Object::New { args, .. } => args.push((name.into(), value.into())),
            _ => panic!("only a constructor takes arguments"),
        }
        self
    }

    /// `self.call`
    pub fn then(self, call: Call) -> Object {
        Object::Chain {
            base: Box::new(Node::unset(self)),
            call,
        }
    }
}

impl Call {
    pub fn new(method: &str) -> Call {
        Call {
            method: method.into(),
            args: Vec::new(),
        }
    }

    pub fn arg(mut self, name: &str, value: impl Into<Arg>) -> Call {
        self.args.push((name.into(), value.into()));
        self
    }
}

impl Anim {
    pub fn prim(ctor: &str) -> Anim {
        Anim::Prim {
            ctor: ctor.into(),
            args: Vec::new(),
            opts: Opts::default(),
        }
    }

    pub fn animate(target: &Place, calls: Vec<Call>) -> Anim {
        Anim::Animate {
            target: target.clone(),
            calls,
            opts: Opts::default(),
        }
    }

    pub fn par(items: Vec<Anim>) -> Anim {
        Anim::Par {
            items: items.into_iter().map(Node::unset).collect(),
            opts: Opts::default(),
        }
    }

    pub fn seq(items: Vec<Anim>) -> Anim {
        Anim::Seq {
            items: items.into_iter().map(Node::unset).collect(),
            opts: Opts::default(),
        }
    }

    /// Adds an argument. Only meaningful on [`Anim::Prim`].
    pub fn arg(mut self, name: &str, value: impl Into<Arg>) -> Anim {
        match &mut self {
            Anim::Prim { args, .. } => args.push((name.into(), value.into())),
            _ => panic!("only a primitive animation takes arguments"),
        }
        self
    }

    pub fn run_time(mut self, secs: f64) -> Anim {
        self.opts_mut().run_time = Some(Time::from_secs(secs));
        self
    }

    pub fn rate(mut self, rate: &str) -> Anim {
        self.opts_mut().rate = Some(rate.into());
        self
    }

    pub fn lag(mut self, lag: f64) -> Anim {
        self.opts_mut().lag = Some(lag);
        self
    }
}

impl Expr {
    pub fn vec3(x: f64, y: f64, z: f64) -> Expr {
        Expr::Vec3(Box::new([Expr::Real(x), Expr::Real(y), Expr::Real(z)]))
    }

    pub fn konst(name: &str) -> Expr {
        Expr::Const(name.into())
    }

    pub fn call(fun: &str, args: Vec<Expr>) -> Expr {
        Expr::Call {
            fun: fun.into(),
            args,
        }
    }

    pub fn query(target: &Place, query: &str) -> Expr {
        Expr::Query {
            target: target.clone(),
            query: query.into(),
            args: Vec::new(),
        }
    }
}

impl From<f64> for Expr {
    fn from(value: f64) -> Expr {
        Expr::Real(value)
    }
}

impl From<i64> for Expr {
    fn from(value: i64) -> Expr {
        Expr::Int(value)
    }
}

impl From<bool> for Expr {
    fn from(value: bool) -> Expr {
        Expr::Bool(value)
    }
}

impl From<&str> for Expr {
    fn from(value: &str) -> Expr {
        Expr::Str(value.to_owned())
    }
}

impl From<Rgba> for Expr {
    fn from(value: Rgba) -> Expr {
        Expr::Color(value)
    }
}

impl From<&Place> for Expr {
    fn from(place: &Place) -> Expr {
        Expr::Place(place.clone())
    }
}

impl<T: Into<Expr>> From<T> for Arg {
    fn from(value: T) -> Arg {
        Arg::Expr(value.into())
    }
}

impl From<Object> for Arg {
    fn from(object: Object) -> Arg {
        Arg::Object(Box::new(Node::unset(object)))
    }
}

impl Arg {
    pub fn rate(name: &str) -> Arg {
        Arg::Rate(Sym::from(name))
    }

    /// `λparam. body`
    pub fn fun(param: VarId, body: Expr) -> Arg {
        Arg::Fun {
            param,
            body: Box::new(body),
        }
    }
}
