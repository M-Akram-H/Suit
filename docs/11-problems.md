# 11. Problems

The list along the bottom of the window says what is wrong with the deck. When it says
*None. The deck is ready to export*, there is nothing to do.

Each problem names its slide and step, says what is wrong, and usually what to do about
it. **Click a problem to go to its step.** The slide is marked ⚠ in the slide list, and the
step in the step list.

There are three sources.

## 1. The checker: mistakes found by reading the deck

Suit reads the steps without running them, on every change. This is quick, and it finds
mistakes that Manim would not report at all, because for Manim they are legal code that
draws the wrong thing.

Problems come in two strengths:

- **Errors** (red). The deck would not do what it says. A deck with errors cannot be
  exported.
- **Warnings** (yellow). The deck works, but probably not as meant.

### Errors

| What it says | What is wrong | What to do |
|---|---|---|
| *“circle” is not on the slide yet, so … would make it pop in out of nowhere.* | An animation moves or changes something that has not been brought in. Manim would add it abruptly at the first frame. | Bring it in first: FadeIn, Create, or Show. |
| *“circle” is already on the slide.* | It is brought in a second time. Manim would make it vanish and reappear. | Remove one of the two entrances. |
| *“circle” was used up by an earlier animation and cannot be used again.* | It was turned into something else (ReplacementTransform), or uncreated. | Use the object it became; or use *Transform*, which keeps the object. |
| *“circle” is animated twice at the same time.* | Two animations of one step act on the same object. Manim would silently keep only one. | Put them in separate steps, or combine them into one change with **And also…** |
| *… is changed earlier in this sequence, but this change starts from where it was before…* | Two animated changes of one object in a one-after-another step. Manim works both out before either runs, so the first would be undone. | Use two steps. |
| *Create works on a VMobject, but “picture” is a ImageMobject.* | The animation does not suit this kind of object. Manim would stop with an error halfway through rendering. | Choose another animation; the **▾** menu only lists those that suit. |
| *This slide shows nothing: it has no animation and no pause.* | manim-slides would drop or reject the slide. | Add an animation or a **Pause**. |
| *“circle” does not exist yet at this point.* | A step or formula uses an object that is made further down, or on a later slide. | Move the step that makes it up. |
| *Circle has no setting called “…”*, *… needs “…”*, *There is no … called …* | A deck written or edited by hand names something the catalogue does not have. | Correct the name. |
| *… cannot be applied to …*, *A condition must be yes or no.* | A formula mixes sorts of values, such as adding a number to a colour. | Correct the formula. |
| *An animation must last longer than zero.* | | Give it a length. |

### Warnings

| What it says | Why it matters | What to do |
|---|---|---|
| *This looping slide does not end the way it starts, so the loop will jump.* | Something is on the slide at the end that was not at the start, or the other way round. | Remove what the slide adds, and bring back what it removes, before it ends. |
| *This slide both loops and moves on by itself.* | A loop never ends, so the slide never moves on. The HTML export decides the other way and moves on after one round. | Choose one of the two. |
| *A looping first or last slide does not work in the web (RevealJS) export.* | | Put a plain slide before or after it. |
| *“square” will not turn: it slides in a straight line to its rotated position.* | An animated *rotate* is a morph between two pictures, not a turn. | Use the **Rotate** animation. |
| *“label” keeps following “circle”, which ReplacementTransform uses up.* | The follower would go on following what is left of an object that is gone. | Add **Stop keeping on** before, and **Keep doing…** for the new object after. |
| *“line” is given a fill colour, but has no inside to fill.* | Manim takes the colour and shows nothing. | **Use color instead**, offered in Properties. |
| *“circle” is given a fill colour that cannot be seen: its inside starts fully transparent.* | The same, for a shape with no fill opacity. | **Add fill opacity**, offered in Properties. |
| *“label” is given a stroke colour, but has no outline: Manim draws it with a stroke width of 0.* | Texts, formulas, numbers, dots and braces have no outline until they are given a stroke width, so a stroke colour or opacity shows nothing. (On a formula Manim colours the letters with a stroke colour, but draws no outline either.) | **Add stroke width**, offered in Properties. |
| *“label” is given a stroke colour by “set stroke”, but has no outline: …* | The same, for a **set stroke** without a width. | Give **set stroke** a width. |
| *“picture” is given “color”, which shows nothing: Manim applies it to vector objects only.* | A picture keeps its own colours, and a number that can change is never drawn. | Take it away with **×**. |
| *“axes” is given “color”, which shows nothing: Manim makes its parts separately, and they get none of it.* | Axes and a Grid are built from lines of their own. A colour or an outline given to the whole reaches none of them. | **Use set color instead** (or **set stroke**), offered in Properties. A fill can only be taken away. |

## 2. Manim's remarks: found by running the deck

Some things only show when Manim has actually made the objects. Suit lists them with the
others, as warnings.

| What it says | What happened | What to do |
|---|---|---|
| *The function “log(x)” is not defined on all of [-3, 3].* | A graph was told to draw where its rule has no value. It is drawn where it has one. | Shorten **drawn on**, or leave it empty to let Suit choose. |
| *“tangent” has nothing to touch: its graph has no value there.* | A tangent stands at an `x` where its graph has no value. | Drag it onto the graph, or change **at x**. |
| *Manim draws nothing of “graph”: some of its points are not numbers.* | An object with an impossible point, mostly one of Manim's own graphs over a range where its rule has no value. Manim leaves the whole object out without a word. | Give it a range where the rule has a value, or use *Graph of a function*. |

## 3. Manim not getting through the deck

If Manim stops with an error of its own, the top bar says *Preview stopped:* with the
reason, and Problems repeats it as *Manim stopped before the end of the deck*. The canvas
keeps the last pictures it got, and says that they are earlier ones.

The usual causes:

| Reason given | Cause |
|---|---|
| *… has no Manim. Start Suit with run.sh …* | Suit found a Python without Manim ([chapter 15](15-troubleshooting.md)). |
| A LaTeX error | A formula that LaTeX cannot typeset, like an unclosed brace. Correct the formula. |
| A Python error naming a step's code | A **Python code** step that fails. |
| A division by zero | A formula that divides by zero, outside a graph. |

**Try again** in the top bar runs the deck again. If Manim seems stuck, **Stop** appears
after four seconds; nothing is lost by pressing it.

## What the checker promises

If the list is empty, then for a deck made only from catalogue steps:

- every animation has its object in the state it needs;
- no object is animated twice at once;
- every slide has a length, and Suit knows it to the frame without rendering;
- the export is valid manim-slides code that renders those same frames.

The promise does not cover what a **Python code** step does, and it is about structure,
not taste: an object can still be off the edge of the frame or under another one. The
canvas is there for that.

The rules behind this are written down formally in [the specification](spec/manim-core.md),
sections 5 and 6.

Next: [12. How it works](12-how-it-works.md) · or [15. Troubleshooting](15-troubleshooting.md)
