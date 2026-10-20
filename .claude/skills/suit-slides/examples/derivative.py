"""A complete deck: what a derivative is, in five slides. Build it with

    python3 derivative.py [where/derivative.suit.json]

It shows the habits worth copying: a title slide, one idea per slide, objects cleared
when their slide is done, a grid behind every graph, formulas that turn into each other,
a tangent that moves while a number keeps up with its slope, and notes for the speaker.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "scripts"))
from suitdeck import *  # noqa: E402,F403

deck = Deck()

# ── 1. Title ─────────────────────────────────────────────────────────────────
s = deck.slide("Title", notes="Today: how steep a curve is, at a single point.")
title = s.new("title", "Text", "The Derivative", font_size=64)
subtitle = s.new("subtitle", "Text", "how steep is a curve at one point?", font_size=32, color=GRAY)
subtitle.then("next_to", title, DOWN, buff=0.4)
s.play(Write(title), run_time=1.5)
s.play(FadeIn(subtitle, shift=0.3 * UP))

# ── 2. The slope of a line ───────────────────────────────────────────────────
s = deck.slide("Slope of a line", notes="A line has one steepness everywhere: rise over run.")
s.play(FadeOut(title, shift=UP), FadeOut(subtitle))
grid = s.new("grid", "NumberPlane").then("set_opacity", 0.35)
line = s.new("line", "Line", (-3, -1.5), (3, 1.5), color=BLUE, stroke_width=6)
run = s.new("run", "Line", (-1, -0.5), (1, -0.5), color=YELLOW)
rise = s.new("rise", "Line", (1, -0.5), (1, 0.5), color=YELLOW)
run_label = s.new("run_label", "MathTex", r"\Delta x", color=YELLOW).then("next_to", run, DOWN)
rise_label = s.new("rise_label", "MathTex", r"\Delta y", color=YELLOW).then("next_to", rise, RIGHT)
rule = s.new("rule", "MathTex", r"\text{slope} = \frac{\Delta y}{\Delta x}", font_size=48)
rule.then("to_corner", UL)
s.play(FadeIn(grid), Create(line))
s.play(Create(run), Create(rise), Write(run_label), Write(rise_label))
s.play(Write(rule))

# ── 3. A curve ───────────────────────────────────────────────────────────────
s = deck.slide("A curve", notes="A curve is steeper in some places than in others.")
s.play(FadeOut(line), FadeOut(run), FadeOut(rise), FadeOut(run_label), FadeOut(rise_label))
curve = s.new("curve", "Plot", lambda x: x ** 2 / 4, color=BLUE, stroke_width=6)
f = s.new("f", "MathTex", r"f(x) = \frac{x^2}{4}", font_size=48, color=BLUE)
f.then("to_corner", UR)
s.play(Create(curve), run_time=2)
s.play(Write(f))
s.play(Circumscribe(f))

# ── 4. The tangent ───────────────────────────────────────────────────────────
s = deck.slide("The tangent", notes="Zoom in far enough and the curve looks like a line: the tangent. Its slope is the derivative.")
tangent = s.new("tangent", "Tangent", curve, at=-2, dot=True, color=YELLOW, length=4)
label = s.new("label", "MathTex", r"f'(x) =", font_size=48, color=YELLOW)
label.then("next_to", rule, DOWN, aligned_edge=LEFT, buff=0.5)
slope = s.new("slope", "DecimalNumber", number=-1, num_decimal_places=2, color=YELLOW)
slope.then("next_to", label, RIGHT)
s.play(Create(tangent))
s.play(FadeIn(label), FadeIn(slope))
s.keep(slope, "set_value", tangent.slope)
s.play(Glide(tangent, to=2.5), run_time=4)
s.play(Glide(tangent, to=0), run_time=2)

# ── 5. The rule ──────────────────────────────────────────────────────────────
s = deck.slide("The rule", notes="For this curve the slope at x is x/2: the derivative is itself a function.")
s.stop(slope)
s.play(FadeOut(tangent), FadeOut(label), FadeOut(slope), FadeOut(rule), FadeOut(curve), FadeOut(grid))
s.play(animate(f, "move_to", 1.2 * UP, also=[("scale", 1.5)]))
# Made after f has moved, so that "below f" means below where f is now.
derivative = s.new("derivative", "MathTex", r"f'(x) = \frac{x}{2}", font_size=72, color=YELLOW)
derivative.then("next_to", f, DOWN, buff=0.8)
s.play(TransformFromCopy(f, derivative), run_time=1.5)
s.play(Indicate(derivative))

deck.save(sys.argv[1] if len(sys.argv) > 1 else "derivative.suit.json")
