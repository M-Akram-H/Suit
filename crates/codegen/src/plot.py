class Plot(VMobject):
    """The graph of a rule, drawn where the rule has a value and nowhere else.

    Manim's own graph is the rule worked out at many points and joined up, whatever comes
    out. Where the rule has no value, like the logarithm of a negative number, the whole
    graph is lost; across a pole, like that of 1/x at 0, the two branches are joined by a
    stroke from the bottom of the picture to the top. This looks first: it works out where
    the rule has a value that is anywhere near the picture, and draws those stretches,
    each for itself.

    `on` is the intervals to look at, each a pair of numbers (what follows the first two
    is ignored); without it, the width of the frame. `asymptotes` adds a dashed line for
    every asymptote found. What was found is left in `found`, for whoever wants to say so.
    """

    def __init__(self, function, on=None, step=0.02, asymptotes=False, **kwargs):
        super().__init__(**kwargs)
        # Further from the middle than this, a branch has left the picture for good.
        reach = 2 * config.frame_height

        def value(x):
            try:
                with np.errstate(all="ignore"):
                    return float(function(x))
            except Exception:
                return float("nan")

        def drawn(y):
            return bool(np.isfinite(y)) and abs(y) <= reach

        def last(good, bad, keeps=lambda x: drawn(value(x))):
            """Between a point that `keeps` and one that does not: the last that does."""
            for _ in range(48):
                middle = (good + bad) / 2
                if keeps(middle):
                    good = middle
                else:
                    bad = middle
            return good

        def runs_off(start, end, xs, ys):
            """Where in a gap the rule runs off to infinity, if it does: the point at which
            its values grow without end, or past which it has none."""
            inside = [(x, y) for x, y in zip(xs, ys) if start < x < end]
            points = [(start, value(start)), *inside, (end, value(end))]
            for _ in range(60):
                finite = [(abs(y), index) for index, (_, y) in enumerate(points) if np.isfinite(y)]
                if not finite:
                    return None
                size, index = max(finite)
                if size > 1e6 * reach:
                    return points[index][0]
                # Look closer on both sides of the largest value found so far.
                closer = []
                for other in (index - 1, index + 1):
                    if 0 <= other < len(points):
                        middle = (points[index][0] + points[other][0]) / 2
                        closer.append((middle, value(middle)))
                points = sorted({*points, *closer})
            # It grew, but slowly, as a logarithm does: an asymptote if it ends where the
            # rule stops having a value at all.
            size, index = max((abs(y), index) for index, (_, y) in enumerate(points) if np.isfinite(y))
            nothing = [x for x, y in points if not np.isfinite(y)]
            if size <= reach or not nothing:
                return None
            x = points[index][0]
            nearest = min(nothing, key=lambda other: abs(other - x))
            return last(x, nearest, keeps=lambda x: bool(np.isfinite(value(x))))

        width = float(config.frame_x_radius)
        asked = [(-width, width)]
        if on is not None and len(on) > 0:
            asked = [(float(piece[0]), float(piece[1])) for piece in on]
        pieces, vertical, missing = [], [], False
        for start, end in asked:
            if not end > start:
                continue
            count = min(int(np.ceil((end - start) / max(step, 1e-4))) + 1, 20000)
            xs = np.linspace(start, end, max(count, 2))
            ys = np.array([value(x) for x in xs])
            missing = missing or not bool(np.isfinite(ys).all())
            good = np.isfinite(ys) & (np.abs(ys) <= reach)
            # A step out of all proportion to the steps around it is a break as well: a
            # pole that fell between two points, or the riser of a staircase.
            rise = np.abs(np.diff(ys))
            rise[~np.isfinite(rise)] = 0.0
            around = np.zeros_like(rise)
            around[1:] += rise[:-1]
            around[:-1] += rise[1:]
            broken = rise > 0.5 + 4 * around

            ends = []
            first = 0
            while first < len(xs):
                if not good[first]:
                    first += 1
                    continue
                final = first
                while final + 1 < len(xs) and good[final + 1] and not broken[final]:
                    final += 1
                left, right = float(xs[first]), float(xs[final])
                if first > 0 and not good[first - 1]:
                    left = last(left, float(xs[first - 1]))
                if final + 1 < len(xs) and not good[final + 1]:
                    right = last(right, float(xs[final + 1]))
                inner = [float(x) for x in xs[first : final + 1] if left < x < right]
                points = [left, *inner, right] if right > left else [left]
                if len(points) > 1:
                    path = np.array([[x, value(x), 0.0] for x in points])
                    self.start_new_path(path[0])
                    self.add_points_as_corners(path[1:])
                    pieces.append((left, right))
                    ends.append((left, right))
                first = final + 1
            # The gaps: before the first stretch, between two, and after the last.
            gaps = zip([start, *(right for _, right in ends)], [*(left for left, _ in ends), end])
            for gap_start, gap_end in gaps:
                if gap_end - gap_start > 1e-9:
                    pole = runs_off(gap_start, gap_end, xs, ys)
                    if pole is not None:
                        vertical.append(float(pole))
        if self.has_points():
            self.make_smooth()

        # A level the rule settles on far out to either side.
        horizontal = []
        for side in (-1.0, 1.0):
            far = [value(side * distance) for distance in (1e4, 1e6, 1e8)]
            settled = all(np.isfinite(far)) and abs(far[2] - far[1]) <= 1e-3 * max(1.0, abs(far[2]))
            settled = settled and abs(far[1] - far[0]) <= 1e-2 * max(1.0, abs(far[2]))
            # A level the graph already lies on at the edge of the picture, as a constant
            # does, would only be a line drawn over the graph.
            apart = abs(value(side * width) - far[2]) > 1e-6
            if settled and apart and not any(abs(far[2] - level) < 1e-3 for level in horizontal):
                horizontal.append(float(far[2]))

        self.found = {
            "asked": asked,
            "pieces": pieces,
            "missing": bool(missing),
            "chosen": on is not None and len(on) > 0,
            # Adding zero turns a minus zero, which rounding leaves behind, into zero.
            "vertical": sorted(set(round(x, 6) + 0.0 for x in vertical)),
            "horizontal": [round(level, 6) + 0.0 for level in horizontal],
        }
        if asymptotes:
            high, wide = float(config.frame_y_radius), width
            look = {"dash_length": 0.12, "stroke_width": 2.0, "color": GRAY}
            for x in self.found["vertical"]:
                self.add(DashedLine([x, -high, 0.0], [x, high, 0.0], **look))
            for level in self.found["horizontal"]:
                if abs(level) < high:
                    self.add(DashedLine([-wide, level, 0.0], [wide, level, 0.0], **look))
