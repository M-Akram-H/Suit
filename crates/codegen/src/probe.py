class _Probe:
    """Records what is on scene after every step, as outlines the editor can draw."""

    def __init__(self, scene, out):
        self.scene = scene
        self.out = out
        self.names = {}
        self.steps = []
        self.slide = 0
        self.error = None

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
        paths = []
        for subpath in mobject.get_subpaths():
            if len(subpath) < 4:
                continue
            curves = subpath[: len(subpath) // 4 * 4].reshape(-1, 4, 3)
            points = [curves[0][0]] + [point for curve in curves for point in curve[1:]]
            paths.append({
                "points": [[round(float(p[0]), 5), round(float(p[1]), 5)] for p in points],
                "closed": bool(np.allclose(subpath[0], subpath[-1])),
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

    def finish(self):
        with open(self.out, "w", encoding="utf-8") as file:
            json.dump({"steps": self.steps, "error": self.error}, file)
