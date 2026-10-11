# Manim Core — abstract syntax, AST, type system and semantics

Draft 0.3 · 2026-10-11 · pinned to **manim 0.21.0** and **manim-slides 5.7.0**

Manim has no syntax of its own: a scene is a Python method that calls a library. What this
document formalises is the small language *embedded* in those calls — make objects, change
them, put them on scene, play animations, wait, cut into slides. We call it **Manim Core
(MC)**. MC is what the editor stores, type-checks, lays out on a timeline and compiles to
Python.

| § | Defines | The editor uses it for |
|---|---|---|
| 2 | Signature Σ — the vocabulary (`Circle`, `shift`, `FadeIn`, …) as data | palette, inspector, plug-ins |
| 3 | Abstract syntax | — |
| 4 | AST (Rust) | the document that is saved, edited and undone |
| 5 | Type system | errors and warnings while editing; slide interfaces |
| 6 | Dynamic semantics | timeline, preview, cache keys; the meaning export must preserve |
| 7 | Translation to Python | export to manim-slides |

Four decisions shape everything below.

1. **Closed and total.** No loops, recursion or arbitrary Python in the core. Every deck
   terminates and its duration is known without running it, which is what makes a timeline
   possible.
2. **The vocabulary is data.** The grammar knows *constructor*, *method*, *animation*; it does
   not know `Circle`. Adding a Manim class is a catalogue entry, not a language change.
3. **The semantics is Manim's, not an idealisation.** §6 describes what Manim actually
   does, quirks included. The type system (§5) is where the quirks are fenced off.
4. **One escape hatch.** `raw` holds verbatim Python with a declared interface, so anything
   Manim can do stays reachable and survives a round trip.

Evidence tags: **[src]** read from Manim's source; **[run]** confirmed by running the pinned
Manim (`tools/verify_quirks.py`, which prints one verdict per claim); **[open]** not yet
investigated.

On the version: the source was first read at 0.22.0, but manim-slides 5.7.0 does not run on
it (0.22 removed configuration keys manim-slides still sets), so the pin is 0.21.0. Every
function this document cites — `Scene.add`, `remove`, `play_internal`, `update_to_time`,
`Animation`, `AnimationGroup`, `Succession`, the Transform family, the rate functions,
`Mobject.update` — is identical in the two versions.

---

## 1. Notation

`x̄` is a sequence `x₁ … xₙ`; `ε` the empty sequence; `·` concatenation. `f[x ↦ v]` updates a
finite map. `A ⇀ B` is a partial map. `δ = 1/fps` is the frame step. `a ?? b` is `a` if present,
else `b`. Rules are written premises-over-conclusion.

---

## 2. Signature Σ

MC is parameterised by a signature, exactly as first-order logic is:

```
Σ = ⟨ K, ≤, elem, Ctor, Meth, Query, Anim, Rate, Fn, Const ⟩
```

| Component | Content | Examples |
|---|---|---|
| `K`, `≤` | Kinds: Manim's mobject classes, ordered by subclassing. A tree with root `Mobject`. | `Circle ≤ Arc ≤ VMobject ≤ Mobject` |
| `elem : K ⇀ K` | Kind of the children of a container kind | `elem(VGroup) = VMobject`, `elem(MathTex) = VMobject` |
| `Ctor` | `C : (l̄ : τ̄ = defaults) → κ` | `Circle : (radius: Real = 1, color: Color = …) → Circle` |
| `Meth` | `f : (κ ; l̄ : τ̄)` — mutators; the receiver keeps its kind | `shift : (Mobject; by: Vec)`, `set_color : (Mobject; color: Color)` |
| `Query` | `q : (κ ; l̄ : β̄) → β` — pure reads | `get_center : (Mobject) → Vec`, `c2p : (Axes; x, y: Real) → Vec` |
| `Anim` | `A : (l̄ : ς̄)`, each `ς` a value type or an object *role* (§5.4); plus default run time, rate, lag | `FadeIn : (mobject: intro Mobject, shift: Vec = 0)` |
| `Rate` | Named rate functions `[0,1] → ℝ` | `smooth`, `linear`, `there_and_back`, `wiggle`, the thirty `ease_*` functions … |
| `Fn`, `Const` | Pure functions, operators and constants | `sin`, `+`, `<` ; `UP`, `PI`, `BLUE` |

Parameters are inherited along `≤` (Manim forwards `**kwargs` to the superclass), so
`VMobject`'s `fill_opacity`, `stroke_width`, … are available on every vector kind **[src]**.

An **interpretation** `𝓘` of Σ gives each symbol its meaning: `𝓘(C)` builds points and style,
`𝓘(f)` transforms an object, `𝓘(q)` reads one, `𝓘(A)` supplies the per-frame step of an
animation. The *reference interpretation* is Manim itself. §6 is generic in `𝓘`; a native Rust
preview would be a second, partial interpretation that must agree with the reference one.

The editor's canvas uses the reference interpretation directly rather than a second one. A
generated *probe* script (`suit-cli probe`) runs the deck in Manim with animations skipped to
their end state and reports, after every command, the outline, fill and stroke of everything
on scene and which variable each outline belongs to. The canvas rasterises those outlines, so
what it shows for text, formulas and graphs is what Manim will draw. The one difference from
a real render is the one skipping makes: a time-based updater sees a single long frame per
step instead of many short ones.

To *play* a slide, a second probe (`suit-cli probe --frames N`) runs the slides before it
skipped and then plays that one for real, frame for frame, writing down what is drawn in
each frame instead of drawing it. The canvas plays those frames back, so the motion is
Manim's own — its interpolation, rate functions, lags and updaters — without a video in
between. It is also a standing check of P3 (§6.7): the recording has exactly `frames(B)`
frames, plus the gap, or the tests fail. A still `wait` is the one thing not passed to Manim
as is: recording makes Manim treat every wait as live, so its frames are written down as
one frame repeated, which is what E-Wait-Still says they are.

Σ should be generated, not typed in: a script imports the installed Manim, dumps constructor
and method signatures by introspection, and merges a small hand-written overlay holding what
Python cannot tell us — kinds of `self`-returning methods, and animation roles. Appendix A
lists the starting catalogue.

---

## 3. Abstract syntax

