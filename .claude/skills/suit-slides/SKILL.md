---
name: suit-slides
description: Creates slide decks for Suit, the visual editor for Manim / manim-slides presentations, as .suit.json files the user opens in Suit (File → Open). Use this whenever the user asks for slides, a deck, a presentation, a lecture, a talk, an explainer or animated maths about a topic, a lecture, a concept, a theorem or a proof — "make slides about X", "prepare lecture 3 on eigenvalues", "a deck explaining derivatives for my students" — even if Suit is not named, and whenever they want Manim or manim-slides animations they can edit by hand afterwards. Also use it to change, extend or check an existing .suit.json deck. Prefer it over PowerPoint, Google Slides or plain Manim code unless the user asks for one of those.
---

# Suit slides

Suit is a visual editor for [Manim](https://www.manim.community/) slides. A deck is one
file, `NAME.suit.json`: slides, each a list of steps (make an object, animate it, pause).
Manim does all the drawing. This skill writes such files from a short Python script, using
`scripts/suitdeck.py`, checks them with Suit's own checker, and renders a preview so the
slides can be looked at before they are handed over.

The user opens the result in Suit with **File → Open…**, presents it with **Present**, and
can change anything in it by clicking. So a deck from this skill is a first draft they
will polish: clear structure and clean layout matter more than flourishes.

## The workflow

### 1. Settle the brief

Work out the topic, the audience (school, first-year university, experts), the length,
the language, and where to save. Ask only if something important is truly unclear;
otherwise choose and say what you chose. Sensible defaults:

- 5 to 8 slides, one idea each, about one to two minutes of talking per slide.
- Saved in the current working directory as `topic-in-kebab-case.suit.json`. If Suit runs
  in Docker, decks belong in the `decks` folder of the Suit checkout.
- The user's language for every word on the slides and in the notes.

If the user gives lecture material (a PDF, notes, a syllabus), read it and follow its
order, notation and examples: it is their course.

### 2. Plan the story before writing code

Write a short plan: for each slide, its title, the one thing it should make clear, what
appears, what moves, and two or three sentences of speaker notes. Show it to the user
when the deck is long or the topic is theirs to shape; otherwise go straight on.

Good Manim slides explain with motion, not with paragraphs. Prefer:

- building a picture up piece by piece, and pointing at the piece being talked about
  (Indicate, Circumscribe, a SurroundingRectangle, colour);
- turning one formula into the next (TransformMatchingTex, ReplacementTransform), so
  the audience sees what changed;
- moving a point, a line or a number continuously (a ValueTracker or a Tangent with
  `keep`), so a relationship becomes visible;
- few words on screen: a title, a formula or two, short labels. What is said goes into
  the notes.

### 3. Write and run the build script

Write a script `NAME.build.py` that imports `suitdeck` and saves the deck. Keep it next to
the deck: later changes are easiest made by editing it and running it again (as long as
the user has not changed the deck in Suit in between: then their file is the truth).

```python
import sys
sys.path.insert(0, "SKILL_DIR/scripts")   # the folder this SKILL.md is in, + /scripts
from suitdeck import *

deck = Deck()
s = deck.slide("Title", notes="What the talk is about.")
title = s.new("title", "Text", "Eigenvalues", font_size=64)
s.play(Write(title))
...
deck.save("eigenvalues.suit.json")
```

`deck.save` writes the file and runs Suit's checker on it. It stops with a message if the
checker finds an error; fix the script and run it again until it says
`0 error(s), 0 warning(s)`, and read the warnings: they are mostly real mistakes. The
timeline it prints says how long each slide takes.

`examples/derivative.py` is a complete five-slide deck to copy from.
`references/recipes.md` has tested snippets for common things: titles, bullet lists,
algebra steps, graphs, tangents, number lines, vectors, tables, matrices, pictures.

### 4. Look at it

```bash
python3 SKILL_DIR/scripts/preview.py NAME.suit.json
```

renders the deck small with real Manim (half a minute or so) and writes
`NAME.preview.png`: a row per slide, pictures taken through the slide, the last one as
the slide ends. Look at it with the Read tool. Check for:

- anything off the edge of the frame, or overlapping something else;
- text too small to read at the back of a room, or too large;
- things still on screen from the slide before that should have gone;
- an empty or nearly empty slide, or one with too much on it;
- a formula that LaTeX could not typeset (the render then fails with LaTeX's message).

Fix and repeat. Two or three rounds are normal. If the preview cannot run (no Manim
found), say so and hand over the checked deck anyway.

### 5. Hand it over

Tell the user where the file is, how to open it (**File → Open…** in Suit), and give a
one-line summary per slide. Mention anything you chose for them and anything you are
unsure of. Offer to change it.

## The library in brief

`python3 scripts/suitdeck.py catalogue` lists every object, animation, action, question,
easing, constant and colour Suit knows; `catalogue NAME` gives the settings of one.
Names are Manim's.

| Write | To |
|---|---|
| `deck = Deck()` | start a deck: 1920×1080, 60 fps, black. `Deck(background="#1e1e2e")` for another colour |
| `s = deck.slide("Title", notes="…")` | add a slide. `loop=True` repeats it until the next key |
| `o = s.new("name", "Circle", radius=1.5, color=BLUE)` | make an object; settings by position or name |
| `o.then("next_to", other, DOWN, buff=0.3)` | place it where it is made (straight after `new` only) |
| `s.play(Write(o))`, `s.play(Create(a), FadeIn(b))` | animate; several start together |
| `s.play(FadeIn(a), FadeIn(b), FadeIn(c), lag=0.3)` | a cascade |
| `s.play(in_turn(Create(a), Create(b)))` | one after another in one step |
| `s.play(Write(o), run_time=2)`, `Indicate(o, rate="linear")` | length and easing |
| `animate(o, "shift", 2 * RIGHT, also=[("scale", 1.5)])` | a smooth change, as an animation for `play` |
| `s.do(o, "move_to", (2, 1))` | an instant change |
| `s.show(o)`, `s.hide(o)`, `s.wait(1)` | appear or vanish without animation; pause |
| `g = s.group("row", a, b, c)`, then `g[0]` | move and animate several as one |
| `c = s.copy("twin", o)` | a copy, an object of its own |
| `k = s.value("k", 0.5)` | a named number for formulas |
| `s.keep(label, "next_to", dot, UP)` | from now on, redo this every frame (follow) |
| `s.keep_every(o, lambda dt: ("rotate", dt * PI))` | same, with the frame time |
| `s.stop(o)` | stop what `keep` started |
| `s.python("self.wait(1)", takes=1)` | raw Python, for what nothing else can do |
| `deck.save("x.suit.json")` | write and check |

Values: numbers; points as `(x, y)` or `point(x, y)`; constants `UP DOWN LEFT RIGHT UL UR
DL DR ORIGIN PI TAU DEGREES`; colours as names (`BLUE`, `YELLOW_C`) or `"#58c4dd"`.
Formulas are built with ordinary arithmetic: `2 * UP + LEFT`, `circle.center + 0.5 * UP`,
`sin(t.value)`, `choose(t.value > 1, GREEN, RED)`. An object can be asked `.center .top
.bottom .left .right .start .end .x .y .width .height .value .slope`, `corner(UR)`,
`point_from_proportion(0.5)`, `axes.c2p(x, y)`, `line.n2p(x)`. A rule is a lambda:
`s.new("graph", "Plot", lambda x: x ** 2 / 4)`.

Mistakes in names, settings or types raise `SuitError` at the line that makes them, with
the names that would have fitted.

## Layout

The frame is 8 units high and 14.2 wide; (0, 0) is the middle, x grows right, y up.
Keep everything inside x from −6.6 to 6.6 and y from −3.6 to 3.6.

- Place things relative to each other (`next_to`, `to_edge`, `to_corner`, `arrange` on a
  group, `align_to`) rather than by coordinates: it survives changes of text.
- Sizes that read well when presented: title 56–64, headings 40–48, body text 32–36,
  labels 24–28. `MathTex` formulas: 48 for the main one, 36 for others.
- At most four or five lines of text per slide. A slide's title, if it has one, sits at
  the top (`then("to_edge", UP)`), and the rest below it.
- Labels under neighbouring parts of one formula collide: each label is wider than its
  part. Put them at different heights, or one above and one below.
- One accent colour for "look here" (YELLOW is Manim's habit), one or two for categories
  (BLUE, GREEN, RED), and white or GRAY for the rest. Keep a meaning for each colour.
- A graph goes on a **Grid** (`NumberPlane`), whose lines are one unit apart like the
  graph's own coordinates; dim it with `.then("set_opacity", 0.35)`.

## What Suit's checker insists on, and why

These are Manim's rules, made visible. The checker reports them; knowing them saves rounds.

- **An object exists from `new`, but is not seen until it is brought in**: by an
  entrance animation (Write, Create, FadeIn, GrowFromCenter, DrawBorderThenFill…) or by
  `show`. Animating or changing something that is not on screen is an error, and so is
  bringing in something already there.
- **Objects stay from slide to slide.** A new slide starts with whatever the last one
  left. Clear what should not stay, usually with FadeOut steps at the start of the next
  slide (or the end of this one).
- **After `ReplacementTransform(a, b)`, `a` is used up** and `b` is on screen; carry on
  with `b`. `Transform(a, b)` instead keeps `a` (it takes b's shape), and `b` is never
  shown. Uncreate and Unwrite use their object up too.
- **One animation per object per step.** To move and grow at once use
  `animate(o, "shift", …, also=[("scale", 2)])`, not two animations.
- **Every slide needs at least one `play` or `wait`.**
- **A group takes its members over**: after `s.group("row", a, b)`, use `row` or `row[0]`.
- **`keep` lasts**: an object told to follow another keeps doing so on later slides
  until `s.stop(o)`. Stop it before the object it follows is faded out or used up.

## Manim's surprises

- `Text` is plain text set by Pango; `MathTex` is LaTeX in math mode, so words inside a
  formula need `\text{…}`, and a dollar sign is `\$`. `Tex` is LaTeX in text mode. Use
  raw strings (`r"\frac{1}{2}"`) for LaTeX.
- Several strings in one `MathTex` make separate parts: `MathTex("a^2", "+", "b^2")`.
  `TransformMatchingTex` matches parts with equal text, so split formulas that way to
  have terms glide to their new places.
- A shape's `fill_color` shows nothing without `fill_opacity`; a line has no inside at
  all. `color` colours the whole shape.
- Manim's `Axes` squeeze their range into a fixed size: their ticks are not one unit
  apart. Draw with Suit's `Plot` on a `NumberPlane`, or use `axes.c2p(x, y)` for points
  on Axes. A colour given to Axes or a NumberPlane when made does not show: use
  `.then("set_color", GRAY)`.
- Animating a `rotate` with `animate` morphs in a straight line; use `Rotate(o, angle=PI/2)`.
- `Text` does not wrap. Break lines with `\n`, or make one object per line and group them
  with `arrange` (see the bullet recipe).

## Changing an existing deck

If you built it and the user has not edited it in Suit since, edit the build script and
run it again. Otherwise the `.suit.json` file is the truth: read
`references/format.md`, change the JSON carefully (ids stay unique, every reference is by
number), and check the result with `python3 scripts/suitdeck.py check NAME.suit.json`.

## When things are missing

`python3 scripts/suitdeck.py where` says which `suit-cli` and which Python with Manim are
used. Without `suit-cli` the deck is written but not checked; without Manim there is no
preview. Both come with Suit (its installers, its Docker image, or a source checkout, where
`cargo build -p suit-cli` makes the first). Set `SUIT_CLI` or `SUIT_PYTHON` to point at
others. Say plainly in the hand-over which checks could not run.

### In a chat, away from the user's computer

In a Claude chat (claude.ai, or the Chat tab of the app) the code runs in a sandbox where
Suit and Manim are not installed. The skill still works: `suitdeck.py` checks every name,
setting and type against the catalogue it carries (`assets/signature.json`), so most
mistakes are still caught while the deck is built. What cannot happen there is Suit's
full check (whether objects are on screen when animated, timings) and the preview. So:

- Be more careful with the rules in "What Suit's checker insists on": nothing else will
  catch a slip there.
- Save the deck where the chat offers files for download (for example
  `/mnt/user-data/outputs/`), and give the user the link.
- Tell the user it is unchecked, and to open it in Suit (**File → Open…**) and look at
  the Problems list along the bottom, which shows anything the checker would have said.
