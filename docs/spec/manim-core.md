# Manim Core — abstract syntax, AST, type system and semantics

Draft 0.11 · 2026-10-14 · pinned to **manim 0.21.0** and **manim-slides 5.7.0**

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
Σ = ⟨ K, ≤, elem, inside, outline, Ctor, Meth, Query, Anim, Rate, Fn, Const ⟩
```

| Component | Content | Examples |
|---|---|---|
| `K`, `≤` | Kinds: Manim's mobject classes, ordered by subclassing. A tree with root `Mobject`. | `Circle ≤ Arc ≤ VMobject ≤ Mobject` |
| `elem : K ⇀ K` | Kind of the children of a container kind | `elem(VGroup) = VMobject`, `elem(MathTex) = VMobject` |
| `inside : K ⇀ {open, clear, filled}` | What a kind has for an inside, where a fill colour would show | `inside(Line) = open`, `inside(Circle) = clear`, `inside(Dot) = filled` |
| `outline : K ⇀ {drawn, bare}` | Whether a kind starts with an outline, where a stroke colour and a stroke opacity would show | `outline(Circle) = drawn`, `outline(Text) = bare` |
| `Ctor` | `C : (l̄ : τ̄ = defaults) → κ` | `Circle : (radius: Real = 1, color: Color = …) → Circle` |
| `Meth` | `f : (κ ; l̄ : τ̄)` — mutators; the receiver keeps its kind | `shift : (Mobject; by: Vec)`, `set_color : (Mobject; color: Color)` |
| `Query` | `q : (κ ; l̄ : β̄) → β` — pure reads | `get_center : (Mobject) → Vec`, `c2p : (Axes; x, y: Real) → Vec` |
| `Anim` | `A : (l̄ : ς̄)`, each `ς` a value type or an object *role* (§5.4); plus default run time, rate, lag | `FadeIn : (mobject: intro Mobject, shift: Vec = 0)` |
| `Rate` | Named rate functions `[0,1] → ℝ` | `smooth`, `linear`, `there_and_back`, `wiggle`, the thirty `ease_*` functions … |
| `Fn`, `Const` | Pure functions, operators and constants | `sin`, `+`, `<` ; `UP`, `PI`, `BLUE` |

Parameters are inherited along `≤` (Manim forwards `**kwargs` to the superclass), so
`VMobject`'s `fill_opacity`, `stroke_width`, … are available on every vector kind **[src]**.

Available is not the same as seen. `inside` says what a fill colour given to a new object
does, and is inherited along `≤` as well, down to the kind that says otherwise:

- `open`: there is no inside. `Line` and the kinds below it, `ParametricFunction`, `Plot`,
  `Axes`. A fill colour shows nothing; `color` colours the line itself (except on `Axes`,
  which are made of parts: see below).
- `clear`: an inside that starts fully transparent, which is every other vector kind:
  Manim's `fill_opacity` is 0 unless a class says otherwise **[src]**. A fill colour shows
  once a fill opacity comes with it.
- `filled`: `Dot`, `Brace`, `Text`, `MarkupText`, `MathTex`, `DecimalNumber` and the kinds
  below them. A fill colour shows as it is.

It changes no rule of §5: `fill_color` stays a parameter of every vector kind, as it is in
Manim. It is what the warning of §5.8 goes by, and what an editor offers settings by.
`tools/check_catalog.py` makes an object of every constructor with and without a fill
colour and compares what can be seen of the two **[run]**.

`outline` says the same of a stroke colour and a stroke opacity, and is inherited along `≤`
in the same way:

- `drawn`: some part starts with a stroke width above 0, which is every vector kind but
  those below. A stroke colour shows as it is (except on `Axes`: see below).
- `bare`: every part starts with a stroke width of 0 **[src]**: `Dot`, `Brace`, `Text`,
  `MarkupText`, `MathTex`, `DecimalNumber` and the kinds below them, the same as those
  whose inside is `filled`. A stroke colour or a stroke opacity draws no outline, whether
  it is given to the new object or by `set_stroke(color = …)` or `set_stroke(opacity = …)`
  later. Both show once there is a stroke width: given with them, or by
  `set_stroke(width = …)` at any time. On `MathTex` and `Tex` Manim also colours the letters
  with a stroke colour given to the new object, unless a fill colour is given too; that
  shows, but it is still no outline **[run]**.

It changes no rule of §5 either. It is what the warning of §5.8 about an outline goes by,
and an editor gives a stroke colour or opacity that is chosen for a `bare` object the
stroke width that shows it. `tools/check_catalog.py` checks that a `bare` object starts
without an outline and a `drawn` one with one; that on a `bare` one a stroke colour or
opacity alone, given to the new object or by `set_stroke`, shows nothing (but for the
letters above), and that with a stroke width, given then or later, each shows; and that on
a `drawn` one a stroke colour shows **[run]**.

Nor does every setting reach what is drawn. A kind `κ ≤ VGroup` is *made of parts*: Manim
makes the parts from the constructor's own parameters, and hands the ones it inherits along
`≤` to the group, which draws nothing itself **[src]**. A kind `κ ≰ VMobject` is not a
vector object, and Manim draws it without the parameters it inherits: `Mobject` keeps
`color` and applies it nowhere, which only `VMobject` does **[src]**. Write `unseen(C)` for
these: the inherited parameters of `C` if its kind is `≤ VGroup` or `≰ VMobject`, and none
otherwise. The catalogue has two constructors made of parts, `Axes` and `NumberPlane`, and
for both `unseen` is `color` with the fill and the stroke settings. None of the six shows
anything there, alone or with the others **[run]**. It has two that are not vector objects,
`ImageMobject` and `ValueTracker`, and for both `unseen` is `color`: an image keeps the
colours of its pixels, and a tracker is never drawn **[run]**.

What a setting says of a new vector object, a method says of a finished one, and a method
goes to every part of its receiver:

| Setting | The call that reaches the parts |
|---|---|
| `color = e` | `set_color(color = e)` |
| `stroke_color = e` | `set_stroke(color = e)` |
| `stroke_width = e` | `set_stroke(width = e)` |
| `stroke_opacity = e` | `set_stroke(opacity = e)` |

The fill settings have none that would show: the parts are lines. Nor has `color` on what
is not a vector object: `set_color` paints every pixel of an image in the colour, and shows
nothing on a tracker. Where both calls are made, `set_color` comes first. Among settings
`stroke_color` counts over `color` wherever the two are written, and of two calls the later
one counts **[run]**.

As with `inside`, no rule of §5 changes: the six stay parameters of `Axes`, and `color` one
of `ImageMobject`, as they are in Manim. `unseen` is what the second warning of §5.8 about
a look goes by, and an editor does not offer what is in it. `tools/check_catalog.py` makes
every object whose `unseen` is not empty with each of these settings and with all of them,
and those made of parts with each call of the table, and compares what can be seen with
the plain object, the pixels of an image included; and it makes every other object with
and without `color`, which shows on all of them **[run]**.

A parameter also says how its arguments reach Python: by position, by name, as *the rest*
(`*args`: any number, by position), or as a *list* (any number, passed together by name).
The last two are the only parameters a call may give more than one argument.

**One constructor is not Manim's.** `Plot : (function: Real → Real, on: list of Vec,
step: Real, asymptotes: Bool) → Plot`, with `Plot ≤ VMobject`, is the graph of a rule
drawn where the rule has a value. Manim's own `FunctionGraph` works the rule out at every
point of its range and joins up whatever comes out: where the rule has no value it draws
none of the graph, and across a pole it joins the two branches by a stroke from the bottom
of the picture to the top **[run]**. `Plot` looks first. Its interpretation is not in
Manim but in a hundred and fifty lines of Python that the translation writes into every file
that uses it
(§7), so it is as much part of the reference interpretation as anything of Manim's:

- it samples the rule on each stretch of `on` (each a pair of numbers; without `on`, the
  width of the frame) and keeps the runs where the value is a number no further than two
  frame heights from the middle, each run a stroke of its own;
- a step out of all proportion to the steps around it ends a run as well, so a staircase is
  drawn without risers;
- where a gap is one the rule runs off into — its values grow without end towards a point,
  or it stops having values at a point it was heading off at — that point is a vertical
  asymptote; a level the rule settles on far out to either side is a horizontal one. With
  `asymptotes` they are drawn, dashed.

What it found — the stretches drawn, whether the rule lacked a value anywhere it was told to
look, the asymptotes — is reported by the probes with the scene, which is how the editor can
say “the function `log(x)` is not defined on all of [-7, 7]” instead of drawing nothing.

**Nor are a second constructor, a method, a query and an animation**, all four for showing
what a derivative is. They are defined the same way, next to `Plot`:

```
Tangent : (graph: Plot, at: Real = 0, length: Real = 3, dot: Bool = false) → Tangent      Tangent ≤ Line
touch   : (Tangent; x: Real)
slope   : (Tangent) → Real
Glide   : (mobject: mut Tangent, to: Real)                run time 1, rate smooth
```

`Tangent(g, at = x)` is the segment of the given length, centred on the graph's point at
`x`, with the graph's steepness there. The steepness is not asked of anyone: it is the
central difference of `g`'s rule over a step of `10⁻⁵·max(1, |x|)`, so the rule alone
decides it. `touch(x)` puts the segment at another `x`, and `slope` reads the steepness
where it is now. Where the rule has no value, `touch` leaves the segment where it was; a
tangent that never had a place is reported like a `Plot`'s findings.

`Glide` exists because `p.animate(touch(x))` is not what it seems (compare the `rotate`
row of §5.8): `.animate` moves the two ends of the segment straight to where they end up,
and between its first frame and its last the segment touches nothing. `Glide` is defined
on the parameter instead: at progress `α` it is `touch(x₀ + ϱ(α)·(x − x₀))`, with `x₀` where
the tangent was when the animation began. Every frame of it is a tangent **[run]**.

A graph that has been shifted or stretched after it was drawn takes its tangents with it:
the point and the steepness are carried over by the same shift and stretch.

An **interpretation** `𝓘` of Σ gives each symbol its meaning: `𝓘(C)` builds points and style,
`𝓘(f)` transforms an object, `𝓘(q)` reads one, `𝓘(A)` supplies the per-frame step of an
animation. The *reference interpretation* is Manim itself. §6 is generic in `𝓘`; a native Rust
preview would be a second, partial interpretation that must agree with the reference one.

The editor's canvas uses the reference interpretation directly rather than a second one. A
generated *probe* script (`suit-cli probe`) runs the deck in Manim with animations skipped to
their end state and reports, after every command, the outline, fill and stroke of everything
on scene and which variable each outline belongs to. The canvas rasterises those outlines, so
what it shows for text, formulas and graphs is what Manim will draw. A picture has no
outline; it is reported as its pixels, three of its corners, and how opaque it is. Manim's
camera draws it from the pixels and all four corners **[src]**; the fourth follows from the
other three unless the picture has been warped by something that is not linear, which
nothing in the catalogue does. The one difference from a real render is the
one skipping makes: a time-based updater sees a single long frame per step instead of many
short ones.

To *play* a slide, a second probe (`suit-cli probe --frames N`) runs the slides before it
skipped and then plays that one for real, frame for frame, writing down what is drawn in
each frame instead of drawing it. The canvas plays those frames back, so the motion is
Manim's own — its interpolation, rate functions, lags and updaters — without a video in
between. It is also a standing check of P3 (§6.7): the recording has exactly `frames(B)`
frames, plus the gap, or the tests fail. A still `wait` is the one thing not passed to Manim
as is: recording makes Manim treat every wait as live, so its frames are written down as
one frame repeated, which is what E-Wait-Still says they are.

Neither probe changes what a deck means, and both can be given up half-way: they ask, after
every command and at every frame, whether their result is still wanted. What a slide's
frames depend on is the deck's configuration and the commands of that slide and of the
slides before it (E-Deck threads `ρ` and `W` forwards only), so a recording stays good
through every change to a later slide, to a title, or to a note.

The same fact is what makes the canvas quick. E-Deck runs slide `i` from `⟨ρᵢ₋₁, Wᵢ₋₁⟩` and
from nothing else, so the Manim that runs the probes keeps that pair for every `i` — the
deck's variables and the scene, saved before each slide under a name made of the
configuration and of the text of the slides run to get there. A probe starts from the
furthest saved pair whose name fits the deck it is given and runs the slides from there:
after a change to slide `k`, slides `1 … k−1` are not run again. A slide that was *played*
for a recording is never saved from, because it has been through other frames than one that
was skipped through (Q3), and the two need not end in the same state. Nor is every
animation of a probe given to Manim to skip: one whose end state is plain (an entrance, a
FadeOut, an indication there and back, each with an easing that ends where its kind needs)
is not run at all, and the scene is put in that state directly. The tests hold all of this
to the definition: after a long run of edits and undos, what the editor shows must be what a
Manim that has never seen the deck makes of it, and a probe that skips those animations must
report every step as one that gives each to Manim (`SUIT_PROBE_EXACT`).

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

### 3.1 Formulas: a concrete syntax for `Expr`

Everything above is abstract. Expressions are the one category people write by hand, in the
editor's settings, so they have a concrete syntax: that of a spreadsheet rather than of
Python. `crates/codegen/src/formula.rs` reads and writes it.

```
e ::= if e then e else e
    | e or e  |  e and e  |  not e
    | e < e  |  e <= e  |  e > e  |  e >= e  |  e = e  |  e != e
    | e + e  |  e - e  |  e * e  |  e / e  |  - e  |  e ^ e
    | h(e, …)                                a function of Σ: sin(x), max(a, b)
    | e.x | e.y | e.z                        a component of a point
    | p.q | p.q(e, …) | p.q(l = e, …)        ask an object: circle.center, axes.c2p(1, 2)
    | (e, e) | (e, e, e)                     a point or direction; two components mean z = 0
    | 1.5 | 2 | true | "text" | #58c4dd      literals; 2 is an integer, 2.0 a real
    | k                                      a constant of Σ: UP, PI, BLUE
    | name | `name` | p[i]                   a value or an object, and the parts of an object
    | (e)