```
x, g ∈ Var        variables, assigned exactly once
l    ∈ Label      parameter names
i, n ∈ ℕ          r ∈ ℝ          s ∈ String          b ∈ 𝔹
C, f, q, A, ϱ, h, k        constructors, methods, queries, animations, rate functions,
                           functions, constants — all drawn from Σ

Deck        D  ::= deck(cfg, S₁ … Sₙ)                       n ≥ 1
Slide       S  ::= slide(sc, B)                             an animated slide
                 | media(sc, s)                             an external video or image
Blueprint   B  ::= c₁ ; … ; cₖ                              k ≥ 0

Command     c  ::= let x = m                                create an object and name it
                 | let x = e                                name a value
                 | p.μ                                      change an object, instantly
                 | add(p̄)                                   put on scene, instantly
                 | remove(p̄)                                take off scene, instantly
                 | play(a₁ … aₙ)                            n ≥ 1; animate in parallel; time passes
                 | wait(d)                                  hold; time passes
                 | attach(p, u)                             add an updater
                 | detach(p)                                clear updaters
                 | raw⟨ι⟩(s)                                verbatim Python, interface ι

Place       p  ::= x | p[i]                                 an object, or its i-th child
Object      m  ::= C(l̄ = w̄)                                 construct
                 | group(p̄) | vgroup(p̄)                     collect existing objects
                 | copy(p)
                 | m.μ                                      construct, then modify
Call        μ  ::= f(l̄ = w̄)
Argument    w  ::= e | m | λx. e | ϱ

Animation   a  ::= A(l̄ = w̄) @ o                             primitive
                 | p.animate(μ₁ … μₖ) @ o                   k ≥ 1; morph to the state after μ̄
                 | par(a₁ … aₙ) @ o                         together, staggered by o.lag
                 | seq(a₁ … aₙ) @ o                         one after another
Options     o  ::= { run_time : d ?, rate : ϱ ?, lag : r ? }

Updater     u  ::= always μ                                 re-apply μ every frame
                 | every dt. μ                              same; μ may mention the frame time dt

Expr        e  ::= r | b | s | ⟨e, e, e⟩ | #rrggbbaa | k | x | p
                 | h(ē) | if e then e else e
                 | p.q(l̄ = ē)                               ask an object something

Duration    d  ::= r                                        a literal, in seconds, r > 0
SlideCfg    sc ::= { loop, auto_next, skip : b ; playback_rate, reversed_playback_rate : r ;
                     notes : s ; direction : horizontal | vertical }
DeckCfg    cfg ::= { width, height, fps : ℕ ; background : color ; gap : r ; gap_on_loops : b }
```

Reading notes.

- A **blueprint** is the body of one slide: a straight-line program. A node-graph or a
  layered timeline are two *concrete* syntaxes for it; both produce this abstract syntax.
- `play` is the only way objects change *over time*; `p.μ` changes them between frames.
- Operators are functions (`a + b` is `+(a, b)`), so `Expr` needs no operator cases.
- Durations are literals, not expressions. That is what makes §5.7 a static computation.
- `SlideCfg` is `BaseSlideConfig` of manim-slides minus `src` and `type` (which became
  `media`) and `dedent_notes` (an export detail) **[src]**. `gap` / `gap_on_loops` are
  `wait_time_between_slides` / `wait_between_looping_slides` **[src]**.

Deliberately absent in 0.1: user-defined functions, loops, 3D scenes, camera moves, sound,
`wait_until`. See §8.

---

## 4. The AST

One Rust type per syntactic category; one variant per production. Three things are added that
abstract syntax does not have, all for the editor's benefit: stable **node ids** (selection,
undo, diffing), **variable ids** distinct from display names (renaming never breaks a
reference), and **exact time**.

This is `crates/core/src/ast.rs` without its serialisation attributes; that file is the
authoritative copy.

```rust
//! Manim Core AST. Mirrors docs/spec/manim-core.md §3; no behaviour lives here.

/// Key into the signature Σ: "Circle", "shift", "FadeIn", "smooth", "+", "UP".
pub type Sym = std::sync::Arc<str>;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct NodeId(pub u64);
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct VarId(pub u64);
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct SlideId(pub u64);

/// Exact time in flicks (1/705_600_000 s). 24, 25, 30, 50, 60 and 120 fps all divide it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Time(pub i64);

/// 8-bit sRGB with alpha, written `#rrggbb` or `#rrggbbaa` as Manim writes colours.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Rgba(pub [u8; 4]);

pub struct Node<T> {
    pub id: NodeId,
    pub item: T,
}

// ── Deck ─────────────────────────────────────────────────────────────────────

pub struct Deck {
    pub config: DeckConfig,
    pub slides: Vec<Slide>,
    /// Display names. Python identifiers are derived from these at export.
    pub names: std::collections::BTreeMap<VarId, String>,
    /// Last id handed out; node, variable and slide ids share one counter.
    pub next_id: u64,
}

pub struct DeckConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub background: Rgba,
    pub gap: Time,
    pub gap_on_loops: bool,
}

pub struct Slide {
    pub id: SlideId,
    pub title: String,
    pub config: SlideConfig,
    pub content: SlideContent,
}

pub enum SlideContent {
    Blueprint(Blueprint),
    Media { path: String },
}

pub struct SlideConfig {
    pub looping: bool,
    pub auto_next: bool,
    pub skip: bool,
    pub playback_rate: f64,
    pub reversed_playback_rate: f64,
    pub notes: String,
    pub direction: Direction,
}

pub enum Direction {
    Horizontal,
    Vertical,
}

pub struct Blueprint {
    pub body: Vec<Node<Command>>,
}

// ── Commands ─────────────────────────────────────────────────────────────────

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
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Place {
    pub root: VarId,
    pub path: Vec<u32>,
}

pub enum Object {
    New { ctor: Sym, args: Args },
    Group { vector: bool, items: Vec<Place> },
    Copy(Place),
    Chain { base: Box<Node<Object>>, call: Call },
}

pub struct Call {
    pub method: Sym,
    pub args: Args,
}

/// Named arguments in source order. Positional passing is an export detail.
pub type Args = Vec<(Sym, Arg)>;

pub enum Arg {
    Expr(Expr),
    Object(Box<Node<Object>>),
    Fun { param: VarId, body: Box<Expr> },
    Rate(Sym),
}

// ── Animations ───────────────────────────────────────────────────────────────

pub enum Anim {
    Prim { ctor: Sym, args: Args, opts: Opts },
    Animate { target: Place, calls: Vec<Call>, opts: Opts },
    Par { items: Vec<Node<Anim>>, opts: Opts },
    Seq { items: Vec<Node<Anim>>, opts: Opts },
}

#[derive(Default)]
pub struct Opts {
    pub run_time: Option<Time>,
    pub rate: Option<Sym>,
    pub lag: Option<f64>,
}

pub enum Updater {
    Always(Call),
    Every { dt: VarId, call: Call },
}

// ── Expressions ──────────────────────────────────────────────────────────────

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
    Call { fun: Sym, args: Vec<Expr> },
    If { cond: Box<Expr>, then: Box<Expr>, otherwise: Box<Expr> },
    Query { target: Place, query: Sym, args: Args },
}

// ── Escape hatch ─────────────────────────────────────────────────────────────

pub struct Raw {
    pub python: String,
    pub iface: RawIface,
}

