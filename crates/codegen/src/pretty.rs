//! Manim Core written out for people: a one-line headline per step for lists, and the
//! full text of a step in the notation of the spec (§3).

use suit_core::{
    Anim, Arg, Args, Call, Command, Deck, Expr, Node, Object, Opts, Place, Sym, Updater, VarId,
};
use suit_sig::{Pass, Signature};

use crate::formula::{self, Names};

pub struct Pretty<'a> {
    pub deck: &'a Deck,
    pub sig: &'a Signature,
}

impl Pretty<'_> {
    pub fn var(&self, var: VarId) -> &str {
        self.deck.names.get(&var).map_or("?", String::as_str)
    }

    pub fn place(&self, place: &Place) -> String {
        let path: String = place
            .path
            .iter()
            .map(|index| format!("[{index}]"))
            .collect();
        format!("{}{path}", self.var(place.root))
    }

    fn places(&self, places: &[Place]) -> String {
        places
            .iter()
            .map(|place| self.place(place))
            .collect::<Vec<_>>()
            .join(", ")
    }

    // ── Headlines ────────────────────────────────────────────────────────────

    /// A few words saying what the step does, for step lists.
    pub fn headline(&self, command: &Command) -> String {
        match command {
            Command::LetObject { var, value } => {
                format!("New {} ({})", self.var(*var), object_kind(&value.item))
            }
            Command::LetValue { var, .. } => format!("Value {}", self.var(*var)),
            Command::Apply { target, call } => {
                format!("Change {}: {}", self.place(target), call.method)
            }
            Command::Add { targets } => format!("Show {}", self.places(targets)),
            Command::Remove { targets } => format!("Hide {}", self.places(targets)),
            Command::Play { anims } => {
                let heads: Vec<String> = anims
                    .iter()
                    .map(|anim| self.anim_headline(&anim.item))
                    .collect();
                heads.join(" + ")
            }
            Command::Wait { duration } => format!("Pause {}", seconds(duration.secs())),
            Command::Attach { target, updater } => {
                format!(
                    "{} keeps doing: {}",
                    self.place(target),
                    updater.call().method
                )
            }
            Command::Detach { target } => format!("{} stops updating", self.place(target)),
            Command::Raw(_) => "Python code".to_owned(),
        }
    }

    fn anim_headline(&self, anim: &Anim) -> String {
        match anim {
            Anim::Prim { ctor, args, .. } => {
                let objects: Vec<String> = args
                    .iter()
                    .filter_map(|(_, arg)| match arg {
                        Arg::Expr(Expr::Place(place)) => Some(self.place(place)),
                        Arg::Expr(Expr::Var(var)) => Some(self.var(*var).to_owned()),
                        _ => None,
                    })
                    .collect();
                format!("{ctor} {}", objects.join(" → "))
                    .trim_end()
                    .to_owned()
            }
            Anim::Animate { target, calls, .. } => {
                let methods: Vec<&str> = calls.iter().map(|call| call.method.as_str()).collect();
                format!("Change {}: {}", self.place(target), methods.join(", "))
            }
            Anim::Par { items, .. } => match shared_ctor(items) {
                Some(ctor) => format!("{ctor} × {}", items.len()),
                None => format!("{} together", items.len()),
            },
            Anim::Seq { items, .. } => format!("{} in a row", items.len()),
        }
    }

    // ── Full text ────────────────────────────────────────────────────────────

    pub fn command(&self, command: &Command) -> String {
        match command {
            Command::LetObject { var, value } => {
                format!("{} = {}", self.var(*var), self.object(&value.item))
            }
            Command::LetValue { var, value } => {
                format!("{} = {}", self.var(*var), self.expr(value))
            }
            Command::Apply { target, call } => format!("{}{}", self.place(target), self.call(call)),
            Command::Add { targets } => format!("add({})", self.places(targets)),
            Command::Remove { targets } => format!("remove({})", self.places(targets)),
            Command::Play { anims } => {
                let anims: Vec<String> =
                    anims.iter().map(|anim| self.anim(&anim.item, 1)).collect();
                match anims.as_slice() {
                    [only] => format!("play({only})"),
                    several => format!("play(\n  {}\n)", several.join(",\n  ")),
                }
            }
            Command::Wait { duration } => format!("wait({})", seconds(duration.secs())),
            Command::Attach { target, updater } => match updater {
                Updater::Always(call) => {
                    format!("always {}{}", self.place(target), self.call(call))
                }
                Updater::Every { dt, call } => {
                    format!(
                        "every {}. {}{}",
                        self.var(*dt),
                        self.place(target),
                        self.call(call)
                    )
                }
            },
            Command::Detach { target } => format!("detach({})", self.place(target)),
            Command::Raw(raw) => raw.python.clone(),
        }
    }

    pub fn object(&self, object: &Object) -> String {
        match object {
            Object::New { ctor, args } => {
                let passes = self.sig.ctors.get(ctor).map(|sig| {
                    self.sig
                        .ctor_params(sig)
                        .into_iter()
                        .map(|p| (p.name.clone(), p.pass))
                        .collect()
                });
                format!("{ctor}({})", self.args(args, passes))
            }
            Object::Group {
                vector: true,
                items,
            } => format!("vgroup({})", self.places(items)),
            Object::Group {
                vector: false,
                items,
            } => format!("group({})", self.places(items)),
            Object::Copy(source) => format!("copy({})", self.place(source)),
            Object::Chain { base, call } => {
                format!("{}{}", self.object(&base.item), self.call(call))
            }
        }
    }

    /// `.method(args)`
    fn call(&self, call: &Call) -> String {
        let passes = self
            .sig
            .methods
            .get(&call.method)
            .and_then(|overloads| overloads.first())
            .map(|method| {
                method
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), p.pass))
                    .collect()
            });
        format!(".{}({})", call.method, self.args(&call.args, passes))
    }

    fn anim(&self, anim: &Anim, depth: usize) -> String {
        let inner = "  ".repeat(depth + 1);
        let outer = "  ".repeat(depth);
        let nested = |name: &str, items: &[Node<Anim>]| {
            let items: Vec<String> = items
                .iter()
                .map(|item| self.anim(&item.item, depth + 1))
                .collect();
            format!(
                "{name}(\n{inner}{}\n{outer})",
                items.join(&format!(",\n{inner}"))
            )
        };
        let body = match anim {
            Anim::Prim { ctor, args, .. } => {
                let passes = self.sig.anims.get(ctor).map(|sig| {
                    sig.params
                        .iter()
                        .map(|p| (p.name.clone(), p.pass))
                        .collect()
                });
                format!("{ctor}({})", self.args(args, passes))
            }
            Anim::Animate { target, calls, .. } => {
                let calls: String = calls.iter().map(|call| self.call(call)).collect();
                format!("{}.animate{calls}", self.place(target))
            }
            Anim::Par { items, .. } => nested("par", items),
            Anim::Seq { items, .. } => nested("seq", items),
        };
        format!("{body}{}", self.opts(anim.opts()))
    }

    fn opts(&self, opts: &Opts) -> String {
        let mut parts = Vec::new();
        if let Some(run_time) = opts.run_time {
            parts.push(format!("run_time: {}", seconds(run_time.secs())));
        }
        if let Some(rate) = &opts.rate {
            parts.push(format!("rate: {rate}"));
        }
        if let Some(lag) = opts.lag {
            parts.push(format!("lag: {}", number(lag)));
        }
        match parts.is_empty() {
            true => String::new(),
            false => format!(" @ {{{}}}", parts.join(", ")),
        }
    }

    /// Positional arguments bare, the rest as `name = value`.
    fn args(&self, args: &Args, passes: Option<Vec<(Sym, Pass)>>) -> String {
        let bare = |name: &Sym| {
            passes.as_ref().is_some_and(|passes| {
                passes.iter().any(|(param, pass)| {
                    param == name && !matches!(pass, Pass::Keyword | Pass::List)
                })
            })
        };
        let parts: Vec<String> = args
            .iter()
            .map(|(name, arg)| match bare(name) {
                true => self.arg(arg),
                false => format!("{name} = {}", self.arg(arg)),
            })
            .collect();
        parts.join(", ")
    }

    fn arg(&self, arg: &Arg) -> String {
        match arg {
            Arg::Expr(expr) => self.expr(expr),
            Arg::Object(object) => self.object(&object.item),
            Arg::Fun { param, body } => format!("{} ↦ {}", self.var(*param), self.expr(body)),
            Arg::Rate(rate) => rate.to_string(),
        }
    }

    /// In the notation formulas are typed in, so that what is read here can be typed there.
    pub fn expr(&self, expr: &Expr) -> String {
        let names = Names {
            deck: self.deck,
            sig: self.sig,
            objects: &[],
            values: &[],
        };
        formula::write(expr, &names)
    }
}

fn object_kind(object: &Object) -> String {
    match object {
        Object::New { ctor, .. } => ctor.to_string(),
        Object::Group { items, .. } => format!("group of {}", items.len()),
        Object::Copy(_) => "copy".to_owned(),
        Object::Chain { base, .. } => object_kind(&base.item),
    }
}

/// The constructor all of `items` share, if they are all the same primitive.
fn shared_ctor(items: &[Node<Anim>]) -> Option<&Sym> {
    let mut ctors = items.iter().map(|item| match &item.item {
        Anim::Prim { ctor, .. } => Some(ctor),
        _ => None,
    });
    let first = ctors.next()??;
    ctors.all(|ctor| ctor == Some(first)).then_some(first)
}

/// A number without a pointless `.0`.
fn number(value: f64) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    format!("{rounded}")
}

pub fn seconds(secs: f64) -> String {
    format!("{} s", number(secs))
}
