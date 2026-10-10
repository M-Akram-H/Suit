"""Confirm against the installed Manim the behaviours docs/spec/manim-core.md §6 relies on.

Runs one scene without writing video and prints a verdict per claim.
"""
import numpy as np
from manim import *

config.dry_run = True
config.frame_rate = 60
config.verbosity = "ERROR"
config.progress_bar = "none"

FPS = 60
results = []


def claim(name, holds, detail=""):
    results.append((name, bool(holds), detail))


class Probe(Scene):
    def frames_of(self, action):
        start = self.renderer.time
        action()
        return round((self.renderer.time - start) * FPS)

    def construct(self):
        # Q2: frame counts follow ceil (animated) and floor (still) of run_time / dt in binary64.
        dot = Dot()
        self.add(dot)
        animated = self.frames_of(lambda: self.play(dot.animate.shift(RIGHT), run_time=23 / 60))
        still = self.frames_of(lambda: self.wait(23 / 60))
        claim("Q2 a 23/60 s play renders 24 frames", animated == 24, f"got {animated}")
        claim("Q2 a 23/60 s still wait renders 23 frames", still == 23, f"got {still}")
        stagger = AnimationGroup(*[Indicate(Dot(), run_time=0.6) for _ in range(5)], lag_ratio=0.3)
        claim("Q2 five 0.6 s animations staggered by 0.3 render 80 frames",
              self.frames_of(lambda: self.play(stagger)) == 80)

        # Q1: the last rendered frame is before the end of the animation.
        alphas = []
        self.play(UpdateFromAlphaFunc(dot, lambda m, a: alphas.append(a)), run_time=1, rate_func=linear)
        claim("Q1 last rendered progress is (N-1)/N, then finish sets 1",
              np.isclose(alphas[-2], 59 / 60) and alphas[-1] == 1, f"tail {alphas[-2]:.4f}, {alphas[-1]}")

        # Q3: a time-based updater advances run_time - dt per play.
        elapsed = [0.0]
        ticker = Dot().add_updater(lambda m, dt: elapsed.__setitem__(0, elapsed[0] + dt))
        self.add(ticker)
        self.wait(1)
        claim("Q3 a dt updater integrates 59/60 s over a 1 s wait", np.isclose(elapsed[0], 59 / 60), f"got {elapsed[0]:.4f}")
        ticker.clear_updaters()
        self.remove(ticker)

        # Q4: a still wait does not run non-time-based updaters.
        leader, follower = Dot(), Dot()
        follower.add_updater(lambda m: m.move_to(leader))
        self.add(leader, follower)
        leader.shift(2 * RIGHT)
        self.wait(0.1)
        claim("Q4 a still wait leaves an `always` follower stale", np.allclose(follower.get_center(), ORIGIN))
        self.wait(0.1, frozen_frame=False)
        claim("Q4 frozen_frame=False makes the follower catch up", np.allclose(follower.get_center(), 2 * RIGHT))
        follower.clear_updaters()
        self.remove(leader, follower)

        # Q5: a composite animation moves its objects to the end of the display list.
        back, front = Square(), Square()
        self.add(back, front)
        order = lambda: [m for m in self.get_mobject_family_members() if m in (back, front)]
        self.play(back.animate.shift(0.1 * UP), run_time=0.1)
        claim("Q5 a plain animation keeps the draw order", order() == [back, front])
        self.play(AnimationGroup(back.animate.shift(0.1 * UP)), run_time=0.1)
        claim("Q5 wrapping it in AnimationGroup brings the object to the front", order() == [front, back])
        self.play(Succession(front.animate.shift(0.1 * UP)), run_time=0.1)
        claim("Q5 so does Succession", order() == [back, front])
        self.remove(back, front)

        # Q6: a shorter animation in a parallel play holds its end state.
        quick = Dot()
        self.add(quick)
        seen = []
        sample = lambda dt: seen.append(quick.get_center()[0])
        self.add_updater(sample)
        self.play(quick.animate(run_time=0.5).shift(RIGHT), Wait(run_time=1))
        self.remove_updater(sample)
        claim("Q6 the shorter animation sits at its end state", len(seen) == 60 and np.allclose(seen[30:], 1.0) and seen[29] < 1.0,
              f"x at frames 29, 30, 59: {seen[29]:.3f}, {seen[30]:.3f}, {seen[59]:.3f}")
        self.remove(quick)

        # Roles: what removers leave behind.
        circle = Circle(fill_opacity=0.5)
        self.add(circle)
        self.play(FadeOut(circle, shift=UP), run_time=0.1)
        claim("FadeOut is an outro: off scene, geometry and opacity restored",
              circle not in self.mobjects and np.isclose(circle.get_fill_opacity(), 0.5) and np.allclose(circle.get_center(), ORIGIN))

        gone = Circle()
        points = gone.get_num_points()
        self.add(gone)
        self.play(Uncreate(gone), run_time=0.1)
        degenerate = gone.get_num_points() == 0 or np.allclose(gone.points, gone.points[0])
        claim("Uncreate is a consume: the object is left without geometry", gone not in self.mobjects and degenerate,
              f"{points} points before, {gone.get_num_points()} after, width {gone.width:.3f}")

        word = Text("ab")
        self.add(word)
        self.play(Unwrite(word), run_time=0.2)
        letters = word.family_members_with_points()
        claim("Unwrite is a consume: every letter is left without extent",
              word not in self.mobjects and all(np.allclose(m.points, m.points[0]) for m in letters),
              f"{len(letters)} letters")

        source, target = Circle(), Square()
        self.add(source)
        self.play(ReplacementTransform(source, target), run_time=0.1)
        claim("ReplacementTransform consumes its source and introduces its target",
              source not in self.mobjects and target in self.mobjects and np.isclose(source.width, target.width))

        source, target = Circle(), Square().shift(3 * RIGHT)
        self.add(source)
        self.play(FadeTransform(source, target), run_time=0.1)
        claim("FadeTransform is an outro for its source",
              source not in self.get_mobject_family_members() and target in self.get_mobject_family_members()
              and np.allclose(source.get_center(), ORIGIN) and np.isclose(source.get_stroke_opacity(), 1.0))

        source, target = Text("ab"), Text("ba").shift(DOWN)
        before = source.get_center().copy()
        self.add(source)
        self.play(TransformMatchingShapes(source, target), run_time=0.1)
        family = self.get_mobject_family_members()
        claim("TransformMatchingShapes is an outro for its source",
              source not in family and target in family and np.allclose(source.get_center(), before))

        morphed, shape = Circle(), Square()
        self.add(morphed)
        self.play(Transform(morphed, shape), run_time=0.1)
        claim("Transform keeps the source on scene with the target's geometry, target stays off",
              morphed in self.mobjects and shape not in self.mobjects and np.isclose(morphed.width, shape.width))

        absent = Circle()
        self.play(Indicate(absent), run_time=0.1)
        claim("Animating an absent object adds it implicitly", absent in self.mobjects)

        # Q7: every .animate of one object in a play shares a single target, so two of them
        # in a Succession do not chain. Transform-based animations, which build their target
        # when they begin, do.
        mover = Dot()
        self.add(mover)
        heights = []
        sample = lambda dt: heights.append(mover.get_center()[1])
        self.add_updater(sample)
        self.play(Succession(mover.animate.shift(UP), mover.animate.shift(DOWN)), rate_func=linear)
        self.remove_updater(sample)
        claim("Q7 two .animate of one object in a Succession do not go up and then down",
              max(heights) < 0.5 and np.isclose(mover.get_center()[1], -1.0),
              f"highest {max(heights):.2f}, ends at {mover.get_center()[1]:.2f}")
        turner = Square()
        self.add(turner)
        self.play(Succession(Rotate(turner, PI / 2), turner.animate.shift(UP)), run_time=0.4)
        claim("Q7 a begin-time animation followed by one .animate does chain",
              np.allclose(turner.get_center(), UP))
        self.remove(mover, turner)

        # .animate interpolates between two states rather than replaying the method.
        bar = Line(LEFT, RIGHT)
        self.add(bar)
        widths = []
        bar.add_updater(lambda m: widths.append(m.width))
        self.play(bar.animate(suspend_mobject_updating=False).rotate(PI), run_time=0.5, rate_func=linear)
        claim(".animate.rotate(PI) shrinks through the centre instead of turning", min(widths) < 0.1)


Probe().render()
print(f"manim {__import__('manim').__version__}")
width = max(len(name) for name, _, _ in results)
for name, holds, detail in results:
    print(f"  {'confirmed' if holds else 'REFUTED  '}  {name:<{width}}  {detail}")
raise SystemExit(0 if all(holds for _, holds, _ in results) else 1)
