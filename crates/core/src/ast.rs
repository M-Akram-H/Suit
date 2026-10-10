use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{NodeId, Rgba, SlideId, Sym, Time, VarId};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node<T> {
    pub id: NodeId,
    pub item: T,
}

// ── Deck ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Deck {
    pub config: DeckConfig,
    pub slides: Vec<Slide>,
    /// Display names. Python identifiers are derived from these at export.
    pub names: BTreeMap<VarId, String>,
    /// Last id handed out; node, variable and slide ids share one counter.
    #[serde(default)]
    pub next_id: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeckConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub background: Rgba,
    /// Hold appended to every slide so it rests on its finished state (spec §6.6).
    pub gap: Time,
    pub gap_on_loops: bool,
}

impl Default for DeckConfig {
    fn default() -> Self {
        DeckConfig {
            width: 1920,
            height: 1080,
            fps: 60,
            background: Rgba::BLACK,
            gap: Time::from_secs(0.1),
            gap_on_loops: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slide {
    pub id: SlideId,
    pub title: String,
    pub config: SlideConfig,
    pub content: SlideContent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlideContent {
    Blueprint(Blueprint),
    Media { path: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlideConfig {
    pub looping: bool,
    pub auto_next: bool,
    pub skip: bool,
    pub playback_rate: f64,
    pub reversed_playback_rate: f64,
    pub notes: String,
    pub direction: Direction,
}

impl Default for SlideConfig {
    fn default() -> Self {
        SlideConfig {
            looping: false,
            auto_next: false,
            skip: false,
            playback_rate: 1.0,
            reversed_playback_rate: 1.0,
            notes: String::new(),
            direction: Direction::Horizontal,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Blueprint {
    pub body: Vec<Node<Command>>,
}

// ── Commands ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    LetObject { var: VarId, value: Node<Object> },
    LetValue { var: VarId, value: Expr },
    Apply { target: Place, call: Call },
    Add { targets: Vec<Place> },
    Remove { targets: Vec<Place> },
    Play { anims: Vec<Node<Anim>> },
    Wait { duration: Time },
    Attach { target: Place, updater: Updater },
    Detach { target: Place },
    Raw(Raw),
}

/// `x[i₁]…[iₖ]`
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct Place {
    pub root: VarId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<u32>,
}

impl Place {
    pub fn var(root: VarId) -> Place {
        Place {
            root,
            path: Vec::new(),
        }
    }

    pub fn child(&self, index: u32) -> Place {
        let mut path = self.path.clone();
        path.push(index);
        Place {
            root: self.root,
            path,
        }
    }

    /// True when `self` is `other` or one of its ancestors.
    pub fn contains(&self, other: &Place) -> bool {
        self.root == other.root && other.path.starts_with(&self.path)
    }

    /// Two places are disjoint when neither contains the other (spec §5.4).
    pub fn disjoint(&self, other: &Place) -> bool {
        !self.contains(other) && !other.contains(self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Object {
    New { ctor: Sym, args: Args },
    Group { vector: bool, items: Vec<Place> },
    Copy(Place),
    Chain { base: Box<Node<Object>>, call: Call },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub method: Sym,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Args,
}

/// Named arguments in source order. Positional passing is an export detail.
pub type Args = Vec<(Sym, Arg)>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arg {
    Expr(Expr),
    Object(Box<Node<Object>>),
    Fun { param: VarId, body: Box<Expr> },
    Rate(Sym),
}

// ── Animations ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Anim {
    Prim {
        ctor: Sym,
        args: Args,
        opts: Opts,
    },
    Animate {
        target: Place,
        calls: Vec<Call>,
        opts: Opts,
    },
    Par {
        items: Vec<Node<Anim>>,
        opts: Opts,
    },
    Seq {
        items: Vec<Node<Anim>>,
        opts: Opts,
    },
}

impl Anim {
    pub fn opts(&self) -> &Opts {
        match self {
            Anim::Prim { opts, .. }
            | Anim::Animate { opts, .. }
            | Anim::Par { opts, .. }
            | Anim::Seq { opts, .. } => opts,
        }
    }

    pub fn opts_mut(&mut self) -> &mut Opts {
        match self {
            Anim::Prim { opts, .. }
            | Anim::Animate { opts, .. }
            | Anim::Par { opts, .. }
            | Anim::Seq { opts, .. } => opts,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Opts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_time: Option<Time>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<Sym>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lag: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Updater {
    Always(Call),
    Every { dt: VarId, call: Call },
}

impl Updater {
    pub fn call(&self) -> &Call {
        match self {
            Updater::Always(call) | Updater::Every { call, .. } => call,
        }
    }
}

// ── Expressions ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Expr {
    Real(f64),
    Int(i64),
    Bool(bool),
    Str(String),
    Vec3(Box<[Expr; 3]>),
    Color(Rgba),
    Const(Sym),
    Var(VarId),
    Place(Place),
    Call {
        fun: Sym,
        args: Vec<Expr>,
    },
    If {
        cond: Box<Expr>,
        then: Box<Expr>,
        otherwise: Box<Expr>,
    },
    Query {
        target: Place,
        query: Sym,
        args: Args,
    },
}

// ── Escape hatch ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Raw {
    pub python: String,
    pub iface: RawIface,
}

/// What the type checker is told about code it cannot read (spec §5.5, T-Raw).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RawIface {
    pub requires: Vec<(Place, Presence)>,
    pub ensures: Vec<(Place, Presence)>,
    /// New variables: id, kind, presence afterwards.
    pub defines: Vec<(VarId, Sym, Presence)>,
    pub duration: Time,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    Off,
    On,
    Spent,
}
