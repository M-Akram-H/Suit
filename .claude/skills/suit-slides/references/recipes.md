# Recipes

Tested snippets for common things. Each assumes `from suitdeck import *`, a `deck`, and a
slide `s = deck.slide(...)`; names are free to change. They were all built into one deck,
checked and rendered: `tests/recipes.py` in this skill does exactly that.

## Contents

1. Title slide
2. A slide title that stays while the content changes
3. Bullet points revealed one by one
4. Algebra, step by step
5. Pointing at part of a formula
6. A graph on a grid, with its rule
7. A tangent and its slope
8. A point moving along a curve
9. A number line with a moving marker
10. Vectors: addition head to tail
11. A table
12. A matrix
13. A picture from a file
14. Two things side by side
15. Clearing the slide
16. A looping slide

## 1. Title slide

```python
s = deck.slide("Title", notes="Welcome; what we will see today.")
title = s.new("title", "Text", "Eigenvalues", font_size=64)
subtitle = s.new("subtitle", "Text", "Linear Algebra, lecture 7", font_size=32, color=GRAY)
subtitle.then("next_to", title, DOWN, buff=0.4)
s.play(Write(title), run_time=1.5)
s.play(FadeIn(subtitle, shift=0.3 * UP))
```

## 2. A slide title that stays while the content changes

```python
heading = s.new("heading", "Text", "Definition", font_size=44).then("to_edge", UP)
s.play(Write(heading))
# On a later slide, change the words smoothly instead of fading out and in:
heading2 = s.new("heading2", "Text", "Example", font_size=44).then("to_edge", UP)
s.play(ReplacementTransform(heading, heading2))
```

After the `ReplacementTransform`, use `heading2`: `heading` is used up.

## 3. Bullet points revealed one by one

```python
lines = ["vectors that keep their direction",
         "they are only stretched",
         "by a factor: the eigenvalue"]
points = []
for i, text in enumerate(lines, start=1):
    bullet = s.new(f"point{i}", "Text", f"• {text}", font_size=34)
    if points:
        bullet.then("next_to", points[-1], DOWN, buff=0.35, aligned_edge=LEFT)
    else:
        bullet.then("move_to", (0, 1.5)).then("to_edge", LEFT, buff=1.5)
    points.append(bullet)
for bullet in points:
    s.play(FadeIn(bullet, shift=0.2 * RIGHT))
```

Each point hangs below the one before with their left edges lined up (`aligned_edge`).
The first is placed by its height and then pushed to the left margin (`to_edge` moves
only sideways). `arrange` on a group would centre the lines on each other instead.

## 4. Algebra, step by step

```python
step1 = s.new("step1", "MathTex", "2x", "+", "3", "=", "11", font_size=60)
step2 = s.new("step2", "MathTex", "2x", "=", "11", "-", "3", font_size=60)
step3 = s.new("step3", "MathTex", "2x", "=", "8", font_size=60)
step4 = s.new("step4", "MathTex", "x", "=", "4", font_size=60)
s.play(Write(step1))
s.play(TransformMatchingTex(step1, step2))
s.play(TransformMatchingTex(step2, step3))
s.play(TransformMatchingTex(step3, step4))
s.play(Circumscribe(step4))
```

Each string is a part; parts with equal text glide to their new places, the rest fade.
Every step is made at the same place (the middle); move the last one afterwards with
`animate` if needed.

## 5. Pointing at part of a formula

```python
energy = s.new("energy", "MathTex", "E", "=", "m", "c^2", font_size=72)
s.play(Write(energy))
frame = s.new("frame", "SurroundingRectangle", energy[3], color=YELLOW, buff=0.1)
note = s.new("note", "Text", "the speed of light, squared", font_size=28, color=YELLOW)
note.then("next_to", frame, DOWN)
s.play(Create(frame), FadeIn(note))
s.play(Indicate(energy[3]))
```

`energy[3]` is the fourth part, `c^2`.

## 6. A graph on a grid, with its rule

```python
grid = s.new("grid", "NumberPlane").then("set_opacity", 0.35)
graph = s.new("graph", "Plot", lambda x: sin(x) + x / 3, color=BLUE, stroke_width=5)
rule = s.new("rule", "MathTex", r"f(x) = \sin x + \frac{x}{3}", color=BLUE).then("to_corner", UL)
s.play(FadeIn(grid))
s.play(Create(graph), run_time=2)
s.play(Write(rule))
```

`Plot` draws only where the rule has a value and finds asymptotes itself
(`asymptotes=True` draws them dashed). `on=[(0, 4)]` limits it to 0 ≤ x ≤ 4.

## 7. A tangent and its slope

