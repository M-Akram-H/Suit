"""Builds every recipe of references/recipes.md into one deck, checks it, and (with
--preview) renders it. Run after changing the recipes or the library:

    python3 tests/recipes.py [--preview]
"""

import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "scripts"))
from suitdeck import *  # noqa: E402,F403

out = Path(tempfile.mkdtemp(prefix="suit-recipes-"))
deck = Deck()

# 1
s = deck.slide("Title", notes="Welcome; what we will see today.")
title = s.new("title", "Text", "Eigenvalues", font_size=64)
subtitle = s.new("subtitle", "Text", "Linear Algebra, lecture 7", font_size=32, color=GRAY)
subtitle.then("next_to", title, DOWN, buff=0.4)
s.play(Write(title), run_time=1.5)
s.play(FadeIn(subtitle, shift=0.3 * UP))

# 2, 3
s = deck.slide("Bullets")
s.play(FadeOut(title), FadeOut(subtitle))
heading = s.new("heading", "Text", "Definition", font_size=44).then("to_edge", UP)
s.play(Write(heading))
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
items = s.group("items", *points)
heading2 = s.new("heading2", "Text", "Example", font_size=44).then("to_edge", UP)
s.play(ReplacementTransform(heading, heading2))

# 4
s = deck.slide("Algebra")
s.play(FadeOut(items), FadeOut(heading2))
step1 = s.new("step1", "MathTex", "2x", "+", "3", "=", "11", font_size=60)
step2 = s.new("step2", "MathTex", "2x", "=", "11", "-", "3", font_size=60)
step3 = s.new("step3", "MathTex", "2x", "=", "8", font_size=60)
step4 = s.new("step4", "MathTex", "x", "=", "4", font_size=60)
s.play(Write(step1))
s.play(TransformMatchingTex(step1, step2))
s.play(TransformMatchingTex(step2, step3))
s.play(TransformMatchingTex(step3, step4))
s.play(Circumscribe(step4))

# 5
s = deck.slide("Pointing")
s.play(FadeOut(step4))
energy = s.new("energy", "MathTex", "E", "=", "m", "c^2", font_size=72)
s.play(Write(energy))
frame = s.new("frame", "SurroundingRectangle", energy[3], color=YELLOW, buff=0.1)
note = s.new("note", "Text", "the speed of light, squared", font_size=28, color=YELLOW)
note.then("next_to", frame, DOWN)
s.play(Create(frame), FadeIn(note))
s.play(Indicate(energy[3]))

# 6, 7, 8
s = deck.slide("Graph")
s.play(FadeOut(energy), FadeOut(frame), FadeOut(note))
grid = s.new("grid", "NumberPlane").then("set_opacity", 0.35)
graph = s.new("graph", "Plot", lambda x: sin(x) + x / 3, color=BLUE, stroke_width=5)
rule = s.new("rule", "MathTex", r"f(x) = \sin x + \frac{x}{3}", color=BLUE).then("to_corner", UL)
s.play(FadeIn(grid))
s.play(Create(graph), run_time=2)
s.play(Write(rule))
tangent = s.new("tangent", "Tangent", graph, at=-2, dot=True, length=3, color=YELLOW)
label = s.new("label", "MathTex", "f'(x) =", color=YELLOW).then("next_to", rule, DOWN, aligned_edge=LEFT)
slope = s.new("slope", "DecimalNumber", number=0, num_decimal_places=2, color=YELLOW)
slope.then("next_to", label, RIGHT)
s.play(Create(tangent), FadeIn(label), FadeIn(slope))
s.keep(slope, "set_value", tangent.slope)
s.play(Glide(tangent, to=3), run_time=4)
s.stop(slope)
t = s.new("t", "ValueTracker", value=-3)
mover = s.new("mover", "Dot", color=RED)
s.keep(mover, "move_to", point(t.value, sin(t.value) + t.value / 3))
s.show(t, mover)
s.play(animate(t, "set_value", 3), run_time=4, rate="linear")
s.stop(mover)

