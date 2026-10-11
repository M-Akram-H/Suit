# 6. Steps and animations

A slide is a list of steps, done from top to bottom. This chapter covers every kind of
step, how long each takes, and how several animations play together.

## On the slide, or not

Every object is in one of three states, and the steps change it:

| State | Meaning | How it gets there |
|---|---|---|
| **not on the slide** | It exists, but is not shown | It was just made, or was taken away |
| **on the slide** | It is shown | An animation brought it in, or **Show** did |
| **used up** | It has turned into something else | It was the source of a *ReplacementTransform*, or was *Uncreated* or *Unwritten* |

Suit keeps track of this for every object at every step, and only offers what makes
sense: an object that is not on the slide can be brought in but not moved, and one that
is on the slide can be moved but not brought in again. Doing the wrong thing anyway is
reported as a problem ([chapter 11](11-problems.md)).

## Animations

Pick an object and click **Animate**. If it is not on the slide it gets a *FadeIn*; if it
is, an *Indicate*. The **▾** next to the button lists every animation that suits the
object as it is now, sorted by purpose:

| Purpose | Examples |
|---|---|
| **Bring it in** | FadeIn, Create, Write, DrawBorderThenFill, GrowFromCenter, SpinInFromNothing |
| **Draw attention to it** | Circumscribe, ShowPassingFlash |
| **Change it** | *Move, turn, resize, recolour…*, Indicate, Wiggle, Rotate, FadeToColor, ApplyWave |
| **Turn it into another** | Transform, ReplacementTransform, TransformMatchingTex, FadeTransform, MoveAlongPath |
| **Take it away** | FadeOut, Uncreate, Unwrite |

### The settings of an animation

Select the step. Properties shows, for each animation in it:

- **which animation** it is: the menu at the top changes it into another one;
- **object**: what it animates, and for animations that need a second object, which;
- the animation's own settings, under **Add setting** (a *FadeIn* can come from a
  direction, a *Rotate* has an angle);
- **takes**: how long it lasts. **×** returns it to the animation's usual length;
- **easing**: how it speeds up and slows down. *smooth* is the usual one; *linear* moves
  at a constant speed; *there and back* goes and returns.

### Several at once, or one after another

**Add animation** puts another animation into the same step. They start at the same
moment and the step lasts as long as the longest.

With more than one animation in a step, Properties offers:

- **Play these together** or **Play these one after another**;
- a **lag** between 0 and 1: how far into one animation the next one starts. 0 is all at
  once, 1 is strictly one after another, 0.2 is a quick cascade.

An object can only be in one animation of a step at a time.

### Animating a change

*Move, turn, resize, recolour…* under **Change it** animates anything that **Change**
can do instantly: the object goes smoothly from how it is to how it would be.

In Properties, **do** chooses what changes:

| Do | What it does |
|---|---|
| shift | Moves by a distance in a direction |
| move to | Moves to a point, or onto another object |
| next to | Puts it beside another object |
| to edge, to corner | Moves it to the edge or a corner of the frame |
| align to | Lines it up with another object |
| scale, scale to fit width / height | Resizes it |
| rotate, flip, stretch | Turns, mirrors or stretches it |
| set color, set fill, set stroke, set opacity | Recolours it |
| set x, set y | Sets one coordinate |
| set value | Changes a number |
| arrange | Lines up the members of a group |
| become | Makes it look like another object |

**And also…** adds a second change made in the same movement (move *and* grow).

> Manim morphs in straight lines. An animated *rotate* therefore shrinks the object on
> the way instead of turning it. Suit warns about this; use the **Rotate** animation to
> turn something.

## Instant steps

| Button | Step | What it does |
|---|---|---|
| **Change** | ✏ | The same changes as above, made between two frames |
| **Show** | 👁 | Puts the object on the slide without an animation |
| **Hide** | 🚫 | Takes it off without an animation |
| **Pause** | ⏸ | Holds still. The step's setting is for how long. |

Instant steps take no time. They are useful before an animation (place an object, then
fade it in) and to set up the start of a slide.