```python
tangent = s.new("tangent", "Tangent", graph, at=-2, dot=True, length=3, color=YELLOW)
label = s.new("label", "MathTex", "f'(x) =", color=YELLOW).then("next_to", rule, DOWN, aligned_edge=LEFT)
slope = s.new("slope", "DecimalNumber", number=0, num_decimal_places=2, color=YELLOW)
slope.then("next_to", label, RIGHT)
s.play(Create(tangent), FadeIn(label), FadeIn(slope))
s.keep(slope, "set_value", tangent.slope)
s.play(Glide(tangent, to=3), run_time=4)
s.stop(slope)
```

`Glide` keeps the line tangent all the way; `keep` keeps the number equal to the slope.

## 8. A point moving along a curve

```python
t = s.new("t", "ValueTracker", value=-3)
mover = s.new("mover", "Dot", color=RED)
s.keep(mover, "move_to", point(t.value, sin(t.value) + t.value / 3))
s.show(t, mover)
s.play(animate(t, "set_value", 3), run_time=4, rate="linear")
s.stop(mover)
```

A `ValueTracker` is an invisible number; animating it moves everything that follows it.
Here the dot's position repeats the graph's rule.

## 9. A number line with a moving marker

```python
line = s.new("line", "NumberLine", x_range=(-5, 5, 1), length=10, include_numbers=True)
marker = s.new("marker", "Triangle", color=YELLOW, fill_opacity=1).then("scale", 0.2).then("rotate", PI)
x = s.new("x", "ValueTracker", value=0)
s.keep(marker, "next_to", line.n2p(x.value), UP, buff=0.1)
s.play(Create(line))
s.show(x)
s.play(FadeIn(marker))
s.play(animate(x, "set_value", 3.5), run_time=2)
s.stop(marker)
```

## 10. Vectors: addition head to tail

```python
v, w = (2, 1), (1, 2)
a = s.new("a", "Arrow", ORIGIN, v, buff=0, color=YELLOW)
b = s.new("b", "Arrow", v, (3, 3), buff=0, color=GREEN)
total = s.new("total", "Arrow", ORIGIN, (3, 3), buff=0, color=RED)
s.play(GrowArrow(a))
s.play(GrowArrow(b))
s.play(GrowArrow(total))
```

`buff=0` makes the arrow run exactly between its points; without it Manim leaves a gap.
On a `NumberPlane` the points are its grid points. A label: `s.new("va", "MathTex",
r"\vec v", color=YELLOW).then("next_to", a, UP)`.

## 11. A table

```python
table = s.new("table", "MathTex",
              r"\begin{array}{l|r} \textbf{Name} & \textbf{Score} \\ \hline "
              r"\text{Ada} & 92 \\ \text{Ben} & 85 \end{array}", font_size=44)
s.play(Write(table))
```

A table is one formula: its cells cannot be animated separately. For rows that appear
one by one, make one `MathTex` per row and `arrange` them in a group.

## 12. A matrix

```python
matrix = s.new("matrix", "MathTex", r"A = \begin{bmatrix} 2 & 1 \\ 1 & 2 \end{bmatrix}", font_size=56)
s.play(Write(matrix))
```

## 13. A picture from a file

```python
photo = s.new("photo", "ImageMobject", "pictures/euler.png").then("scale_to_fit_height", 4)
s.play(FadeIn(photo))
```

The name is relative to the folder the deck is saved in; put the file there. Pictures
can only fade, move and grow: they are not drawn with lines.

## 14. Two things side by side

```python
left = s.new("left", "MathTex", r"\frac{d}{dx} x^2 = 2x").then("move_to", 3.5 * LEFT)
right = s.new("right", "MathTex", r"\int 2x \, dx = x^2 + C").then("move_to", 3.5 * RIGHT)
divider = s.new("divider", "Line", (0, -3), (0, 3), color=GRAY)
s.play(Write(left), Create(divider), Write(right))
```

## 15. Clearing the slide

```python
s.play(FadeOut(graph), FadeOut(rule), FadeOut(grid))
```

Everything faded out together in one step. Anything a `keep` makes follow something
else is stopped first: `s.stop(slope)`.

## 16. A looping slide

```python
spinner = s.new("spinner", "Square", color=BLUE)      # on the slide before
s.play(Create(spinner))
s = deck.slide("Waiting", loop=True)
s.play(Rotate(spinner, angle=PI / 2), run_time=1, rate="linear")
```

A looping slide should end the way it starts, or it jumps on every round: so what it
shows is brought in on the slide before, and a quarter turn of a square looks the same as
none. The checker warns otherwise, and also about a loop on the first or last slide,
which the HTML page of `manim-slides convert` cannot show (Suit's own website can).