# 9
s = deck.slide("Number line")
s.play(FadeOut(graph), FadeOut(rule), FadeOut(tangent), FadeOut(label), FadeOut(slope), FadeOut(mover), FadeOut(grid))
line = s.new("line", "NumberLine", x_range=(-5, 5, 1), length=10, include_numbers=True)
marker = s.new("marker", "Triangle", color=YELLOW, fill_opacity=1).then("scale", 0.2).then("rotate", PI)
x = s.new("x", "ValueTracker", value=0)
s.keep(marker, "next_to", line.n2p(x.value), UP, buff=0.1)
s.play(Create(line))
s.show(x)
s.play(FadeIn(marker))
s.play(animate(x, "set_value", 3.5), run_time=2)
s.stop(marker)

# 10
s = deck.slide("Vectors")
s.play(FadeOut(line), FadeOut(marker))
plane = s.new("plane", "NumberPlane").then("set_opacity", 0.35)
s.play(FadeIn(plane))
v, w = (2, 1), (1, 2)
a = s.new("a", "Arrow", ORIGIN, v, buff=0, color=YELLOW)
b = s.new("b", "Arrow", v, (3, 3), buff=0, color=GREEN)
total = s.new("total", "Arrow", ORIGIN, (3, 3), buff=0, color=RED)
s.play(GrowArrow(a))
s.play(GrowArrow(b))
s.play(GrowArrow(total))
va = s.new("va", "MathTex", r"\vec v", color=YELLOW).then("next_to", a, UP)
s.play(Write(va))

# 11, 12
s = deck.slide("Table and matrix")
s.play(FadeOut(a), FadeOut(b), FadeOut(total), FadeOut(va), FadeOut(plane))
table = s.new("table", "MathTex",
              r"\begin{array}{l|r} \textbf{Name} & \textbf{Score} \\ \hline "
              r"\text{Ada} & 92 \\ \text{Ben} & 85 \end{array}", font_size=44)
table.then("move_to", 3 * LEFT)
matrix = s.new("matrix", "MathTex", r"A = \begin{bmatrix} 2 & 1 \\ 1 & 2 \end{bmatrix}", font_size=56)
matrix.then("move_to", 3 * RIGHT)
s.play(Write(table), Write(matrix))

# 13
s = deck.slide("Picture")
s.play(FadeOut(table), FadeOut(matrix))
(out / "pictures").mkdir()
subprocess.run([find_python() or sys.executable, "-c",
                "from PIL import Image, ImageDraw; im = Image.new('RGB', (400, 300), '#335577'); "
                "ImageDraw.Draw(im).ellipse((100, 50, 300, 250), fill='#ffcc00'); "
                f"im.save({str(out / 'pictures' / 'euler.png')!r})"], check=True)
photo = s.new("photo", "ImageMobject", "pictures/euler.png").then("scale_to_fit_height", 4)
s.play(FadeIn(photo))

# 14
s = deck.slide("Side by side")
s.play(FadeOut(photo))
left = s.new("left", "MathTex", r"\frac{d}{dx} x^2 = 2x").then("move_to", 3.5 * LEFT)
right = s.new("right", "MathTex", r"\int 2x \, dx = x^2 + C").then("move_to", 3.5 * RIGHT)
divider = s.new("divider", "Line", (0, -3), (0, 3), color=GRAY)
s.play(Write(left), Create(divider), Write(right))

# 15, 16
spinner = s.new("spinner", "Square", color=BLUE)
s.play(FadeOut(left), FadeOut(divider), FadeOut(right))
s.play(Create(spinner))
s = deck.slide("Waiting", loop=True)
s.play(Rotate(spinner, angle=PI / 2), run_time=1, rate="linear")
s = deck.slide("End")
s.play(FadeOut(spinner))

path = deck.save(out / "recipes.suit.json")
if "--preview" in sys.argv:
    subprocess.run([sys.executable, str(HERE.parent / "scripts" / "preview.py"), str(path), "--samples", "2"], check=True)
print("deck in", out)
