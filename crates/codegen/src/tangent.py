class Tangent(Line):
    """The line that touches a graph where x is `at`: through the graph's point there, and
    as steep as the graph is there.

    How steep is worked out from the graph's rule, by trying a little to either side of
    the point, so nobody has to know the derivative. `dot` marks the point where the two
    touch. `touch` puts the line somewhere else along the graph, and `Glide` takes it
    there; `slope` is how steep the graph is where it touches now.
    """

    def __init__(self, graph, at=0.0, length=3.0, dot=False, **kwargs):
        super().__init__(LEFT, RIGHT, **kwargs)
        self.graph = graph
        self.length = float(length)
        self.at, self.steep = float(at), 0.0
        self.mark = None
        # Nothing to touch: the rule has no value where the line was put.
        self.lost = not self.reaches(at)
        self.touch(at)
        if dot:
            self.mark = Dot(self.get_center(), color=self.get_color())
            self.add(self.mark)

    def value(self, x):
        try:
            with np.errstate(all="ignore"):
                return float(self.graph.function(x))
        except Exception:
            return float("nan")

    def steepness(self, x):
        step = 1e-5 * max(1.0, abs(x))
        return (self.value(x + step) - self.value(x - step)) / (2 * step)

    def reaches(self, x):
        return bool(np.isfinite(self.value(float(x))) and np.isfinite(self.steepness(float(x))))

    def touch(self, x):
        """Puts the line where the graph's x is `x`. Where the rule has no value, or is
        too steep to say, the line stays where it is."""
        x = float(x)
        if not self.reaches(x):
            return self
        y, slope = self.value(x), self.steepness(x)
        # A graph can have been moved or stretched since it was drawn.
        where = getattr(self.graph, "where", lambda x, y: np.array([x, y, 0.0]))
        middle = where(x, y)
        along = where(x + 1.0, y + slope) - middle
        size = float(np.linalg.norm(along))
        if size < 1e-12:
            return self
        half = along / size * self.length / 2
        self.set_points_by_ends(middle - half, middle + half)
        if self.mark is not None:
            self.mark.move_to(middle)
        self.at, self.steep = x, slope
        return self

    @property
    def slope(self):
        """How steep the graph is where the line touches it: its derivative there."""
        return self.steep


class Glide(Animation):
    """Takes a tangent along its graph to where x is `to`, touching it all the way.

    Changing where a tangent touches with `.animate` would not do: that moves the two ends
    of the line straight to where they end up, and on the way the line touches nothing.
    """

    def __init__(self, mobject, to, **kwargs):
        self.to = float(to)
        super().__init__(mobject, **kwargs)

    def begin(self):
        self.start = self.mobject.at
        super().begin()

    def interpolate_mobject(self, alpha):
        self.mobject.touch(self.start + (self.to - self.start) * self.rate_func(alpha))