/// What the type checker is told about code it cannot read (§5.5, T-Raw).
pub struct RawIface {
    pub requires: Vec<(Place, Presence)>,
    pub ensures: Vec<(Place, Presence)>,
    pub defines: Vec<(VarId, Sym, Presence)>,
    pub duration: Time,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Presence {
    Off,
    On,
    Spent,
}
```

Well-formedness the types do not express and the checker must: node ids are unique in a deck;
every `VarId` is bound by exactly one `Let*`, `Fun`, `Every` or `RawIface::defines`, before
its uses; `Time` values are positive where the grammar says `d`.

---

## 5. Type system

Two layers. The first is ordinary: values and objects have types, and kinds follow Manim's
class tree. The second is a **typestate**: for every object the checker tracks whether it is
on scene. Almost every mistake a newcomer makes in Manim is a presence mistake, so this layer
is where the usability comes from.

### 5.1 Types, kinds, presence

```
β ::= Real | Int | Bool | Str | Vec | Color            base types        (Int ≤ Real)
τ ::= β | Mob κ | β → β | Rate                         types             (Mob κ ≤ Mob κ′ if κ ≤ κ′)
π ::= off | on | spent                                 presence of an object
T ::= π | ⟨T₁, …, Tₙ⟩                                  presence of an object and its children
Γ ::= ·  |  Γ, x : β  |  Γ, x : Mob κ @ T  |  Γ, x ≔ p     contexts; the last form is an alias
```

- `off` — exists, not displayed. `on` — displayed. `spent` — an animation used it up; its
  geometry is no longer meaningful (§5.4).
- `Γ(p)` follows aliases, then walks the path into `T`; reaching a leaf `π` early means the
  children share it. The **state** of a place is that `π` if every leaf below it agrees, and
  `mixed` otherwise. `Γ[p ↦ π]` overwrites the subtree at `p` and collapses `⟨π, …, π⟩` to `π`.
- The kind of `p[i]` is `elem(κ)`. Whether `i` is in range depends on the object (how many
  glyphs a formula has is only known after typesetting); since MC is closed and total the
  checker simply asks the interpretation.
- `Vec` is a point *and* a displacement. Manim does not separate them (`Dot(point = 2*LEFT + UP)`
  is idiomatic), so neither does MC.

### 5.2 Expressions — `Γ ⊢ e : β`

Literals and constants have the evident types; `h(ē)` and `if` are checked against Σ as usual.
The two rules that involve objects:

```
 x : Mob κ @ T ∈ Γ (via aliases)     path valid     state ≠ spent
 ────────────────────────────────────────────────────────────── T-Place
 Γ ⊢ p : Mob κ_p

 Γ ⊢ p : Mob κ     κ ≤ κ′     Σ.Query(q) = (κ′ ; l̄ : β̄) → β     Γ ⊢ eᵢ : βᵢ
 ────────────────────────────────────────────────────────────────────── T-Query
 Γ ⊢ p.q(l̄ = ē) : β
```

A query on an `off` object is fine: `get_center` does not care whether something is displayed.

### 5.3 Objects — `Γ ⊢ m : Mob κ`

```
 Σ.Ctor(C) = (l̄ : τ̄ = d̄) → κ     every given argument has its declared type
 every omitted parameter has a default
 ───────────────────────────────────────────────────────────────────── T-New
 Γ ⊢ C(l̄ = w̄) : Mob κ

 Γ ⊢ m : Mob κ     κ ≤ κ′     Σ.Meth(f) = (κ′ ; l̄ : τ̄)     arguments well-typed
 ───────────────────────────────────────────────────────────────────── T-Chain
 Γ ⊢ m.f(l̄ = w̄) : Mob κ

 Γ ⊢ p : Mob κ
 ──────────────────── T-Copy
 Γ ⊢ copy(p) : Mob κ
```

Arguments: `e` at a base type; `p` or an inline `m` at `Mob κ`; `λx. e` at `β₁ → β₂` when
`Γ, x : β₁ ⊢ e : β₂`; `ϱ` at `Rate`. Grouping changes the context, so it is a command rule
(T-LetGroup below).

### 5.4 Animations — `Γ ⊢ a ▷ Φ`

An animation's type is its **effect** `Φ`: a finite map from places to presence transitions.
Σ assigns every object parameter of every animation one of six roles:

| Role | Requires | Ensures | Meaning | Examples |
|---|---|---|---|---|
| `intro` | off | on | appears through the animation | `FadeIn`, `Create`, `Write`, `GrowFromCenter` |
| `outro` | on | off | disappears; object intact afterwards | `FadeOut`, source of `FadeTransform` |
| `consume` | on | spent | disappears; object unusable afterwards | source of `ReplacementTransform`, `Uncreate` |
| `mut` | on | on | changed in place | `Indicate`, `Rotate`, `p.animate`, source of `Transform` |
| `flash` | off | off | visible only while it runs | `ShowPassingFlash` |
| `use` | off or on | unchanged | read only: a target, a path, a template | target of `Transform`, path of `MoveAlongPath` |

```
 Σ.Anim(A) = (l̄ : ς̄)
 for each object parameter lⱼ : rⱼ κⱼ —   Γ ⊢ pⱼ : Mob κ,  κ ≤ κⱼ,  state(Γ(pⱼ)) = requires(rⱼ)
 an inline object m is allowed only where rⱼ = use
 value parameters well-typed     non-use places pairwise disjoint     o well-formed
 ──────────────────────────────────────────────────────────────────────────── T-Prim
 Γ ⊢ A(l̄ = w̄) @ o  ▷  { pⱼ ↦ rⱼ }

 Γ ⊢ p : Mob κ     state(Γ(p)) = on     each μᵢ is a method of κ, arguments well-typed
 ──────────────────────────────────────────────────────────────────────────── T-Animate
 Γ ⊢ p.animate(μ̄) @ o  ▷  { p ↦ mut }

 Γ ⊢ aᵢ ▷ Φᵢ  (all i)     Φᵢ # Φⱼ  (all i ≠ j)
 ────────────────────────────────────────────── T-Par
 Γ ⊢ par(ā) @ o  ▷  Φ₁ ⊎ … ⊎ Φₙ