```

Listed loosest first; `+ - * /` and `and`, `or` associate to the left, comparisons do not
chain, and `-2 ^ 2` is `-(2 ^ 2)`. Each operator is the function of Σ of the same name
(`=` is `==`, `^` is `pow`, a leading `-` is `neg`), as §3 says operators are.

- **Names** are display names, so they may be several words long: `dot 2`. Where several
  words could be a name, the longest name wins. A name that could not be told from something
  else — it contains other characters, or a word like `and`, or is spelled like a constant —
  is written in backticks.
- A name stands for the innermost value of that name (a parameter, then a named value), else
  for the newest object of that name.
- **Asking an object.** `p.q` is the query `q` of Σ, or `get_q` if there is no `q`:
  `circle.center` is `circle.get_center()`. Arguments go by position or by parameter name.
  Of anything that is not an object, `.x`, `.y`, `.z` are components instead.
- `⟨e, e, e⟩` is read as a point too, so the notation of §3 can be typed as it stands.

**Stretches** of the number line, which a `Plot` is told to draw on, are written as a
mathematician writes them and read forgivingly: `[-1, -0.1] ∪ [0.1, 1]`, with `;` for the
comma and `U`, `union` or `or` for `∪` if that is what is typed. The two ends of each are
formulas.

Writing is the inverse of reading: `read(write(e)) = e` for every expression, which is what
lets a formula be shown, edited, and put back. Whether it makes sense — that `sin` is given a
number — is not syntax; it is §5.2, as for every other expression.

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
| an object with a point that is not a number | Manim draws none of that object, and says nothing **[run]**; found by running the deck, not by the checker |
| `let x = C(…, fill_color = e)` with no `fill_opacity`, where `unseen(C)` is empty and `inside(C)` is `open`, or is `clear` and nothing in the deck calls `set_opacity` or `set_fill(opacity = …)` on `x`, a part of it, or a group that holds it | Manim takes the colour and shows nothing of it (§2) **[run]** |
| `let x = C(…, l = e)` with `l ∈ unseen(C)` | Manim takes the setting and draws `x` without it: no part gets it, or `x` is not a vector object (§2) **[run]**; reported once for `x`, with the calls that do reach the parts where there are any |
| `let x = C(…)` with `outline(C) = bare` and no `stroke_width`, where `x` is given `stroke_color` or `stroke_opacity` by `C`, or by a `set_stroke` without `width` called on the whole of `x` (straight after `C`, or later with the kind of `x` still `bare`), and nothing in the deck calls `set_stroke(width = …)` on `x`, a part of it, or a group that holds it | Manim takes them and draws no outline for them to show on (§2) **[run]**; reported where each is given |
| a `Plot` told to draw on stretches where its rule has no value | it draws where it has one; reported with the rule and the stretches |
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
| a list parameter given `w₁ … wₖ` | `l=[⟦w₁⟧, …, ⟦wₖ⟧]` |
| `Plot(…)` | `Plot(…)`, preceded once in the file by `class Plot(VMobject): …` (§2) |
| `Tangent(…)`, `Glide(…)` | themselves, preceded once by `class Tangent(Line): …` and `class Glide(Animation): …` |
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

A deck names a **picture** relative to its own folder when the picture lies in it, so that
the folder can be moved as a whole. The exported file then sets `config.assets_dir` to the
folder it is in, which is where Manim looks for a file it is given by a relative name
**[src]**, and the probes are given every name in full.

A fifth is a precaution rather than a rule of Manim's: **points are arrays of reals.**
`⟨e₁, e₂, e₃⟩` becomes `np.array([…])` with integer literals written as reals, and with
`dtype=float` when a component is computed. An array of integers refuses real arithmetic in
place **[run]**, and nothing promises that Manim never does that to a point it is handed.

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

Enough to make real slides. It lives in `crates/sig/src/builtin.rs`, and `suit-cli signature`
prints all of it: 37 kinds, 32 of them with a constructor, 22 methods and 17 queries (a name
counted once), 31 animations, 45 rate functions, 28 functions and 103 constants.
`tools/check_catalog.py` checks every kind and its parent, every parameter name, every
method and query, the defaults of the animations, and every rate function, constant,
inside and outline in it against the installed Manim, and what is `unseen` on the kinds
that are made of parts or are not vector objects.
Roles for animations are from the `introducer` / `remover` flags and clean-up code **[src]**,
and the ones that matter most are confirmed by `tools/verify_quirks.py` **[run]**.

**Kinds.** Each stands under its parent, which is how `≤` is read off.

```
Mobject                               no constructor
  Group                               no constructor: made by group(p̄) (§3)
  ImageMobject
  ValueTracker
  VMobject                            no constructor
    VGroup                            no constructor: made by vgroup(p̄) (§3)
      Axes
        NumberPlane
    Arc
      Circle
        Dot
        Ellipse
    Polygram                          no constructor: Manim's parent of the two below
      Polygon
        Rectangle
          Square
          RoundedRectangle
            SurroundingRectangle
        Star
      RegularPolygon
        Triangle
    Line
      Arrow
        DoubleArrow
      DashedLine
      NumberLine
      Tangent                         Suit's own (§2)
    Brace
    Text
    MarkupText
    MathTex
      Tex
    DecimalNumber
      Integer
    ParametricFunction
      FunctionGraph
    Plot                              Suit's own (§2)
