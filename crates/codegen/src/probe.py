import copy
import hashlib
import io
import os
import pickle
import types
import zlib


class _Abandoned(BaseException):
    """Nobody wants the result any more. Not an `Exception`, so that nothing on the way out
    mistakes it for a problem with the deck."""


def _digest(*parts):
    whole = hashlib.blake2b(digest_size=12)
    for part in parts:
        whole.update(part if isinstance(part, bytes) else str(part).encode())
        whole.update(b"\0")
    return whole.hexdigest()


class _World:
    """A scene that is kept between probes, and what it looked like before every slide.

    Running a deck in Manim means running it from its first slide, and most of the time
    that is spent on slides nobody has touched. So the scene and the deck's variables are
    saved before each slide, under a name made of everything that was run to get there,
    and a later probe starts from the furthest state whose name still fits its deck.

    A scene is never rendered here, only driven: Manim does not mind `play` and `add`
    being called from outside `construct`.
    """

    # Arrays from this size are saved once and shared between the states: the pixels of
    # a photograph are many megabytes, and are in every state after the slide that adds it.
    BIG = 1 << 18

    def __init__(self, key, base):
        self.scene = Scene()
        # Nothing is ever drawn: what is on scene is read off the objects.
        self.scene.renderer.update_frame = lambda *args, **kwargs: None
        # The deck's variables live in `space`, the globals of every slide's code. An
        # updater made there looks its objects up in it by name when it runs, so putting
        # other objects under those names is all that restoring takes.
        self.space = {name: value for name, value in base.items() if name != "_SLIDES"}
        self.space["self"] = self.scene
        self.space["_probe"] = None
        self.base = set(self.space)
        # Which of the deck's variables each object is, for telling the editor.
        self.names = {}
        self.saved = {}
        # The name of the state the scene is in, if it is exactly a saved-or-savable one.
        self.at = key
        self.arrays = {}
        # Pictures written out so far: what they show, to the file it is in.
        self.pictures = {}
        self.save(key)

    def fixtures(self):
        scene = self.scene
        return scene, scene.renderer, scene.camera

    def state(self):
        variables = {name: value for name, value in self.space.items() if name not in self.base}
        scene = self.scene
        return variables, scene.mobjects, scene.foreground_mobjects, self.names

    def save(self, key):
        """Keeps the present state as `key`: pickled if it can be, copied if not."""
        if key in self.saved:
            return
        world = self
        kept = []
        used = set()
        fixtures = self.fixtures()

        class Saver(pickle.Pickler):
            def persistent_id(self, value):
                # Functions stay the very same: Python cannot write a lambda down, and
                # the ones here do not hold objects, they look them up.
                # A class the script defined itself is kept the same way, there being no
                # module to find it in again.
                own = isinstance(value, type) and value.__module__ == "__main__"
                if isinstance(value, types.FunctionType) or own:
                    kept.append(value)
                    return "function", len(kept) - 1
                if isinstance(value, np.ndarray) and value.nbytes >= world.BIG:
                    name = _digest(value.shape, value.dtype, value.tobytes())
                    if name not in world.arrays:
                        world.arrays[name] = value.copy()
                    used.add(name)
                    return "array", name
                # The scene itself is not part of its state.
                for which, fixture in enumerate(fixtures):
                    if value is fixture:
                        return "fixture", which
                return None

        try:
            written = io.BytesIO()
            Saver(written, protocol=pickle.HIGHEST_PROTOCOL).dump(self.state())
            self.saved[key] = ("pickled", written.getvalue(), kept, used)
        except Exception:
            try:
                self.saved[key] = ("copied", copy.deepcopy(self.state()), None, set())
            except Exception:
                # Then this slide is simply run again when it is needed.
                pass

    def restore(self, key):
        how, state, kept, _ = self.saved[key]
        world = self

        class Loader(pickle.Unpickler):
            def persistent_load(self, name):
                kind, which = name
                if kind == "function":
                    return kept[which]
                if kind == "array":
                    return world.arrays[which].copy()
                return world.fixtures()[which]

        if how == "pickled":
            state = Loader(io.BytesIO(state)).load()
        else:
            state = copy.deepcopy(state)
        variables, mobjects, foreground, names = state
        for name in [name for name in self.space if name not in self.base]:
            del self.space[name]
        self.space.update(variables)
        scene = self.scene
        scene.mobjects, scene.foreground_mobjects, self.names = mobjects, foreground, names
        scene.updaters = []
        self.at = key

    def keep(self, keys):
        """Forgets the states that are not on the way through the deck as it is now."""
        self.saved = {key: state for key, state in self.saved.items() if key in keys}
        used = set().union(*(state[3] for state in self.saved.values()))
        self.arrays = {name: array for name, array in self.arrays.items() if name in used}


