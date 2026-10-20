import copy
import gc
import hashlib
import io
import os
import pickle
import sys
import types
import zlib

import svgelements

# What a saved state refers to instead of containing it: a function, a class the script
# defined, a glyph's SVG path, a large array kept once, the scene itself. Written down as a
# call of `mark`, which only the loader of that state knows how to answer; it lives in a
# module of its own so that pickle can name it.
_marks = sys.modules.setdefault("_suit_marks", types.ModuleType("_suit_marks"))


def _mark(kind, which):
    raise RuntimeError("a saved state is only read back by its own loader")


_mark.__module__, _mark.__qualname__ = "_suit_marks", "mark"
_marks.mark = _mark

# The SVG path a glyph was drawn from. Manim reads it once, while it makes the glyph, and
# never changes it, but every copy of the glyph copies it too: in a text, nine objects in
# ten are parts of those paths. So copies here keep the very same one, those Manim makes
# when an animation starts as well, unless `SUIT_PROBE_EXACT` says to copy as Manim does.
_SVGPath = svgelements.Path
if "__deepcopy__" not in vars(_SVGPath) and not os.environ.get("SUIT_PROBE_EXACT"):
    _SVGPath.__deepcopy__ = lambda path, memo: path


class _Copier(pickle.Pickler):
    """Writes an object down to be copied from, keeping the SVG paths in it as they are,
    in `kept`."""

    def __init__(self, file):
        super().__init__(file, protocol=pickle.HIGHEST_PROTOCOL)
        self.kept = []

    def reducer_override(self, value):
        if type(value) is _SVGPath:
            self.kept.append(value)
            return _mark, ("same", len(self.kept) - 1)
        return NotImplemented


class _Paster(pickle.Unpickler):
    """Reads back what a `_Copier` wrote, with what it kept."""

    def __init__(self, written, kept):
        super().__init__(io.BytesIO(written))
        self.kept = kept

    def find_class(self, module, name):
        if (module, name) == ("_suit_marks", "mark"):
            return lambda kind, which: self.kept[which]
        return super().find_class(module, name)


def _remembered(kind):
    """`kind.__init__` for a text that Pango draws, keeping what it made.

    Such a text is drawn by Pango into an SVG file, read back curve by curve, and its
    curves closed point by point: a fifth of a second for a line, and every edit makes
    all the words of its slide again. What it is made into depends only on what it is made
    from and on the frame, so after the first time it is a copy of the first, by pickling,
    which is ten times as fast. A text keeps nothing that refers to itself, so a copy of
    its attributes is a copy of it. `SUIT_PROBE_EXACT` makes every one anew.
    """
    make = kind.__init__
    # Newest last, and at most this many bytes of them, which with the paths they share
    # with the texts made from them is some hundreds of lines of text.
    kept, budget = {}, 1 << 26
    used = [0]

    def __init__(self, *args, **settings):
        key = None
        if type(self) is kind and not os.environ.get("SUIT_PROBE_EXACT"):
            frame = (config.pixel_width, config.pixel_height, config.frame_width,
                     config.frame_height, str(config.renderer))
            try:
                key = pickle.dumps((args, settings, frame), 5)
            except Exception:  # noqa: BLE001 - something that only Python can name
                pass
        made = kept.pop(key, None) if key is not None else None
        if made is None:
            make(self, *args, **settings)
            if key is None:
                return
            try:
                written = io.BytesIO()
                copier = _Copier(written)
                copier.dump(vars(self))
            except Exception:  # noqa: BLE001 - given something that cannot be copied
                return
            made = written.getvalue(), copier.kept
            used[0] += len(made[0])
            # The ones used longest ago go first.
            while used[0] > budget and kept:
                used[0] -= len(kept.pop(next(iter(kept)))[0])
        else:
            vars(self).update(_Paster(*made).load())
        kept[key] = made

    __init__.__wrapped__ = make
    return __init__


for _kind in (Text, MarkupText):
    if not hasattr(_kind.__init__, "__wrapped__"):
        _kind.__init__ = _remembered(_kind)


