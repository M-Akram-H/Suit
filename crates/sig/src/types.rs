use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use suit_core::{Presence, Rgba, Sym};

/// Base types `β` (spec §5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Base {
    Real,
    Int,
    Bool,
    Str,
    Vec,
    Color,
}

impl Base {
    /// `Int ≤ Real` is the only subtyping between base types.
    pub fn fits(self, expected: Base) -> bool {
        self == expected || (self == Base::Int && expected == Base::Real)
    }
}

/// Types `τ` a parameter can have.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Type {
    Base(Base),
    /// An object of this kind or a subkind.
    Mob(Sym),
    Fun(Base, Base),
    Rate,
    /// Any one of these, e.g. "a point or an object".
    Union(Vec<Type>),
}

/// How an argument reaches Python.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pass {
    /// By position while no earlier positional parameter was omitted, by name after that.
    Positional,
    Keyword,
    /// Zero or more positional arguments; the name may repeat in `Args`.
    Variadic,
    /// Zero or more values passed together by name, as a list; the name may repeat in
    /// `Args`, once for every value.
    List,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: Sym,
    pub ty: Type,
    pub required: bool,
    pub pass: Pass,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Kind {
    pub parent: Option<Sym>,
    /// Kind of the children, where it differs from the parent kind's.
    pub elem: Option<Sym>,
    /// Constructor parameters every subkind accepts (Manim forwards `**kwargs` upward).
    pub params: Vec<Param>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ctor {
    pub kind: Sym,
    pub params: Vec<Param>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Method {
    pub receiver: Sym,
    pub params: Vec<Param>,
    /// The receiver takes on another object's geometry, so its kind is weakened (spec §5.4).
    pub reshapes: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Query {
    pub receiver: Sym,
    pub params: Vec<Param>,
    pub result: Base,
    /// Read as `p.name`, not `p.name()`.
    pub property: bool,
}

/// What an animation does to the presence of one of its objects (spec §5.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Intro,
    Outro,
    Consume,
    Mut,
    Flash,
    Use,
}

impl Role {
    /// The presence the object must have; `None` for `use`, which accepts off or on.
    pub fn requires(self) -> Option<Presence> {
        match self {
            Role::Intro | Role::Flash => Some(Presence::Off),
            Role::Outro | Role::Consume | Role::Mut => Some(Presence::On),
            Role::Use => None,
        }
    }

    /// The presence the object has afterwards; `None` for `use`, which leaves it alone.
    pub fn ensures(self) -> Option<Presence> {
        match self {
            Role::Intro | Role::Mut => Some(Presence::On),
            Role::Outro | Role::Flash => Some(Presence::Off),
            Role::Consume => Some(Presence::Spent),
            Role::Use => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimParamType {
    Object { role: Role, kind: Sym },
    Value(Type),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimParam {
    pub name: Sym,
    pub ty: AnimParamType,
    pub required: bool,
    pub pass: Pass,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    pub params: Vec<AnimParam>,
    /// Defaults; export always passes the run time explicitly, so this is what is rendered.
    pub run_time: f64,
    pub rate: Sym,
    pub lag: f64,
    /// The `mut` object ends with the `use` object's geometry (Transform).
    pub weakens: bool,
}

/// How a function or operator is spelled in Python.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PyForm {
    Infix(String),
    Prefix(String),
    Call(String),
    /// Component of a vector: `v[i]`.
    Index(usize),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Function {
    pub params: Vec<Base>,
    pub result: Base,
    pub py: PyForm,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Constant {
    Real(f64),
    Vec([f64; 3]),
    Color(Rgba),
}

impl Constant {
    pub fn ty(&self) -> Base {
        match self {
            Constant::Real(_) => Base::Real,
            Constant::Vec(_) => Base::Vec,
            Constant::Color(_) => Base::Color,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Signature {
    pub kinds: BTreeMap<Sym, Kind>,
    pub ctors: BTreeMap<Sym, Ctor>,
    /// Overloads differ in their receiver (`set_value` on a tracker and on a number).
    pub methods: BTreeMap<Sym, Vec<Method>>,
    pub queries: BTreeMap<Sym, Vec<Query>>,
    pub anims: BTreeMap<Sym, Animation>,
    /// Rate function name to its Python spelling.
    pub rates: BTreeMap<Sym, String>,
    pub fns: BTreeMap<Sym, Vec<Function>>,
    pub consts: BTreeMap<Sym, Constant>,
}

pub const ROOT_KIND: &str = "Mobject";
pub const GROUP_KIND: &str = "Group";
pub const VGROUP_KIND: &str = "VGroup";

impl Signature {
    /// `kind` and its ancestors, nearest first.
    pub fn ancestors<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a str> {
        let mut next = self.kinds.contains_key(kind).then_some(kind);
        std::iter::from_fn(move || {
            let current = next?;
            next = self.kinds[current].parent.as_deref();
            Some(current)
        })
    }

    /// `sub ≤ sup`
    pub fn is_subkind(&self, sub: &str, sup: &str) -> bool {
        self.ancestors(sub).any(|kind| kind == sup)
    }

    /// Least common ancestor `a ⊔ b`; the kinds form a tree, so it exists.
    pub fn join(&self, a: &str, b: &str) -> Sym {
        self.ancestors(a)
            .find(|&kind| self.is_subkind(b, kind))
            .unwrap_or(ROOT_KIND)
            .into()
    }

    /// Kind of the children of an object of `kind`.
    pub fn elem(&self, kind: &str) -> Sym {
        self.ancestors(kind)
            .find_map(|k| self.kinds[k].elem.clone())
            .unwrap_or_else(|| ROOT_KIND.into())
    }

    /// Own parameters followed by those inherited along `≤`.
    pub fn ctor_params<'a>(&'a self, ctor: &'a Ctor) -> Vec<&'a Param> {
        let inherited = self
            .ancestors(&ctor.kind)
            .flat_map(|kind| &self.kinds[kind].params);
        let mut params: Vec<&Param> = ctor.params.iter().collect();
        for param in inherited {
            if !params.iter().any(|own| own.name == param.name) {
                params.push(param);
            }
        }
        params
    }

    /// The overload of `name` that applies to an object of `kind`.
    pub fn method(&self, kind: &str, name: &str) -> Option<&Method> {
        self.methods
            .get(name)?
            .iter()
            .find(|m| self.is_subkind(kind, &m.receiver))
    }

    pub fn query(&self, kind: &str, name: &str) -> Option<&Query> {
        self.queries
            .get(name)?
            .iter()
            .find(|q| self.is_subkind(kind, &q.receiver))
    }

    /// Methods an object of `kind` has, for the editor's menus.
    pub fn methods_of<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = (&'a Sym, &'a Method)> {
        self.methods.iter().filter_map(move |(name, overloads)| {
            overloads
                .iter()
                .find(|m| self.is_subkind(kind, &m.receiver))
                .map(|m| (name, m))
        })
    }
}
