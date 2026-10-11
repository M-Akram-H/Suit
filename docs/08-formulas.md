# 8. Formulas

Most settings are plain values: a number in a field, a colour from a list. Any of them can
instead be *computed*. Press the small **fx** next to a setting and it becomes a text field
that takes a formula.

```
2 * radius
circle.center + 1.5 * UP
axes.c2p(t.value, sin(t.value))
if t.value > 0 then GREEN else RED
```

Formulas are written the way a spreadsheet writes them, not as Python. A formula that
cannot be read is said to be so under the field, and nothing changes until it can.

## What a formula can contain

| Written | Meaning |
|---|---|
| `1.5`, `-3`, `42` | Numbers |
| `true`, `false` | Yes and no |
| `"hello"` | Text |
| `#58c4dd` | A colour |
| `(1, 2)` | A point or a direction. A third number, `(1, 2, 0)`, is the depth and is rarely needed. |
| `UP`, `DOWN`, `LEFT`, `RIGHT`, `UL`, `UR`, `DL`, `DR`, `ORIGIN` | Directions, and the middle of the frame |
| `PI`, `TAU`, `DEGREES` | π, 2π, and one degree: `90 * DEGREES` is a quarter turn |
| `BLUE`, `YELLOW`, `GRAY_B`, … | Manim's named colours |
| `circle`, `dot 2` | An object, by its name. Names with spaces work as they stand. |
| `` `odd name!` `` | A name that would otherwise be misread, in backticks |
| `row[0]` | A part of an object: the first member of a group, the first part of a formula |
| `speed` | A named value |

## Arithmetic

| Written | Meaning |
|---|---|
| `+` `-` `*` `/` | As usual. Points can be added, and multiplied by a number: `2 * UP + LEFT`. |
| `^` | Power: `x ^ 2` |
| `<` `<=` `>` `>=` `=` `!=` | Comparisons |
| `and`, `or`, `not` | For combining comparisons |
| `if … then … else …` | One value or another |
| `sin`, `cos`, `tan`, `exp`, `log`, `sqrt`, `abs` | Functions of a number. `log` is the natural logarithm. |
| `min(a, b)`, `max(a, b)` | The smaller and the larger of two numbers |
| `norm(p)` | The length of a direction |

## Asking an object

An object can be asked things with a dot after its name:

| Written | Gives |
|---|---|
| `circle.center` | The point in its middle |
| `circle.top`, `.bottom`, `.left`, `.right` | The middle of that side |
| `circle.get_corner(UR)` | A corner |
| `line.start`, `line.end` | The ends of a line |
| `circle.x`, `circle.y` | Where its middle is, as numbers |
| `circle.width`, `circle.height` | Its size |
| `curve.point_from_proportion(0.25)` | The point a quarter of the way along its outline |
| `t.value` | The value of a *number that can change*, or of a number that is shown |
| `tangent.slope` | The slope of a tangent |
| `axes.c2p(x, y)` | Where the axes put the point (x, y) |
| `line.n2p(x)` | Where a number line puts the number x |

A point has parts too: `p.x` and `p.y`, as in `circle.center.x`.

## Named values

**More → Named value** in the Add bar adds a step that gives a number a name. Every step
after it can use the name in a formula. Change the number once, and everything computed
from it follows:

```
gap = 0.4
```

```
circle 2 = Circle(radius = 1.0)        then: next to circle, RIGHT, buff = gap
```

The value of such a name is fixed. For a number that changes *during* the slide, use
**More → Number that can change**: it is an object, read as `t.value`, and an animation
can change it ([chapter 6](06-steps-and-animations.md#keep-doing)).

## Rules

A graph's **function** and a curve's rule are formulas in a variable of their own, shown
in front of the field as `x ↦` or `t ↦`:

```
x ↦  x ^ 3 - 2 * x
t ↦  (2 * cos(t), sin(2 * t))
```

## When is a formula worked out?

When its step is done. A setting of a new object is worked out when the object is made,
and a setting of an animation when the animation starts. Afterwards the value stays as it
was, even if what it was computed from changes.

To have something follow along continuously, put the formula into a
**Keep doing…** step: that one is worked out again on every frame.

## What can go wrong

- **A name that is not known**: *There is nothing called “valu” here.* Check the spelling, and that the
  thing is made in a step *before* this one.
- **The wrong kind of value**: a text where a number is needed, say. The formula is
  accepted, and the step is listed under Problems.
- **A value that does not exist**, like `log(-1)` or `1 / 0`. For a graph, Suit simply
  does not draw that part. Elsewhere a division by zero stops Manim, and the bar says
  why; a value like `log(-1)` is *not a number*, and Manim silently draws nothing of an
  object placed with it, which Problems reports.

Next: [9. Slides and presenting](09-slides-and-presenting.md)