 Γ ⊢ a₁ ▷ Φ₁     Γ·Φ₁ ⊢ a₂ ▷ Φ₂     …     Γ·Φ₁·…·Φₙ₋₁ ⊢ aₙ ▷ Φₙ
 ─────────────────────────────────────────────────────────── T-Seq
 Γ ⊢ seq(ā) @ o  ▷  Φ₁ ; … ; Φₙ
```

- Two places are **disjoint** when neither is a prefix of the other.
- `Φ # Φ′` (*separate*): every non-`use` place of one is disjoint from every place of the
  other. This is the rule "one object, one animation at a time": Manim accepts two animations
  on the same object in one `play` and silently lets the later one win.
- `Γ·Φ` applies the *ensures* column to `Γ`. `Φ ; Φ′` composes transitions on shared places.
- `o` is well-formed when `run_time > 0` if present, `lag ≥ 0` if present, and `lag` is absent
  on `seq`.
- T-Seq has one side condition. The target of `p.animate(…)` is worked out when the `play`
  starts (§6.4), not when its turn in the sequence comes. So in `seq(a₁ … aₙ)` an `animate`
  on a place may not follow a child that is `mut` on an overlapping place: it would start
  from the state before the sequence and undo that child (§6.5, Q7). `intro` before
  `animate` is fine, and so is `animate` before a primitive such as `Rotate`, which works
  out its target when it begins.

`Transform(x, y)` has one more consequence the catalogue records: afterwards `x` keeps its
identity but has `y`'s geometry, so its kind is weakened to the least common ancestor
`κₓ ⊔ κ_y` and indexing into it is no longer allowed. A `Circle` morphed into a `Square` is a
`VMobject`, not a circle. `ReplacementTransform` has no such problem — the source is consumed
and the target, with its own kind and structure, is what remains — which is why it should be
the editor's default "morph".

### 5.5 Commands — `Γ ⊢ c ⊣ Γ′`

```
 Γ ⊢ m : Mob κ     x fresh
 ──────────────────────────────────────── T-Let
 Γ ⊢ let x = m  ⊣  Γ, x : Mob κ @ off

 G ∈ {group, vgroup}     p̄ distinct variables, none an alias     Γ ⊢ pᵢ : Mob κᵢ,  κᵢ ≤ elem(G)
 state(Γ(pᵢ)) = π for all i,  π ∈ {off, on}     g fresh
 ──────────────────────────────────────────────────────────────────────────── T-LetGroup
 Γ ⊢ let g = G(p̄)  ⊣  (Γ without p̄), g : Mob G @ π, p₁ ≔ g[0], …, pₙ ≔ g[n−1]

 Γ ⊢ e : β     x fresh                        Γ ⊢ p : Mob κ     μ a method of κ, well-typed
 ───────────────────────────── T-LetVal       ──────────────────────────────────────── T-Apply
 Γ ⊢ let x = e  ⊣  Γ, x : β                    Γ ⊢ p.μ  ⊣  Γ

 p̄ pairwise disjoint     state(Γ(pᵢ)) = off     p̄ pairwise disjoint     state(Γ(pᵢ)) = on
 ───────────────────────────────────── T-Add    ───────────────────────────────────── T-Remove
 Γ ⊢ add(p̄)  ⊣  Γ[p̄ ↦ on]                       Γ ⊢ remove(p̄)  ⊣  Γ[p̄ ↦ off]

 Γ ⊢ par(ā) ▷ Φ     max dur(aᵢ) > 0                         d > 0
 ──────────────────────────────────── T-Play                ──────────────────── T-Wait
 Γ ⊢ play(ā)  ⊣  Γ·Φ                                         Γ ⊢ wait(d)  ⊣  Γ

 Γ ⊢ p : Mob κ     (Γ, dt : Real if u = every dt. μ) ⊢ μ a method of κ, well-typed
 ──────────────────────────────────────────────────────────────────────── T-Attach
 Γ ⊢ attach(p, u)  ⊣  Γ

 every (p, π) ∈ ι.requires has state(Γ(p)) = π
 ──────────────────────────────────────────────────────── T-Raw
 Γ ⊢ raw⟨ι⟩(s)  ⊣  Γ[ι.ensures], ι.defines
```

T-LetGroup is an ownership rule. Grouping *moves* its operands: the old names stay usable but
now denote places inside the group, so there is one source of truth for presence and an
object never has two parents. Manim allows a mobject in several groups; the result is
double updates and removals that half work, and MC rules it out.

`raw` is trusted, not checked: `ι` is a promise the author makes.

### 5.6 Slides and decks — `Γ ⊢ S ⊣ Γ′`

```
 Γ ⊢ c₁ ⊣ Γ₁   …   Γₖ₋₁ ⊢ cₖ ⊣ Γ′     B contains a play or wait     sc well-formed
 ──────────────────────────────────────────────────────────────────────── T-Slide
 Γ ⊢ slide(sc, B)  ⊣  Γ′

 sc well-formed                          · ⊢ S₁ ⊣ Γ₁     Γ₁ ⊢ S₂ ⊣ Γ₂   …   Γₙ₋₁ ⊢ Sₙ ⊣ Γₙ
 ───────────────────────── T-Media       ───────────────────────────────────────────── T-Deck
 Γ ⊢ media(sc, s)  ⊣  Γ                  ⊢ deck(cfg, S̄)
```

- `sc` is well-formed when both playback rates are positive.
- "Contains a play or wait" is not pedantry: manim-slides rejects a slide with no animation
  ("You have to play at least one animation … before pausing") and silently drops the
  configuration of an empty one **[src]**.
- **A slide's type is its interface**: `Γ` is what it needs to find on scene, `Γ′` what it
  leaves. Reordering, duplicating or deleting slides is safe exactly when T-Deck still holds,
  and the checker can say which object is missing where.

### 5.7 Static duration

Because durations are literals, the length of everything is computed from the AST alone.

```
dur(A(…) @ o)         = o.run_time ?? Σ.default(A)
dur(p.animate(…) @ o) = o.run_time ?? 1
dur(par(ā) @ o)       = o.run_time ?? maxᵢ (sᵢ + dᵢ)       dᵢ = dur(aᵢ),  s₁ = 0,  sᵢ₊₁ = sᵢ + λ·dᵢ,  λ = o.lag ?? 0
dur(seq(ā) @ o)       = o.run_time ?? dur(a₁) + … + dur(aₙ)

frames(play(ā)) = ⌈ max(δ, maxᵢ dur(aᵢ)) ÷ δ ⌉
frames(wait(d)) = ⌈ max(δ, d) ÷ δ ⌉  if an updater is live,   ⌊ max(δ, d) ÷ δ ⌋  otherwise
frames(c)       = 0   for every other command
frames(B)       = frames(c₁) + … + frames(cₖ)
```

`÷` is IEEE-754 binary64 division and `δ` is the binary64 value of `1/fps`. That is not
fussiness: Manim obtains its frame times from `numpy.arange(0, R, 1/fps)`, whose length is
that ceiling, and for about 8 % of whole-frame durations the rounding lands one frame high —
a run time of `23/60` s at 60 fps gives 24 frames, not 23 **[src] [run]**. An updater is
*live* when it is attached to something at least partly on scene. Rust's `f64` is the same arithmetic, so the editor can reproduce Manim's count
exactly, provided it uses this formula and not `round(d·fps)`.

### 5.8 What the checker reports

Errors are rule violations. Each prevents a specific Manim behaviour:

| Rejected | What Manim would do |
|---|---|
| `mut`/`outro`/`consume` on an `off` object | add it abruptly at the first frame, then animate **[src]** |
| `intro` on an `on` object | make it vanish and reappear |
| any use of a `spent` object | show the leftover geometry of a finished morph, or nothing |
| two animations on overlapping places in one `play` | let the later one silently win |
| `animate` after a `mut` of the same place in one `seq` | undo the earlier change **[run]** |
| wrong kind (`Create` on an image, `TransformMatchingTex` on plain text) | raise `TypeError`, or fail an assertion, mid-render **[src]** |
| a slide without `play`/`wait` | drop or reject the slide **[src]** |
| `play` whose longest animation has duration 0 | raise `ValueError` **[src]** |

Warnings are legal programs that are probably not meant:

| Warned | Why |
|---|---|
| `loop` slide whose exit presence differs from its entry | the loop will visibly jump |
| `loop` together with `auto_next` | the presenter loops for ever, the web export moves on after one pass (§6.6) **[src]** |
| `loop` on the first or last slide | not supported by the RevealJS export **[src]** |
| `p.animate(rotate(…))` | morphs along straight lines instead of rotating; `Rotate` is meant **[run]** |
| an updater that reads an object which an animation then consumes | it keeps following the leftover geometry |
| `Transform` where the source is never used again | `ReplacementTransform` is simpler and keeps kinds exact |
| an object left `on` at the end of the deck with no later use | harmless; often a forgotten `FadeOut` |
| two on-scene roots sharing a layer | draw order then depends on edit history (§6.7, P4) |

---

## 6. Dynamic semantics

Big-step and deterministic. A command maps a state to a state and emits a finite sequence of
frames. A frame is a display list, not pixels, so the semantics is resolution-independent;
rasterising is a separate function of `cfg`.

### 6.1 Domains

```
ℓ ∈ Loc                                           object identities
o ∈ Obj    = ⟨κ, P, sty, z, ch, U, susp⟩           kind, points, style, layer, children, updaters, suspended
P ∈ Vec*                                          Bézier control points, four per cubic segment
σ ∈ Store  = Loc ⇀ Obj
Δ ∈ Loc*                                          display list: roots on scene, oldest first
v ∈ Val    = ℝ | 𝔹 | String | ℝ³ | Color | Loc | (Val → Val)
ρ ∈ Env    = Var ⇀ Val
W ∈ World  = ⟨σ, Δ⟩                               the state of a scene
φ ∈ Frame  = (P × sty)*                           what is drawn, back to front
```

```
fam(σ, ℓ)     = ℓ · fam(σ, c₁) · … · fam(σ, cₙ)     where σ(ℓ).ch = c₁ … cₙ          (pre-order)
frame(W)      = [ ⟨σ(ℓ).P, σ(ℓ).sty⟩ | ℓ ∈ sort_z(fam(σ, Δ)),  σ(ℓ).P ≠ ε ]          sort is stable
Δ ⊖ L         = Δ with every member of fam(L) removed; a root that loses only part of its
                family is replaced, in place, by its surviving children (recursively)
upd(W, dt)    = for each ℓ in Δ, in order: update(ℓ, dt)
update(ℓ, dt) = nothing, if σ(ℓ).susp;  else run each u ∈ σ(ℓ).U in order, then update each child
```

These are Manim's `Scene.mobjects`, `get_family`, `restructure_mobjects` and `Mobject.update`
**[src]**. Coordinates are Manim units with Manim's default frame: origin at the centre, `y`
up, height 8, width `8 · width/height`.

### 6.2 Expressions and objects

`ρ, σ ⊢ e ⇓ v` is standard. Places resolve through the store, `ρ(x[i]) = σ(ρ(x)).ch[i]`, and a
query reads it: `p.q(ē) ⇓ 𝓘(q)(σ, ρ(p), v̄)`. Expressions never write.

`⟨m, ρ, σ⟩ ⇓ ⟨ℓ, σ′⟩` allocates:

```
C(l̄ = w̄)     evaluate w̄;  allocate the object tree 𝓘(C)(v̄) at fresh locations;  ℓ is its root
group(p̄)     allocate ⟨Group, ε, ·, 0, ρ(p̄), ε, false⟩ — children by reference
copy(p)      allocate a deep copy of fam(ρ(p)), updaters included
m.f(l̄ = w̄)   evaluate m to ℓ, then w̄;  σ′ = 𝓘(f)(σ, ℓ, v̄)
```

Arguments are evaluated **once, when the command runs**. `λx. e` becomes a function value
closed over `ρ`; variables are single-assignment, so capture by value and Python's capture by
reference coincide.

### 6.3 Commands — `⟨c, ρ, W⟩ ⇓ ⟨ρ′, W′, F⟩`

`F ∈ Frame*` are the frames emitted. Only `play` and `wait` emit any.

```
let x = m       ρ′ = ρ[x ↦ ℓ], σ′ from §6.2.  If m = group(p̄) and the operands are on scene:
                Δ′ = (Δ ⊖ ℓ) · ℓ, so the group itself is listed and its updaters run.
let x = e       ρ′ = ρ[x ↦ v]
p.f(l̄ = w̄)      σ′ = 𝓘(f)(σ, ρ(p), v̄).  Visible from the next emitted frame.
add(p̄)          Δ′ = (Δ ⊖ ρ(p̄)) · ρ(p̄)
remove(p̄)       Δ′ = Δ ⊖ ρ(p̄)
attach(p, u)    append the closure ⟨u, ρ⟩ to σ(ρ(p)).U
detach(p)       σ(ℓ).U ← ε for every ℓ ∈ fam(ρ(p))
raw⟨ι⟩(s)       whatever Python does.  Defined only by the reference interpretation.
```

An updater closure `⟨always f(w̄), ρ⟩` run on `ℓ` re-evaluates `w̄` in the *current* store and
applies `𝓘(f)`; `every dt. f(w̄)` does the same with `dt` bound to the frame's time step.

### 6.4 Animations

At the start of a `play`, each animation is turned into a record

```
⟦a⟧ρ,W = ⟨ d, pre, at, post ⟩        d : ℝ≥0      pre, post : World → World      at : ℝ × World → World
```

mirroring Manim's `run_time`, `_setup_scene` + `begin`, `interpolate`, and `finish` +
`clean_up_from_scene` **[src]**. The argument of `at` is the progress `α`, Manim's "alpha".

**Primitive** `A(…) @ o` on object `ℓ`, whose family members with points are `ℓ₀ … ℓₘ₋₁`:

```
d      = o.run_time ?? Σ.default(A)          ϱ = o.rate ?? Σ.rate(A)          λ = o.lag ?? Σ.lag(A)

pre    : if A is an intro and ℓ ∉ fam(Δ):  Δ ← Δ · ℓ
         s ← a snapshot of fam(ℓ);   susp ← true on fam(ℓ);   at(0)
at(α)  : for i < m:    αᵢ = ϱ( α · ((m−1)·λ + 1) − i·λ )        σ(ℓᵢ) ← 𝓘(A).step(sᵢ, αᵢ)
post   : at(1);   susp ← false on fam(ℓ);   if A removes:  Δ ← Δ ⊖ ℓ;   𝓘(A).cleanup
```

Defaults in `Animation`: run time 1, rate `smooth`, lag 0 **[src]**. Every built-in rate
function clamps its argument to `[0, 1]` (or returns 0 outside it, for the there-and-back
family) **[src]**, which is why `at` may safely be called with `α > 1`.

The whole Transform family has the same step — interpolate between the snapshot and an
aligned copy `t` of the target:

```
𝓘(Transform).step(sᵢ, α) :   P   = path(sᵢ.P, tᵢ.P, α)        straight line unless path_arc is given
                             sty = (1 − α)·sᵢ.sty + α·tᵢ.sty
```

`p.animate(μ̄)` is a Transform whose target is `copy(p)` after `μ̄`, built when the `play`
starts; `post` then applies `μ̄` to the real object **[src]**. So it *interpolates between two
states*; it does not replay the methods. `p.animate(rotate(π))` therefore shrinks through the
centre and grows back — a famous surprise that falls straight out of this rule.

`cleanup` per catalogue entry **[src]**: `ReplacementTransform` puts the target in the
source's slot of `Δ` and leaves the source morphed, hence `consume`. `FadeOut`, `FadeTransform`
and `TransformMatching*` reset their source to its snapshot before removing it (the latter two
then append the target), hence `outro`. `Uncreate` ends at an empty path with no reset, hence
`consume` **[run]**.

**Composites.** With `dᵢ = dur(aᵢ)`, `s₁ = 0`, `sᵢ₊₁ = sᵢ + λ·dᵢ`, `M = maxᵢ (sᵢ + dᵢ)`:

```
par(ā) @ o     d = o.run_time ?? M       λ = o.lag ?? 0       ϱ = o.rate ?? linear
   pre   : pre₁ ; … ; preₙ
   at(α) : θ = ϱ(α) · M;   for each i with sᵢ ≤ θ that has not finished:  atᵢ( clamp₀¹((θ − sᵢ) / dᵢ) )
   post  : post₁ ; … ; postₙ

seq(ā) @ o     as par with λ = 1, except that preᵢ₊₁ runs when aᵢ finishes, not at the start
```

Note the default rate of a composite is `linear`, not `smooth`, and that setting `run_time`
on a composite rescales its children rather than truncating them **[src]**.

### 6.5 `play` and `wait`

```
 ⟦aᵢ⟧ρ,W = ⟨dᵢ, preᵢ, atᵢ, postᵢ⟩                                    arguments evaluated in W
 R  = max(δ, maxᵢ dᵢ)             N = ⌈R ÷ δ⌉
 W⁰ = (preₙ ∘ … ∘ pre₁)(present(W))
 for j = 0 … N−1:     t = j·δ       dt = 0 if j = 0, else δ
       Wʲ⁺¹ = upd( (atₙ(t/dₙ) ∘ … ∘ at₁(t/d₁))(Wʲ), dt )
       φⱼ   = frame(Wʲ⁺¹)
 W′ = upd( (postₙ ∘ … ∘ post₁)(Wᴺ), 0 )
 ──────────────────────────────────────────────────────────────── E-Play
 ⟨play(ā), ρ, W⟩ ⇓ ⟨ρ, W′, φ₀ … φ_{N−1}⟩
```

`present(W)` is Manim's "anything animated that is not in the scene gets added": for each
top-level `aᵢ` that is not an intro, if its object is not in `fam(Δ)`, then `Δ ← (Δ ⊖ ℓ) · ℓ`.
For a composite, "its object" is a *fresh group* of the objects of its non-intro children —
never in `fam(Δ)` — so those objects are always moved to the end of `Δ` **[src] [run]**.

```
 R = max(δ, d)        live(W) ⟺ some ℓ ∈ fam(σ, Δ) carries an updater

 ¬live(W)        N = ⌊R ÷ δ⌋                            live(W)
 ─────────────────────────────────── E-Wait-Still       ─────────────────────────────── E-Wait-Live
 ⟨wait(d), ρ, W⟩ ⇓ ⟨ρ, W, frame(W)ᴺ⟩                     as E-Play with one no-op animation of length R
```

`live` is where MC departs from a bare `self.wait`: Manim calls a wait still unless a
*time-based* updater is present, which leaves `always` updaters unrun (Q4 below). MC counts
every updater, and the translation says so to Manim with `frozen_frame=False` (§7).

Seven consequences, each a known Manim surprise that now has a precise cause. All seven are
**[run]**:

| # | Consequence | Cause in the rules |
|---|---|---|
| Q1 | The finished state of an animation is **not** in its own `play`. The last frame is at `α = (N−1)δ/R < 1`; the end state first shows in the *next* frame emitted. | frames are taken at `t = jδ < R`; `post` runs after the last one **[src]** |
| Q2 | Frame counts can be one more than `R·fps`, and a still `wait` rounds the other way. | `⌈·⌉` vs `⌊·⌋` in binary64 (§5.7) **[src]** |
| Q3 | Time-based updaters advance `R − δ` per `play`, not `R`. | `dt = 0` on the first frame of every `play` **[src]** |
| Q4 | In plain Manim a still `wait` shows stale `always` updaters: move an object with `p.μ`, then wait, and its followers have not moved. MC's `wait` is live in that case, so it does not happen. | E-Wait-Still does not call `upd` **[src]** |
| Q5 | Wrapping animations in `par`/`seq` brings their objects to the front. | `present` re-adds a fresh group **[src]** |
| Q6 | In a parallel `play`, a shorter animation sits at its end state until the longest finishes. | `at(t/dᵢ)` with `t/dᵢ > 1` is clamped by the rate function **[src]** |
| Q7 | `seq(p.animate(shift(UP)), p.animate(shift(DOWN)))` does not go up and then down: it goes down and stops. Likewise `seq(Rotate(p), p.animate(…))` ends unrotated. | every `p.animate` in a `play` is given its target before the `play` starts, and they share one target per object **[src]** |

### 6.6 Slides, decks, presentations

A **clip** `χ = ⟨sc, F⟩` is a slide configuration with its frames. A **presentation**
`Π = ⟨cfg, χ̄⟩` is the deck configuration and a list of clips.

```
 ⟨B, ρ, W⟩ ⇓ ⟨ρ′, W₁, F⟩
 G = wait(cfg.gap)  if cfg.gap > 0 and (¬sc.loop or cfg.gap_on_loops),  else nothing
 ⟨G, ρ′, W₁⟩ ⇓ ⟨ρ′, W′, F_G⟩
 ──────────────────────────────────────────────────────── E-Slide
 ⟨slide(sc, B), ρ, W⟩ ⇓ ⟨ρ′, W′, ⟨sc, F · F_G⟩⟩

 ρ₀ = ∅     W₀ = ⟨∅, ε⟩     ⟨Sᵢ, ρᵢ₋₁, Wᵢ₋₁⟩ ⇓ ⟨ρᵢ, Wᵢ, χᵢ⟩   for i = 1 … n
 ──────────────────────────────────────────────────────────────── E-Deck
 deck(cfg, S₁ … Sₙ) ⇓ ⟨cfg, [ χᵢ | ¬scᵢ.skip ]⟩
```

`media(sc, s)` contributes the file's frames and leaves the world alone. A skipped slide
still runs — later slides need its effect on the world — but its clip is dropped.

The gap answers Q1: without it, the frame a slide rests on is the one *before* its last
animation finished. manim-slides has the same device (`wait_time_between_slides`) **[src]**,
but applies it with a bare `self.wait` and not to the last slide, so the translation emits
the gap itself (§7).

**Presenting** is a small transition system over `⟨i, d, m⟩`: `i` is the current clip,
`d ∈ {→, ←}` says whether its frames run forwards or as `reverse(Fᵢ)`, and
`m ∈ {▶ running, ‖ held, ■ resting on the last frame of that run}`. A newly loaded clip
starts at its first frame; `→` runs at `scᵢ.playback_rate`, `←` at
`scᵢ.reversed_playback_rate`.

```
start                            ⟨1, →, ▶⟩
run ends,  scᵢ.loop              ⟨i, d, ▶⟩ ⟶ ⟨i, d, ▶⟩        from its first frame, for ever
run ends,  scᵢ.auto_next         ⟨i, →, ▶⟩ ⟶ ⟨i+1, →, ▶⟩      i < n
                                 ⟨i, ←, ▶⟩ ⟶ ⟨i, →, ▶⟩
run ends,  otherwise             ⟨i, d, ▶⟩ ⟶ ⟨i, d, ■⟩
NEXT                             ⟨i, d, ‖⟩ ⟶ ⟨i, d, ▶⟩        carries on where it was held
                                 ⟨i, →, m⟩ ⟶ ⟨i+1, →, ▶⟩      m ≠ ‖,  i < n
                                 ⟨i, ←, m⟩ ⟶ ⟨i, →, ▶⟩        m ≠ ‖
PREVIOUS                         ⟨i, d, m⟩ ⟶ ⟨i−1, →, ▶⟩      i > 1
REVERSE                          ⟨i, d, m⟩ ⟶ ⟨i−1, ←, ▶⟩      i > 1
REPLAY                           ⟨i, d, m⟩ ⟶ ⟨i, d, ▶⟩        from its first frame
PLAY/PAUSE                       ⟨i, d, ▶⟩ ⟶ ⟨i, d, ‖⟩        ⟨i, d, ‖⟩ ⟶ ⟨i, d, ▶⟩
```

Where no rule applies, nothing happens. These are manim-slides' transitions, read from
`present/player.py` and `config.py` **[src]**; four of them are not what the names suggest:

- `loop` beats `auto_next`: a looping clip never ends, so it never moves on by itself.
  (The RevealJS export decides the other way: `data-autoslide` moves on after one pass.)
- `NEXT` does not wait for the clip to finish — it loads the next one at once — and while
  the clip is held it only lets it go.
- `REVERSE` rewinds the clip *before* the current one: from the picture the current clip
  starts with, back to the one that clip started with. `NEXT` then plays that same clip
  forwards, so `REVERSE`, `NEXT` is "show me that step again".
- `PLAY/PAUSE` cannot restart a clip that has come to rest; that is `REPLAY`.

The player's command line changes the table in four ways **[src]**:

```
--start-paused            start is ⟨1, →, ‖⟩ instead of ⟨1, →, ▶⟩
--next-terminates-loop    NEXT on a running clip with scᵢ.loop marks the round as its last:
                          when the run ends it is  ⟨i, d, ▶⟩ ⟶ ⟨i, d, ■⟩,  or the auto_next
                          transition if scᵢ.auto_next. A NEXT at ■ moves on as usual.
--exit-after-last-slide   NEXT (or auto_next) at ⟨n, →, ·⟩ ends the presentation
--skip-all                every clip is treated as  ¬loop  and  auto_next
```

Suit's own presenter follows the table and offers the same four choices. It differs in
three things. It starts at the selected slide (`⟨k, →, ·⟩`). At the first clip `PREVIOUS`
and `REVERSE` do nothing at all, where manim-slides turns its reversed flag on or off before
it finds there is nothing to load. And a clip that is not recorded yet is waited for; its
clock starts when it arrives.

### 6.7 Properties

Stated, not yet proved. P1–P3 are routine inductions; P4–P6 are what the test suite exists
for. `tools/verify_render.py` checks P3 on a real render: on the sample deck all six slides
come out with exactly the predicted number of frames.

- **P1 Totality.** If `⊢ D` and `D` contains no `raw`, then `D ⇓ Π` for exactly one `Π`.
- **P2 Presence soundness.** Say `W ⊨ Γ` when, for every place, `state(Γ(p)) = on` iff every
  member of `fam(ρ(p))` that has points is in `fam(Δ)`, and `off` iff none is. If `W ⊨ Γ`,
  `Γ ⊢ c ⊣ Γ′` and `⟨c, ρ, W⟩ ⇓ ⟨ρ′, W′, F⟩`, then `W′ ⊨ Γ′`. *The checker's picture of the
  scene is the scene.*
- **P3 Timing.** Under the same hypotheses, `|F| = frames(c)`. *The timeline is exact.*
- **P4 Layer discipline.** If no two members of `fam(Δ)` belonging to different roots share
  a `z`, then `frame(W)` does not depend on the order of `Δ`. *With explicit layers, Q5 and
  the history of `add` calls become invisible.*
- **P5 Slide locality.** The clip `χᵢ` is a function of the slide `Sᵢ`, the entry environment
  `ρᵢ₋₁`, the entry world `Wᵢ₋₁` and `cfg` only. *A clip can be cached under a hash of those
  four, and slides can be rendered independently once their entry world is known.*
- **P6 Adequacy.** For well-typed `D` satisfying P4, rendering `⟦D⟧` (§7) with the pinned
  versions yields `Π`, up to rasterisation. *Export means what the editor shows.*

Known gap in P2/P6: updaters attached to a group whose members were later removed and re-added
one by one stop running in Manim, because the group itself is no longer listed in `Δ`. The
checker should warn; the rules do not yet exclude it.

---

## 7. Translation to Python

Compositional, one Manim call per node.

| MC | Python |
|---|---|
| `deck(cfg, S̄)` | `config.pixel_width = …` etc. · `class Deck(Slide):` · `def construct(self):` · `⟦S₁⟧ … ⟦Sₙ⟧` |
| `slide(sc, B)` | `self.next_slide(loop=…, auto_next=…, notes=…, …)` · `⟦B⟧` · `⟦wait(gap)⟧` when the slide gets a gap (E-Slide). The first slide needs no `next_slide()` unless it has settings. |
| `media(sc, s)` | `self.next_slide(src=s, …)` |
| `let x = m` | `x_17 = ⟦m⟧` then `x_17.set_z_index(layer)`; plus `self.add(x_17)` when grouping on-scene objects |
| `p.f(l̄ = w̄)` | `⟦p⟧.f(l=⟦w⟧, …)` |
| `add(p̄)` / `remove(p̄)` | `self.add(…)` / `self.remove(…)` |
| `play(ā)` | `self.play(⟦a₁⟧, …)` |
| `wait(d)` | `self.wait(d)`; with `frozen_frame=False` if any updater is attached to something on scene |
| `attach(p, always f(w̄))` | `⟦p⟧.add_updater(lambda m: m.f(…))` |
| `attach(p, every dt. f(w̄))` | `⟦p⟧.add_updater(lambda m, dt: m.f(…))` |
| `detach(p)` | `⟦p⟧.clear_updaters()` |
| `A(l̄ = w̄) @ o` | `A(…, run_time=dur, rate_func=…, lag_ratio=…)` — `run_time` always, the others when set |
| `p.animate(μ̄) @ o` | `⟦p⟧.animate(run_time=…, …).f₁(…).f₂(…)` |
| `par(ā) @ o` | `AnimationGroup(…, lag_ratio=…, …)` |
| `seq(ā) @ o` | `Succession(…)` |
| `x[i]`, `copy(p)`, `group(p̄)`, `vgroup(p̄)` | `x_17[i]`, `p.copy()`, `Group(…)`, `VGroup(…)` |
| `λx. e`, `p.q(…)` | `lambda x: ⟦e⟧`, `p.q(…)` |
| `raw⟨ι⟩(s)` | `s`, verbatim |

Four things the translation does on purpose:

- **Explicit layers.** Every constructed object gets `set_z_index` (which reaches its whole
  family) with a number of its own, establishing P4, so Q5 cannot be seen. A copy of a group
  gets fresh layers that keep its members' order.
- **`frozen_frame=False`** on every `wait` during which updaters are live. This is what makes
  Manim follow E-Wait-Live where MC says so.
- **The gap as an explicit `wait`** at the end of each slide, rather than through
  `wait_time_between_slides`, so that it obeys the same still-or-live rule and the last slide
  gets one too.
- **An explicit `run_time` on every primitive and every `animate`.** `Write` and
  `AddTextLetterByLetter` otherwise pick a length from the text, which would make durations
  depend on a render. Composites get one only when it was set; theirs follows from their
  children.

Variables become Python names made of the display name and the variable's id (`circle_17`),
which are unique and cannot shadow anything Manim exports.

`skip` maps to `next_slide(skip_animations=True)`. `cfg` maps to Manim's configuration
(`pixel_width`, `pixel_height`, `frame_rate`, `background_color`).

---

## 8. Outside version 0.1

| Missing | Plan |
|---|---|
| Camera pan and zoom | The camera frame of `MovingCameraScene` *is* a mobject. Add a predeclared variable `camera : Mob Frame @ on`; `camera.animate(…)` then needs no new syntax. |
| 3D (`ThreeDSlide`) | A second scene class with its own Σ entries. |
| Loops and user functions | Surface sugar that elaborates to MC by unrolling; bounds must be literals, preserving totality. |
| Canvas (title, slide number) | Nothing to add: objects left `on` across slides. The editor can present them as a "master" layer. `Wipe` and `Zoom` are catalogue animations. |
| `wait_until`, stop conditions | Excluded for good: they make duration dynamic. Reachable through `raw`. |
| Sound, subcaptions | Commands with no effect on frames; straightforward to add. |
| Custom rate functions | `Rate` could admit `λt. e`; needs the clamp of §6.4 made explicit. |

---

## Appendix A — starting catalogue

Enough to make real slides. It lives in `crates/sig/src/builtin.rs`; `tools/check_catalog.py`
checks every kind, parameter name, default and constant in it against the installed Manim.
Roles for animations are from the `introducer` / `remover` flags and clean-up code **[src]**,
and the ones that matter most are confirmed by `tools/verify_quirks.py` **[run]**.

**Kinds.** `Mobject` ⊃ `VMobject` ⊃ { `Circle`, `Dot`, `Ellipse`, `Square`, `Rectangle`,
`RoundedRectangle`, `Triangle`, `RegularPolygon`, `Polygon`, `Star`, `Line`, `Arrow`,
`DoubleArrow`, `DashedLine`, `Arc`, `Brace`, `SurroundingRectangle`, `Text`, `MarkupText`,
`MathTex`, `Tex`, `DecimalNumber`, `Integer`, `NumberLine`, `Axes`, `NumberPlane`,
`FunctionGraph`, `ParametricFunction`, `Table`, `Code`, `VGroup` }; `Mobject` ⊃ `Group`,
`ImageMobject`, `ValueTracker`.

**Methods** (on `Mobject` unless noted). `shift`, `move_to`, `next_to`, `to_edge`,
`to_corner`, `align_to`, `scale`, `rotate`, `flip`, `stretch`, `set_color`, `set_opacity`,
`set_z_index`, `arrange` (containers); `set_fill`, `set_stroke` (`VMobject`); `set_value`
(`ValueTracker`, `DecimalNumber`).

**Queries.** `get_center`, `get_top`, `get_bottom`, `get_left`, `get_right`, `get_corner`,
`get_width`, `get_height` (`Mobject`); `point_from_proportion` (`VMobject`); `get_value`
(`ValueTracker`); `c2p` (`Axes`); `n2p` (`NumberLine`).

**Animations.**

| Animation | Object parameters and roles | Requires |
|---|---|---|
| `FadeIn` | `intro` | — |
| `FadeOut` | `outro` | — |
| `Create`, `DrawBorderThenFill`, `Write` | `intro` | `VMobject` |
| `Uncreate`, `Unwrite` | `consume` **[run]** | `VMobject` |
| `GrowFromCenter`, `GrowFromPoint`, `GrowFromEdge`, `SpinInFromNothing` | `intro` | — |
| `GrowArrow` | `intro` | `Arrow` |
| `AddTextLetterByLetter` | `intro` | `Text` |
| `RemoveTextLetterByLetter` | `consume` **[open]**; not in the catalogue yet | `Text` |
| `ShowPassingFlash` | `flash` | `VMobject` |
| `Transform` | source `mut` (kind weakened), target `use` | — |
| `ReplacementTransform` | source `consume`, target `intro` | — |
| `TransformFromCopy` | source `use`, target `intro` | — |
| `FadeTransform` | source `outro`, target `intro` | — |
| `TransformMatchingShapes` | source `outro`, target `intro` | — |
| `TransformMatchingTex` | source `outro`, target `intro` | `MathTex` both |
| `Indicate`, `Wiggle`, `ApplyWave`, `Rotate` | `mut` | — |
| `Circumscribe` | `use`: it draws a frame around the object and leaves the object alone | — |
| `MoveAlongPath` | object `mut`, path `use` | path: `VMobject` |
| `Wipe`, `Zoom` (manim-slides) | current… `outro`, future… `intro`; not in the catalogue yet | — |

Verified defaults: `Animation` run time 1, rate `smooth`, lag 0; `AnimationGroup` rate
`linear`, lag 0; `Succession` lag 1; `LaggedStart` lag 0.05; `Wait` run time 1, rate `linear`;
`Add` run time 0 **[src]**. All other per-animation defaults are to come from the generated
catalogue, not from memory.
