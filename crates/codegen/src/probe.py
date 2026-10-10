class _Probe:
    """Records what is on scene, as outlines the editor can draw: after every step, or at
    every frame of one slide."""

    def __init__(self, scene, out):
        self.scene = scene
        self.out = out
        self.names = {}
        self.steps = []
        self.slide = 0
        self.error = None
        # Set by `record`: the distinct outlines seen, and per frame the ones drawn.
        self.pool = None
        self.seen = {}
        self.frames = []
        self.marks = []

    def name(self, var, mobject):
        self.names[var] = mobject

    def enter(self, slide):
        self.slide = slide
        self.step(None)

    def owners(self):
        """For every part of a named object: which names reach it, and by which path."""
        owners = {}

        def walk(var, mobject, path):
            owners.setdefault(id(mobject), []).append([var, path])
            for index, child in enumerate(mobject.submobjects):
                walk(var, child, path + [index])

        for var, mobject in self.names.items():
            walk(var, mobject, [])
        return owners

    @staticmethod
    def outline(mobject):
        """The object's curves as chains: a start point, then three points per curve.

        A new chain starts wherever one curve does not begin where the last one ended, which
        is how Manim itself tells sub-paths apart; done on whole arrays here, because a
        slide can have hundreds of thousands of points.
        """
        points = mobject.points[: len(mobject.points) // 4 * 4, :2]
        if len(points) == 0:
            return []
        curves = points.reshape(-1, 4, 2)
        apart = np.abs(curves[1:, 0] - curves[:-1, 3]).max(axis=1) > 1e-6
        starts = [0, *(np.flatnonzero(apart) + 1), len(curves)]
        paths = []
        for first, after in zip(starts, starts[1:]):
            chain = curves[first:after]
            ordered = np.concatenate((chain[0, :1], chain[:, 1:].reshape(-1, 2)))
            paths.append({
                "points": np.round(ordered, 5).tolist(),
                "closed": bool(np.abs(chain[0, 0] - chain[-1, 3]).max() <= 1e-6),
            })
        return paths

    def step(self, node):
        from manim.utils.family import extract_mobject_family_members

        owners = self.owners()
        members = extract_mobject_family_members(
            self.scene.mobjects, use_z_index=True, only_those_with_points=True
        )
        drawn = []
        for member in members:
            if not isinstance(member, VMobject):
                continue
            fill = [float(c) for c in member.get_fill_rgbas()[0]]
            stroke = [float(c) for c in member.get_stroke_rgbas()[0]]
            width = float(member.get_stroke_width())
            if fill[3] <= 0 and (stroke[3] <= 0 or width <= 0):
                continue
            drawn.append({
                "owners": owners.get(id(member), []),
                "paths": self.outline(member),
                "fill": fill,
                "stroke": stroke,
                "stroke_width": width,
            })
        boxes = {}
        on_scene = {id(member) for member in members}
        for var, mobject in self.names.items():
            # Only what can be seen has a place on the canvas; a value tracker does not.
            family = [m for m in mobject.family_members_with_points() if isinstance(m, VMobject)]
            if not family:
                continue
            low, high = mobject.get_critical_point(DL), mobject.get_critical_point(UR)
            boxes[str(var)] = {
                "rect": [float(low[0]), float(low[1]), float(high[0]), float(high[1])],
                "on": any(id(member) in on_scene for member in family),
            }
        self.steps.append({"slide": self.slide, "node": node, "drawn": drawn, "boxes": boxes})

    def quiet(self):
        """Run without drawing anything: only the state of the scene is wanted."""
        self.scene.renderer.update_frame = lambda *args, **kwargs: None
        self.scene.next_section(skip_animations=True)

    def record(self):
        """From here on, note what is drawn at every frame Manim would render."""
        self.scene.next_section(skip_animations=False)
        self.pool = []
        # Scene updaters run last in every frame, after animations and object updaters,
        # so this sees exactly the state that would be drawn.
        self.scene.add_updater(lambda dt: self.frames.append(self.capture()))

    def capture(self):
        """What is drawn right now, as indices into the pool of distinct outlines."""
        from manim.utils.family import extract_mobject_family_members

        members = extract_mobject_family_members(
            self.scene.mobjects, use_z_index=True, only_those_with_points=True
        )
        frame = []
        for member in members:
            if not isinstance(member, VMobject):
                continue
            fill, stroke = member.get_fill_rgbas()[0], member.get_stroke_rgbas()[0]
            width = float(member.get_stroke_width())
            if fill[3] <= 0 and (stroke[3] <= 0 or width <= 0):
                continue
            # Most outlines do not change from one frame to the next; each distinct one is
            # written out once.
            key = (member.points.tobytes(), fill.tobytes(), stroke.tobytes(), width)
            index = self.seen.get(key)
            if index is None:
                index = self.seen[key] = len(self.pool)
                self.pool.append({
                    "paths": self.outline(member),
                    "fill": [float(c) for c in fill],
                    "stroke": [float(c) for c in stroke],
                    "stroke_width": width,
                })
            frame.append(index)
        return frame

    def hold(self, frames):
        """A still wait: the same frame, `frames` times, without asking Manim to wait."""
        self.frames.extend([self.capture()] * frames)

    def mark(self, node):
        """Step `node` ends here."""
        self.marks.append([node, len(self.frames)])

    def finish(self):
        result = {"steps": self.steps, "error": self.error}
        if self.pool is not None:
            result.update(pool=self.pool, frames=self.frames, marks=self.marks)
        # `dumps` uses the fast encoder; `dump` to a file does not.
        with open(self.out, "w", encoding="utf-8") as file:
            file.write(json.dumps(result))