```

The other 32 kinds each have a constructor of their own name. Every one of them takes
`color`, which is `Mobject`'s, and those below `VMobject` take its `fill_color`,
`fill_opacity`, `stroke_color`, `stroke_opacity` and `stroke_width` as well (§2). The two
below `VGroup` take all six and show none of them: they are `unseen` there (§2).
`ImageMobject` and `ValueTracker` take `color` and show nothing of it either (§2).
`elem(Mobject) = Mobject` and `elem(VMobject) = VMobject`, inherited the same way. With
`Plot` and `Tangent` come the method `touch`, the query `slope` and the animation `Glide`
(`mut`, on a `Tangent`); the catalogue check holds these to their own definitions instead of
to Manim.

**Methods.** On `Mobject`: `shift`, `move_to`, `next_to`, `to_edge`, `to_corner`,
`align_to`, `set_x`, `set_y`, `scale`, `scale_to_fit_width`, `scale_to_fit_height`, `rotate`,
`flip`, `stretch`, `arrange`, `set_color`, `become`. On `VMobject`: `set_opacity`,
`set_fill`, `set_stroke`. `set_value` on `ValueTracker` and on `DecimalNumber`, whose
parameter Manim calls `value` on the one and `number` on the other. `touch` on `Tangent`.

- `arrange` lines up the children of its receiver, so it is of use on a group; it is a
  method of every `Mobject` all the same, as it is in Manim.
- `become` gives its receiver the geometry of another object. It is the one exception to
  "the receiver keeps its kind" (§2): afterwards the kind is the least common ancestor of
  the two and indexing into the receiver is no longer allowed, exactly as for the source of
  a `Transform` (§5.4).
- `set_z_index` is Manim's and is deliberately not a method of Σ: the layers are given out
  by the translation (§7), which is what P4 rests on.

**Queries.** On `Mobject`, to a `Vec`: `get_center`, `get_top`, `get_bottom`, `get_left`,
`get_right`, `get_corner`, `get_start`, `get_end`; to a `Real`: `get_x`, `get_y`, `width`,
`height`. `point_from_proportion` (`VMobject`); `get_value` (`ValueTracker`,
`DecimalNumber`); `c2p` (`Axes`); `n2p` (`NumberLine`); `slope` (`Tangent`).

`width`, `height` and `slope` are *properties*: Python reads them without brackets, `p.width`
where another query is `p.get_center()`. The catalogue says of every query which of the two
it is, and the catalogue check holds it to that. In a formula they are all asked the same
way (§3.1).

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
| `ShowPassingFlash` | `flash` | `VMobject` |
| `Transform` | source `mut` (kind weakened), target `use` | — |
| `ReplacementTransform` | source `consume`, target `intro` | — |
| `TransformFromCopy` | source `use`, target `intro` | — |
| `FadeTransform` | source `outro`, target `intro` | — |
| `TransformMatchingShapes` | source `outro`, target `intro` | — |
| `TransformMatchingTex` | source `outro`, target `intro` | `MathTex` both |
| `Indicate`, `Wiggle`, `ApplyWave`, `Rotate`, `ScaleInPlace`, `FadeToColor` | `mut` | — |
| `Circumscribe` | `use`: it draws a frame around the object and leaves the object alone | — |
| `MoveAlongPath` | object `mut`, path `use` | path: `VMobject` |
| `Glide` | `mut` | `Tangent` |
| `Flash`, `FocusOn` | none: what they are given is a value, a point or an object standing for its centre | — |

Not in the catalogue yet: `RemoveTextLetterByLetter` (`consume` **[open]**, on a `Text`), and
`Wipe` and `Zoom` of manim-slides (current… `outro`, future… `intro`).

Verified defaults: `Animation` run time 1, rate `smooth`, lag 0; `AnimationGroup` rate
`linear`, lag 0; `Succession` lag 1; `LaggedStart` lag 0.05; `Wait` run time 1, rate `linear`;
`Add` run time 0 **[src]**. Every other default is in the catalogue, and the catalogue check
compares it with the one in Manim's signature wherever the signature states one. They are
`Animation`'s three, except:

- run time 2 for `DrawBorderThenFill`, `Wiggle`, `ApplyWave` and `FocusOn`;
- rate `linear` for `Write`, `Unwrite`, `AddTextLetterByLetter`, `Flash`, `Circumscribe`,
  `TransformMatchingShapes` and `TransformMatchingTex`; `double_smooth` for
  `DrawBorderThenFill`; `there_and_back` for `Indicate`;
- lag 1 for `Create`, `Uncreate` and `Circumscribe`.

For five animations the run time 1 is the catalogue's own, which Manim's signature does not
state: `Write`, `Unwrite` and `AddTextLetterByLetter` pick a length from the object they are
given, and the two `TransformMatching…` take theirs from their parts. It is what is rendered
all the same, because the translation always passes a run time (§7).

**Rate functions.** By the names Manim exports: `linear`, `smooth`, `smoothstep`,
`smootherstep`, `smoothererstep`, `rush_into`, `rush_from`, `slow_into`, `double_smooth`,
`there_and_back`, `there_and_back_with_pause`, `running_start`, `wiggle`, `lingering`,
`exponential_decay`. And the thirty `ease_in_…`, `ease_out_…` and `ease_in_out_…` of `sine`,
`quad`, `cubic`, `quart`, `quint`, `expo`, `circ`, `back`, `elastic` and `bounce`, which Manim
does not export by name: Python is given `rate_functions.ease_in_sine`.

**Functions.** With their types, since an operator stands for several. The first that fits
is taken, so the integer forms come first (`Int ≤ Real`, §5.1).

| Function | Types |
|---|---|
| `+`, `-` | `Int × Int → Int`, `Real × Real → Real`, `Vec × Vec → Vec` |
| `*` | `Int × Int → Int`, `Real × Real → Real`, `Real × Vec → Vec`, `Vec × Real → Vec` |
| `/` | `Real × Real → Real`, `Vec × Real → Vec` |
| `neg` | `Int → Int`, `Real → Real`, `Vec → Vec` |
| `pow`, `min`, `max` | `Real × Real → Real` |
| `<`, `<=`, `>`, `>=`, `==`, `!=` | `Real × Real → Bool` |
| `and`, `or` | `Bool × Bool → Bool` |
| `not` | `Bool → Bool` |
| `sin`, `cos`, `tan`, `exp`, `log`, `sqrt`, `abs` | `Real → Real` |
| `norm`, `x`, `y`, `z` | `Vec → Real` |

**Constants.** Of type `Vec`: `ORIGIN`, `UP`, `DOWN`, `LEFT`, `RIGHT`, `UL`, `UR`, `DL`, `DR`,
`IN`, `OUT`. Of type `Real`: `PI`, `TAU`, `DEGREES`. Of type `Color`, the 89 colours Manim
exports by name, which `tools/gen_colors.py` writes into the catalogue from the installed
Manim:

- `WHITE` and `BLACK`;
- `BLUE`, `TEAL`, `GREEN`, `YELLOW`, `GOLD`, `RED`, `MAROON`, `PURPLE` and `GRAY`, each also
  in the five shades `_A` to `_E`;
- `DARK_GRAY`, `DARKER_GRAY`, `LIGHT_GRAY`, `LIGHTER_GRAY` and `GRAY_BROWN`;
- every name with `GRAY` in it once more with `GREY`;
- `DARK_BLUE`, `DARK_BROWN`, `LIGHT_BROWN`, `LIGHT_PINK`, `PINK` and `ORANGE`;
- `PURE_RED`, `PURE_GREEN`, `PURE_BLUE`, `PURE_CYAN`, `PURE_MAGENTA` and `PURE_YELLOW`;
- `LOGO_WHITE`, `LOGO_GREEN`, `LOGO_BLUE`, `LOGO_RED` and `LOGO_BLACK`.