class _Abandoned(BaseException):
    """Nobody wants the result any more. Not an `Exception`, so that nothing on the way out
    mistakes it for a problem with the deck."""


def _digest(*parts):
    whole = hashlib.blake2b(digest_size=12)
    for part in parts:
        whole.update(part if isinstance(part, bytes) else str(part).encode())
        whole.update(b"\0")
    return whole.hexdigest()


def _jump(scene, animations):
    """Leaves the scene as playing `animations` to their end would, where that end is
    plain without playing them, and returns the ones that still have to be played.

    An animation that only brings an object in ends with the object on scene as it was
    made; one that only takes it away ends without it, and puts it back as it was; one
    that points at it, there and back, ends with it as it was. Manim gets there by copying
    the object several times over to draw the frames in between, and a text is made of
    many shapes: a deck of texts spent most of a look at its steps copying them.

    Each of those only ends that way with an easing that ends where it should: a FadeIn
    eased there and back ends unseen, an Indicate eased smoothly ends large. Uncreate and
    Unwrite leave their object folded up to nothing, which is not plain either. Those, and
    everything else, are played, and so is everything while some object follows another,
    which only playing can say. `SUIT_PROBE_EXACT` plays every animation, for comparing.
    """
    if os.environ.get("SUIT_PROBE_EXACT"):
        return animations
    from manim.animation.composition import AnimationGroup

    # Not AddTextLetterByLetter: being made, it makes the letters see-through, and only
    # its frames make them seen again.
    brought_in = (FadeIn, Create, Write, DrawBorderThenFill, GrowFromPoint)
    shown = []

    def ends(animation, *where):
        end = animation.rate_func(1.0)
        return any(abs(end - value) < 1e-9 for value in where)

    def put_on(animation, plan):
        # What is not brought in, Manim puts on scene for the animation, if it is not.
        if not shown:
            shown.append({id(mobject) for mobject in scene.get_mobject_family_members()})
        if id(animation.mobject) not in shown[0]:
            plan.append((scene.add, animation.mobject))
        return True

    def walk(animation, plan):
        if isinstance(animation, (FadeOut, ShowPassingFlash)):
            # Both put the object back as it was on taking it away, however eased.
            plan.append((scene.remove, animation.mobject))
            return True
        if isinstance(animation, (Circumscribe, Flash, FocusOn)):
            # Frames, lines and dots of their own, gone at the end; the object untouched.
            return True
        if isinstance(animation, Indicate):
            return ends(animation, 0) and put_on(animation, plan)
        if isinstance(animation, (Wiggle, ApplyWave)):
            return ends(animation, 0, 1) and put_on(animation, plan)
        # Before the groups: some of these are made of groups.
        if isinstance(animation, brought_in):
            # Uncreate is a Create, Unwrite a Write: those take away.
            if animation.is_remover() or not ends(animation, 1):
                return False
            plan.append((scene.add, animation.mobject))
            return True
        if isinstance(animation, AnimationGroup):
            # A group is jumped as a whole or played as a whole.
            return ends(animation, 1) and all(walk(part, plan) for part in animation.animations)
        return False

    def follows(mobject):
        return any(member.updaters for member in mobject.get_family())

    if scene.updaters or any(follows(mobject) for mobject in scene.mobjects):
        return animations
    jumped, played = [], []
    for animation in animations:
        plan = []
        if walk(animation, plan) and not any(follows(mobject) for _, mobject in plan):
            jumped += plan
        else:
            played.append(animation)
    # Animations of one step act on different objects (the checker sees to it), so those
    # jumped and those played do not meet: only the order of the scene's list could tell
    # them apart, and every object is drawn by its own layer.
    for do, mobject in jumped:
        do(mobject)
    return played


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
        # Where nothing is drawn between two steps, an animation only has to leave things
        # as Manim would after its last frame; for some that is plain without playing them.
        playing = self.scene.play

        def play(*animations, **settings):
            probe = self.space.get("_probe")
            if probe is not None and probe.mode != "record":
                animations = _jump(self.scene, animations)
            if animations:
                playing(*animations, **settings)

        self.scene.play = play
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
        # The name of the state the scene is in, if it is exactly a saved-or-savable one,
        # and the same name while that state is still to be saved.
        self.at = key
        self.unsaved = None
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
        fixtures = {id(fixture): which for which, fixture in enumerate(self.fixtures())}
        array, function = np.ndarray, types.FunctionType

        class Saver(pickle.Pickler):
            # Asked about every object but numbers, strings and plain containers, which
            # are most of a scene: a hook that Python calls for each of those as well
            # (`persistent_id`) took longer than the pickling itself. Most of what is left
            # is small arrays, so those are let through first.
            def reducer_override(self, value):
                kind = type(value)
                if kind is array and value.nbytes < world.BIG:
                    return NotImplemented
                # Functions stay the very same: Python cannot write a lambda down, and
                # the ones here do not hold objects, they look them up. So do the paths
                # glyphs were drawn from (`_SVGPath`), most of the objects in a text.
                # A class the script defined itself is kept the same way, there being no
                # module to find it in again.
                if kind is function or kind is _SVGPath:
                    if value is _mark:
                        return NotImplemented
                    kept.append(value)
                    return _mark, ("same", len(kept) - 1)
                if isinstance(value, type) and value.__module__ == "__main__":
                    kept.append(value)
                    return _mark, ("same", len(kept) - 1)
                # The scene itself is not part of its state.
                which = fixtures.get(id(value))
                if which is not None:
                    return _mark, ("fixture", which)
                if isinstance(value, array) and value.nbytes >= world.BIG:
                    name = _digest(value.shape, value.dtype, value.tobytes())
                    if name not in world.arrays:
                        world.arrays[name] = value.copy()
                    used.add(name)
                    return _mark, ("array", name)
                return NotImplemented

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

    def leave(self):
        """Keeps the state the scene is in before anything changes it, if it is one to
        start from and not kept yet.

        Saving is left until then because most of the time it never comes: while words
        are typed into a slide, every key brings a probe of that slide from the state
        before it, and the one after it is out of date at once.
        """
        if self.unsaved is not None and self.unsaved == self.at:
            self.save(self.at)
        self.unsaved = self.at = None

    def restore(self, key):
        how, state, kept, _ = self.saved[key]
        world = self

        def answer(kind, which):
            if kind == "same":
                return kept[which]
            if kind == "array":
                return world.arrays[which].copy()
            return world.fixtures()[which]

        class Loader(pickle.Unpickler):
            def find_class(self, module, name):
                if (module, name) == ("_suit_marks", "mark"):
                    return answer
                return super().find_class(module, name)

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
        self.at, self.unsaved = key, None

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
        # What is known of each set of points shown on this slide (`shape`).
        self.shapes = {}
        self.frames = []
        self.marks = []

    @staticmethod
    def check():
        """Stops the run if whoever started it has said the result is not wanted, and
        collects if a long one has left much behind (see `_run`)."""
        if not globals().get("_wanted", lambda: True)():
            raise _Abandoned
        if gc.get_count()[0] > _PILED_UP:
            gc.collect()

    # ── What the slides' code calls ──────────────────────────────────────────

    def enter(self, slide):
        self.slide = slide
        self.shapes = {}
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

    @staticmethod
    def family(mobject):
        """The object and all its parts, each once: what Manim's `get_family` gives, in
        another order, which a bound does not mind, and in a fraction of the time."""
        family, seen, left = [], set(), [mobject]
        while left:
            member = left.pop()
            if id(member) not in seen:
                seen.add(id(member))
                family.append(member)
                left.extend(member.submobjects)
        return family

    def shape(self, member):
        """What a step needs of an object's own points: whether they are all numbers, the
        corners of all of them and of its curves' anchors, and its outline once asked for.

        Most objects stay as they are from one step to the next, and a group's parts are
        also parts of the objects in it, so this is worked out once for each set of points
        a slide shows.
        """
        points = member.points
        every = member.n_points_per_cubic_curve if isinstance(member, VMobject) else 1
        key = (every, points.dtype.str, points.shape, points.tobytes())
        known = self.shapes.get(key)
        if known is None:
            ends = points
            if every > 1 and len(points) > 1:
                starts, finishes = points[::every], points[every - 1 :: every]
                count = min(len(starts), len(finishes))
                ends = np.concatenate((starts[:count], finishes[:count]))
            known = self.shapes[key] = {
                "whole": bool(np.isfinite(points).all()),
                "all": (points.min(axis=0), points.max(axis=0)),
                "anchors": (ends.min(axis=0), ends.max(axis=0)) if len(ends) else None,
                "paths": None,
            }
        return known

    def corners(self, mobject, parts):
        """What Manim's `get_critical_point(DL)` and `(UR)` give, from the `parts` of the
        object that have points: a vector object is bounded by the anchors of its curves,
        anything else by all its points. Asked of Manim corner by corner, after every
        step, it was most of what a step took."""
        which = "anchors" if isinstance(mobject, VMobject) else "all"
        bounds = [bound for bound in (self.shape(part)[which] for part in parts) if bound]
        if not bounds:
            return np.zeros(3), np.zeros(3)
        lows, highs = zip(*bounds)
        return np.min(lows, axis=0), np.max(highs, axis=0)

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
            if not isinstance(member, VMobject):
                continue
            shape = self.shape(member)
            if not shape["whole"]:
                continue
            fill = [float(c) for c in member.get_fill_rgbas()[0]]
            stroke = [float(c) for c in member.get_stroke_rgbas()[0]]
            width = float(member.get_stroke_width())
            if fill[3] <= 0 and (stroke[3] <= 0 or width <= 0):
                continue
            if shape["paths"] is None:
                shape["paths"] = self.outline(member)
            drawn.append({
                "owners": owners.get(id(member), []),
                "paths": shape["paths"],
                "fill": fill,
                "stroke": stroke,
                "stroke_width": width,
            })
        boxes = {}
        on_scene = {id(member) for member in members}
        for var, mobject in self.world.names.items():
            # Only what can be seen has a place on the canvas; a value tracker does not.
            seen = (VMobject, AbstractImageMobject)
            parts = [member for member in self.family(mobject) if len(member.points)]
            family = [member for member in parts if isinstance(member, seen)]
            if not family:
                continue
            if not all(self.shape(member)["whole"] for member in family):
                self.remark(var, node, "undefined")
                continue
            low, high = self.corners(mobject, parts)
            boxes[str(var)] = {
                "rect": [float(low[0]), float(low[1]), float(high[0]), float(high[1])],
                "on": any(id(member) in on_scene for member in family),
            }
        for var in self.named:
            made = self.world.names.get(var)
            found = getattr(made, "found", None)
            if isinstance(found, dict):
                self.remarks.append({"slide": self.slide, "node": node, "var": var, "what": "plot", **found})
            # A tangent put where its graph's rule has no value has nothing to touch.
            if getattr(made, "lost", False):
                self.remark(var, node, "lost")
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


# New objects past which a probe collects anyway: about what two edits of a long deck
# leave, some hundreds of megabytes.
_PILED_UP = 2_000_000


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
    # Python's collector looks through every object there is each time enough new ones
    # have been made, and restoring a state makes hundreds of thousands: it took half of
    # a probe. So it waits until the probe is done, and whoever runs one probe after
    # another collects in between; unless much has piled up, from probes run back to back
    # while words are typed, or in one that plays a long slide frame by frame.
    collecting = gc.isenabled()
    gc.disable()
    try:
        probe.check()
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
            world.leave()
            exec(compile(slides[index], f"slide {index + 1}", "exec"), world.space)
            # A slide that was played for real has been through other frames than one
            # that was skipped through, so only the latter is a state to start from.
            if probe.mode != "record":
                world.at = world.unsaved = keys[index + 1]
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
        if collecting:
            gc.enable()