## Dragging on the canvas

Dragging an object says where it should be. What that means depends on the object:

- If it **has not moved since it was made**, the drag changes where it is made.
- If it **has**, Suit asks:

| Choice | Result |
|---|---|
| **Glide there** | A step that animates the move |
| **Jump there** | A step that moves it instantly |
| **Start from here** | Changes where it is made, on every slide |
| **Cancel** | Nothing |

## Keep doing

**More → Keep doing…** makes an object do something again on every frame, from that step
on. This is how one thing follows another:

- a label that stays **next to** a shape wherever the shape goes,
- a dot that stays at a point computed from a number that changes,
- a number that shows a value as it changes.

Properties shows *On every frame, it does this again:* followed by the same choices as
**Change**. Combined with a formula ([chapter 8](08-formulas.md)), this is very
flexible. In the sample deck, the dot on the sine wave is told to keep moving to
`axes.c2p(t.value, sin(t.value))`, and then the number `t` is animated from 0 to 2π.

Ticking **by an amount that depends on the time passed** gives the step the time since
the last frame as `dt`, for steady motion: `rotate` by `dt * PI` turns half a turn per
second.

**More → Stop keeping on** ends it from that step on.

A slide whose objects keep doing something stays alive while it waits for you: every
frame of its pause is computed anew.

## Python code

**More → Python code** adds a step that holds Python, for anything Suit has no step for.
The code runs as it stands inside the scene's `construct`, and `self` is the scene.

```python
# self is the scene
self.camera.background_color = "#101020"
```

Suit cannot read Python, so two things are yours to look after:

- If the code plays or waits, set **takes** to how long, so that the times of the slide
  stay right.
- Objects the code makes are not known to the steps after it.

## Order

**↑** and **↓** above the list move the selected step. **Delete**, or the Delete key,
removes it. Moving a step to before the step that makes its object, or deleting the step
that makes an object others use, shows up at once in Problems.

## Every animation

| Animation | Usual length | What it does to its object |
|---|---|---|
| AddTextLetterByLetter | 1 s | Brings a text in, a letter at a time |
| ApplyWave | 2 s | Sends a wave through it |
| Circumscribe | 1 s | Draws a frame around it for a moment |
| Create | 1 s | Brings it in by drawing it |
| DrawBorderThenFill | 2 s | Brings it in: outline first, then the inside |
| FadeIn / FadeOut | 1 s | Brings it in, or takes it away, by fading |
| FadeToColor | 1 s | Changes its colour |
| FadeTransform | 1 s | Fades it into another object |
| Flash | 1 s | Flashes lines around a point |
| FocusOn | 2 s | Closes a spotlight on a point |
| Glide | 1 s | Takes a tangent along its graph ([chapter 7](07-graphs-and-tangents.md)) |
| GrowArrow | 1 s | Brings an arrow in from its start |
| GrowFromCenter / Edge / Point | 1 s | Brings it in by growing |
| Indicate | 1 s | Makes it bigger and yellow for a moment |
| MoveAlongPath | 1 s | Moves it along another object's outline |
| ReplacementTransform | 1 s | Morphs it into another object, which takes its place |
| Rotate | 1 s | Turns it |
| ScaleInPlace | 1 s | Resizes it |
| ShowPassingFlash | 1 s | Runs a flash along its outline |
| SpinInFromNothing | 1 s | Brings it in spinning |
| Transform | 1 s | Morphs it to look like another object; it keeps its name |
| TransformFromCopy | 1 s | Morphs a copy of it into another object |
| TransformMatchingShapes | 1 s | Morphs into another object, matching equal shapes |
| TransformMatchingTex | 1 s | The same for two formulas, matching equal parts |
| Uncreate / Unwrite | 1 s | Takes it away by undoing the drawing |
| Wiggle | 2 s | Wiggles it |
| Write | 1 s | Brings it in as if written by hand |

Next: [7. Graphs and tangents](07-graphs-and-tangents.md)