class _Probe:
    """Records what is on scene, as outlines the editor can draw: after every step, or at
    every frame of one slide. The slides' code calls it after everything it does."""

    def __init__(self, world, out):
        self.world = world
        self.scene = world.scene
        self.out = out
        # What the slide now running is run for: "quiet" to get past it, "steps" to see
        # the scene after each of its steps, "record" to see every frame.
        self.mode = "quiet"
        self.steps = []
        self.slide = 0
        self.error = None
        # What was found odd along the way, for the editor to pass on: each once a slide.
        self.remarks = []
        self.remarked = set()
        # The objects named since the last look at the scene.
        self.named = []
        # For a recording: the distinct outlines seen, and per frame the ones drawn.
        self.pool = None
        self.seen = {}
        self.frames = []
        self.marks = []

    @staticmethod
    def check():
        """Stops the run if whoever started it has said the result is not wanted."""
        if not globals().get("_wanted", lambda: True)():
            raise _Abandoned

    # ── What the slides' code calls ──────────────────────────────────────────

    def enter(self, slide):
        self.slide = slide
        self.scene.next_section(skip_animations=self.mode != "record")
        if self.mode == "steps":
            self.step(None)
        elif self.mode == "record":
            self.pool = []
            # Scene updaters run last in every frame, after animations and object
            # updaters, so this sees exactly the state that would be drawn.
            self.scene.add_updater(lambda dt: self.frames.append(self.capture()))

    def name(self, var, mobject):
        self.world.names[var] = mobject
        self.named.append(var)

    def after(self, node):
        """Step `node` has just been run."""
        if self.mode == "steps":
            self.step(node)
        elif self.mode == "record":
            self.marks.append([node, len(self.frames)])
        else:
            self.check()

    def wait(self, seconds, frames, live):
        if self.mode == "record" and not live:
            # A still wait: the same frame, `frames` times. Asking Manim to wait would
            # not do, because being recorded makes it treat every wait as live.
            self.frames.extend([self.capture()] * frames)
        elif live:
            self.scene.wait(seconds, frozen_frame=False)
        else:
            self.scene.wait(seconds)

    # ── Looking at the scene ─────────────────────────────────────────────────

    @staticmethod
    def whole(mobject):
        """Whether every point of the object is a number. A graph of a rule that has no
        value somewhere in its range has points that are not, and Manim then draws none
        of that object, without a word."""
        return bool(np.isfinite(mobject.points).all())

    def remark(self, var, node, what):
        if (self.slide, var, what) not in self.remarked:
            self.remarked.add((self.slide, var, what))
            self.remarks.append({"slide": self.slide, "node": node, "var": var, "what": what})

    def owners(self):
        """For every part of a named object: which names reach it, and by which path."""
        owners = {}

        def walk(var, mobject, path):
            owners.setdefault(id(mobject), []).append([var, path])
            for index, child in enumerate(mobject.submobjects):
                walk(var, child, path + [index])

        for var, mobject in self.world.names.items():
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

    def picture(self, image):
        """A picture as the editor can draw it: a file with its pixels, where three of its
        corners are, and how opaque it is.

        The pixels are written once per distinct picture. An image that is fading changes
        its alpha at every frame, so that is left out of the file and reported apart.
        """
        from PIL import Image

        pixels = image.pixel_array
        solid = getattr(image, "orig_alpha_pixel_array", pixels[:, :, 3])
        key = (pixels.shape, zlib.crc32(pixels[:, :, :3].tobytes()), zlib.crc32(solid.tobytes()))
        file = self.world.pictures.get(key)
        if file is None:
            whole = np.dstack((pixels[:, :, :3], solid)).astype(np.uint8)
            folder = os.path.join(globals().get("_scratch", "."), "pictures")
            os.makedirs(folder, exist_ok=True)
            file = os.path.abspath(os.path.join(folder, f"{key[1]:08x}{key[2]:08x}.png"))
            scaled = Image.fromarray(whole, "RGBA")
            # No screen shows more than this; a camera's photograph has far more.
            scaled.thumbnail((2048, 2048))
            scaled.save(file)
            self.world.pictures[key] = file
        return {
            "file": file,
            "corners": np.round(image.points[:3, :2], 5).tolist(),
            # Read off the pixels, which are what Manim draws; the attributes that are
            # meant to say the same do not all follow a fade.
            "opacity": float(pixels[:, :, 3].max()) / max(float(solid.max()), 1.0),
        }

    def members(self):
        from manim.utils.family import extract_mobject_family_members

        return extract_mobject_family_members(
            self.scene.mobjects, use_z_index=True, only_those_with_points=True
        )

    def step(self, node):
        """Notes the scene as it is after step `node`."""
        from manim.mobject.types.image_mobject import AbstractImageMobject

        self.check()
        owners = self.owners()
        members = self.members()
        drawn = []
        for member in members:
            if isinstance(member, AbstractImageMobject):
                drawn.append({
                    "owners": owners.get(id(member), []),
                    "paths": [],
                    "fill": [0.0] * 4,
                    "stroke": [0.0] * 4,
                    "stroke_width": 0.0,
                    "picture": self.picture(member),
                })
                continue
            if not isinstance(member, VMobject) or not self.whole(member):
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
        for var, mobject in self.world.names.items():
            # Only what can be seen has a place on the canvas; a value tracker does not.
            seen = (VMobject, AbstractImageMobject)
            family = [m for m in mobject.family_members_with_points() if isinstance(m, seen)]
            if not family:
                continue
            if not all(self.whole(member) for member in family):
                self.remark(var, node, "undefined")
                continue
            low, high = mobject.get_critical_point(DL), mobject.get_critical_point(UR)
            boxes[str(var)] = {
                "rect": [float(low[0]), float(low[1]), float(high[0]), float(high[1])],
                "on": any(id(member) in on_scene for member in family),
            }
        for var in self.named:
            found = getattr(self.world.names.get(var), "found", None)
            if isinstance(found, dict):
                self.remarks.append({"slide": self.slide, "node": node, "var": var, "what": "plot", **found})
        self.named = []
        self.steps.append({"slide": self.slide, "node": node, "drawn": drawn, "boxes": boxes})

    def capture(self):
        """What is drawn right now, as indices into the pool of distinct outlines."""
        from manim.mobject.types.image_mobject import AbstractImageMobject

        self.check()
        frame = []
        for member in self.members():
            if isinstance(member, AbstractImageMobject):
                picture = self.picture(member)
                key = (picture["file"], member.points.tobytes(), picture["opacity"])
                index = self.seen.get(key)
                if index is None:
                    index = self.seen[key] = len(self.pool)
                    self.pool.append({
                        "paths": [],
                        "fill": [0.0] * 4,
                        "stroke": [0.0] * 4,
                        "stroke_width": 0.0,
                        "picture": picture,
                    })
                frame.append(index)
                continue
            if not isinstance(member, VMobject) or not self.whole(member):
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

    def finish(self, first):
        result = {"first": first, "steps": self.steps, "error": self.error}
        result["remarks"] = self.remarks
        if self.pool is not None:
            result.update(pool=self.pool, frames=self.frames, marks=self.marks)
        try:
            # `dumps` uses the fast encoder; `dump` to a file does not.
            written = json.dumps(result, allow_nan=False)
        except ValueError:
            # Something that is not a number got through, which no reader of JSON takes.
            # Rather than have nothing to show at all, show it with nothing in its place.
            def numbers(value):
                if isinstance(value, float):
                    return value if value == value and abs(value) != float("inf") else 0.0
                if isinstance(value, dict):
                    return {key: numbers(item) for key, item in value.items()}
                if isinstance(value, (list, tuple)):
                    return [numbers(item) for item in value]
                return value

            result = numbers(result)
            if result["error"] is None:
                result["error"] = (
                    "Something on the slide has a size or a place that is not a number, "
                    "as after a division by zero"
                )
            written = json.dumps(result, allow_nan=False)
        with open(self.out, "w", encoding="utf-8") as file:
            file.write(written)


def _run(slides, what, have, last, out):
    """Runs the deck as far as slide `last` and writes what was asked for to `out`.

    `what` is "steps" for the scene after every step of the slides from `have` on, or
    "frames" for every frame of slide `last`. The slides before those are only run if
    there is no saved state to start from instead.
    """
    # Kept by whoever runs one script after another in the same Python; alone, a script
    # has only itself.
    worlds = globals().get("_worlds")
    if worlds is None:
        worlds = {}
    root = _digest(config.frame_rate, config.frame_height, config.frame_width)
    if root not in worlds:
        worlds.clear()
        worlds[root] = _World(root, globals())
    world = worlds[root]
    # The state before slide `i` is named after everything run to get there.
    keys = [root]
    for code in slides:
        keys.append(_digest(keys[-1], code))
    world.keep(set(keys))

    first = last if what == "frames" else min(have, last)
    probe = _Probe(world, out)
    world.space["_probe"] = probe
    try:
        # From where the scene is, if that is on the way; else from the furthest saved
        # state before the first slide that is to be looked at.
        start = max(i for i in range(first + 1) if keys[i] in world.saved or keys[i] == world.at)
        if world.at != keys[start]:
            world.restore(keys[start])
        for index in range(start, last + 1):
            if index < first:
                probe.mode = "quiet"
            else:
                probe.mode = "steps" if what == "steps" else "record"
            world.at = None
            exec(compile(slides[index], f"slide {index + 1}", "exec"), world.space)
            # A slide that was played for real has been through other frames than one
            # that was skipped through, so only the latter is a state to start from.
            if probe.mode != "record":
                world.at = keys[index + 1]
                world.save(keys[index + 1])
    except _Abandoned:
        # Nobody will read it, and writing half a recording down takes its time.
        probe.out = None
        raise
    except Exception as error:
        probe.error = f"{type(error).__name__}: {error}"
    finally:
        world.scene.updaters = []
        world.space["_probe"] = None
        if probe.out is not None:
            probe.finish(first)
