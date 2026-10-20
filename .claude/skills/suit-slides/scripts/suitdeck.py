#!/usr/bin/env python3
"""suitdeck: write Suit decks (.suit.json) from a short Python script.

    import sys; sys.path.insert(0, "<this folder>")
    from suitdeck import *

    deck = Deck()
    s = deck.slide("The derivative", notes="What the slope of a curve is.")
    title = s.new("title", "Text", "The derivative", font_size=60).then("to_edge", UP)
    s.play(Write(title))
    deck.save("derivative.suit.json")        # writes the file, then checks it with suit-cli

Every object, setting, animation, method and constant is the one Suit's catalogue names
(`python3 suitdeck.py catalogue` lists them). Settings are checked against the catalogue
while the deck is built, so a mistake is reported at the line that makes it, and values
are written with the type the setting wants: `radius=1` becomes the real number 1.0.

As a program:

    python3 suitdeck.py catalogue [NAME ...]    what Suit knows, or all about some of it
    python3 suitdeck.py check DECK.suit.json     suit-cli's checker and timeline
    python3 suitdeck.py where                    which suit-cli and which Python are used

Needs only Python 3.9 or newer. The checking needs `suit-cli` (installed with Suit, or
built in a checkout of Suit's source); without it the deck is still written, unchecked.
"""

from __future__ import annotations

import inspect
import json
import math
import os
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
SNAPSHOT = HERE.parent / "assets" / "signature.json"


class SuitError(Exception):
    """A deck that cannot be what was asked for, said in words."""


# ── Finding Suit ─────────────────────────────────────────────────────────────


def _source_checkouts():
    """Folders that may be a checkout of Suit's source: named in SUIT_REPO, above this
    script (the skill may live in one), the one a copy of the skill was made from (named
    in assets/checkout.txt), and above the working directory."""
    seen = []
    named = os.environ.get("SUIT_REPO")
    starts = [Path(named)] if named else []
    starts.append(HERE)
    recorded = HERE.parent / "assets" / "checkout.txt"
    if recorded.is_file():
        starts.append(Path(recorded.read_text().strip()))
    starts.append(Path.cwd())
    for start in starts:
        for folder in [start, *start.parents]:
            if (folder / "Cargo.toml").is_file() and (folder / "crates").is_dir():
                if folder not in seen:
                    seen.append(folder)
    return seen


def find_cli():
    """The `suit-cli` to check and export with, or None."""
    named = os.environ.get("SUIT_CLI")
    if named:
        return named
    exe = "suit-cli.exe" if os.name == "nt" else "suit-cli"
    for root in _source_checkouts():
        for build in ("release", "debug"):
            candidate = root / "target" / build / exe
            if candidate.is_file():
                return str(candidate)
    return shutil.which("suit-cli")


def find_python():
    """A Python that has Manim and manim-slides, the way Suit itself looks for one."""
    candidates = []
    if os.environ.get("SUIT_PYTHON"):
        candidates.append(Path(os.environ["SUIT_PYTHON"]))
    inside = Path("Scripts/python.exe") if os.name == "nt" else Path("bin/python")
    for root in _source_checkouts():
        candidates.append(root / ".venv" / inside)
    if os.name == "nt" and os.environ.get("LOCALAPPDATA"):
        candidates.append(Path(os.environ["LOCALAPPDATA"]) / "Suit" / "venv" / inside)
    else:
        shared = os.environ.get("XDG_DATA_HOME") or str(Path.home() / ".local" / "share")
        candidates.append(Path(shared) / "suit" / "venv" / inside)
    # A Python environment only works on the system it was made on: one made in a
    # container is there on the host too, and finds none of its packages. So each is asked.
    for python in candidates:
        if python.is_file() and _has_manim(python):
            return str(python)
    return None


def _has_manim(python):
    try:
        asked = subprocess.run([str(python), "-c", "import manim, manim_slides"],
                               capture_output=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return False
    return asked.returncode == 0


# ── The catalogue ────────────────────────────────────────────────────────────

_CATALOGUE = None


def catalogue():
    """Suit's catalogue: from `suit-cli signature` when there is one, else the snapshot
    that comes with this skill."""
    global _CATALOGUE
    if _CATALOGUE is None:
        cli = find_cli()
        if cli:
            try:
                out = subprocess.run([cli, "signature"], capture_output=True, text=True, check=True)
                _CATALOGUE = json.loads(out.stdout)
            except (OSError, subprocess.CalledProcessError, ValueError):
                _CATALOGUE = None
        if _CATALOGUE is None:
            _CATALOGUE = json.loads(SNAPSHOT.read_text())
    return _CATALOGUE


def _ancestors(kind):
    kinds = catalogue()["kinds"]
    while kind is not None and kind in kinds:
        yield kind
        kind = kinds[kind]["parent"]


def is_subkind(sub, sup):
    return any(kind == sup for kind in _ancestors(sub))


def ctor_params(ctor):
    """A constructor's own settings, then those it shares with the kinds above it."""
    sig = catalogue()
    made = sig["ctors"].get(ctor)
    if made is None:
        raise SuitError(f"There is no object called {ctor!r}. {_suggest(ctor, sig['ctors'])}")
    params = list(made["params"])
    for kind in _ancestors(made["kind"]):
        for param in sig["kinds"][kind]["params"]:
            if all(own["name"] != param["name"] for own in params):
                params.append(param)
    return params


def _method(kind, name):
    for overload in catalogue()["methods"].get(name, []):
        if is_subkind(kind, overload["receiver"]):
            return overload
    known = [m for m, overloads in catalogue()["methods"].items()
             if any(is_subkind(kind, o["receiver"]) for o in overloads)]
    if name in catalogue()["methods"]:
        raise SuitError(f"A {kind} cannot do {name!r}. It can: {', '.join(sorted(known))}.")
    raise SuitError(f"There is no action called {name!r}. {_suggest(name, known)}")


def _query(kind, name):
    for overload in catalogue()["queries"].get(name, []):
        if is_subkind(kind, overload["receiver"]):
            return overload
    raise SuitError(f"A {kind} cannot be asked {name!r}.")


def _suggest(name, known):
    import difflib
    known = list(known)
    lower = name.lower()
    close = [k for k in known if lower in k.lower() or k.lower() in lower]
    close += [k for k in difflib.get_close_matches(name, known, n=4, cutoff=0.6) if k not in close]
    return f"Did you mean: {', '.join(close[:6])}?" if close else ""


# ── Expressions ──────────────────────────────────────────────────────────────


class Expr:
    """A value, possibly computed: a number, a point, a colour, or a formula of them.
    Arithmetic on it builds formulas, which Manim works out when the step is done."""

    def __init__(self, json_value):
        self.json = json_value

    def _op(self, fun, other, flipped=False):
        a, b = expr(self), expr(other)
        return Expr({"call": {"fun": fun, "args": [b, a] if flipped else [a, b]}})

    def __add__(self, other): return self._op("+", other)
    def __radd__(self, other): return self._op("+", other, True)
    def __sub__(self, other): return self._op("-", other)
    def __rsub__(self, other): return self._op("-", other, True)
    def __mul__(self, other): return self._op("*", other)
    def __rmul__(self, other): return self._op("*", other, True)
    def __truediv__(self, other): return self._op("/", other)
    def __rtruediv__(self, other): return self._op("/", other, True)
    def __pow__(self, other): return self._op("pow", other)
    def __rpow__(self, other): return self._op("pow", other, True)
    def __neg__(self): return Expr({"call": {"fun": "neg", "args": [expr(self)]}})
    def __lt__(self, other): return self._op("<", other)
    def __le__(self, other): return self._op("<=", other)
    def __gt__(self, other): return self._op(">", other)
    def __ge__(self, other): return self._op(">=", other)

    @property
    def x(self): return Expr({"call": {"fun": "x", "args": [expr(self)]}})

    @property
    def y(self): return Expr({"call": {"fun": "y", "args": [expr(self)]}})

    def __repr__(self):
        return f"Expr({json.dumps(self.json)})"


def _call(fun, *args):
    return Expr({"call": {"fun": fun, "args": [expr(a) for a in args]}})


def sin(x): return _call("sin", x)
def cos(x): return _call("cos", x)
def tan(x): return _call("tan", x)
def exp(x): return _call("exp", x)
def log(x): return _call("log", x)
def sqrt(x): return _call("sqrt", x)
def absolute(x): return _call("abs", x)
def smaller(a, b): return _call("min", a, b)
def larger(a, b): return _call("max", a, b)
def norm(v): return _call("norm", v)
def equal(a, b): return _call("==", a, b)
def both(a, b): return _call("and", a, b)
def either(a, b): return _call("or", a, b)
def negation(a): return _call("not", a)


def choose(condition, then, otherwise):
    """`if condition then … else …`"""
    return Expr({"if": {"cond": expr(condition), "then": expr(then), "otherwise": expr(otherwise)}})


def point(x, y, z=0.0):
    """A point or direction: (x, y) in scene units. Plain tuples work as well."""
    return Expr({"vec3": [expr(x, "real"), expr(y, "real"), expr(z, "real")]})


def colour(hex_text):
    """A colour written #rrggbb. Catalogue names like BLUE are constants already."""
    if not (isinstance(hex_text, str) and hex_text.startswith("#") and len(hex_text) in (7, 9)):
        raise SuitError(f"A colour is written #rrggbb, not {hex_text!r}.")
    return Expr({"color": hex_text.lower()})


def expr(value, want=None):
    """The JSON of `value` as a formula. `want` is the base type the place wants
    ("real", "int", "vec", "color", "str", "bool"), where it is known."""
    if isinstance(value, Expr):
        return value.json
    if isinstance(value, Obj):
        return {"place": value._place()}
    if isinstance(value, bool):
        return {"bool": value}
    if isinstance(value, int):
        return {"real": float(value)} if want == "real" else {"int": value}
    if isinstance(value, float):
        if want == "int":
            if not value.is_integer():
                raise SuitError(f"A whole number is wanted here, not {value}.")
            return {"int": int(value)}
        if not math.isfinite(value):
            raise SuitError(f"{value} is not a number Manim can use.")
        return {"real": value}
    if isinstance(value, (tuple, list)):
        if len(value) not in (2, 3):
            raise SuitError(f"A point has two or three numbers, not {value!r}.")
        parts = list(value) + [0.0] * (3 - len(value))
        return {"vec3": [expr(part, "real") for part in parts]}
    if isinstance(value, str):
        consts = catalogue()["consts"]
        if want == "color":
            if value.startswith("#"):
                return colour(value).json
            if value in consts and "color" in consts[value]:
                return {"const": value}
            names = [n for n, c in consts.items() if "color" in c]
            raise SuitError(f"There is no colour called {value!r}. {_suggest(value, names)}")
        if want == "vec" and value in consts and "vec" in consts[value]:
            return {"const": value}
        return {"str": value}
    raise SuitError(f"{value!r} cannot be put in a deck.")


def _base(ty):
    """The base type a parameter type wants, for writing plain Python values."""
    if "base" in ty:
        return ty["base"]
    if "value" in ty:
        return _base(ty["value"])
    if "union" in ty:
        bases = [_base(option) for option in ty["union"]]
        return next((b for b in bases if b), None)
    return None


# ── Objects ──────────────────────────────────────────────────────────────────

_QUERIES = {
    "center": "get_center", "top": "get_top", "bottom": "get_bottom", "left": "get_left",
    "right": "get_right", "start": "get_start", "end": "get_end", "x": "get_x", "y": "get_y",
    "width": "width", "height": "height", "value": "get_value", "slope": "slope",
}


class Obj:
    """An object on a slide, or a part of one (`row[0]`). Asking it something, like
    `circle.center` or `tracker.value`, gives a formula."""

    def __init__(self, deck, var, kind, path=(), made=None, slide=None):
        self._deck = deck
        self._var = var
        self._kind = kind
        self._path = tuple(path)
        self._made = made  # the step that makes it, while `then` may still extend it
        self._slide = slide  # the slide it is made on, and how many steps it had then
        self._steps_then = len(slide._body) if slide is not None else 0

    def _place(self):
        place = {"root": self._var}
        if self._path:
            place["path"] = list(self._path)
        return place

    def __getitem__(self, index):
        elem = catalogue()["kinds"].get(self._kind, {}).get("elem")
        kind = elem or self._kind
        if elem is None:
            for k in _ancestors(self._kind):
                if catalogue()["kinds"][k]["elem"]:
                    kind = catalogue()["kinds"][k]["elem"]
                    break
            else:
                kind = "Mobject"
        return Obj(self._deck, self._var, kind, self._path + (int(index),))

    def __getattr__(self, name):
        if name.startswith("_"):
            raise AttributeError(name)
        if name not in _QUERIES:
            raise AttributeError(
                f"{name!r}: an object can be asked {', '.join(sorted(_QUERIES))}, "
                "or corner(direction), point_from_proportion(t), c2p(x, y), n2p(x)"
            )
        return self._ask(_QUERIES[name])

    def _ask(self, query, *args):
        found = _query(self._kind, query)
        bound = _bind(found["params"], args, {}, f"{self._kind}.{query}")
        return Expr({"query": {"target": self._place(), "query": query, "args": bound}})

    def corner(self, direction): return self._ask("get_corner", direction)
    def point_from_proportion(self, t): return self._ask("point_from_proportion", t)
    def c2p(self, x, y): return self._ask("c2p", x, y)
    def n2p(self, number): return self._ask("n2p", number)

    def then(self, method, *args, **settings):
        """Does `method` to the object as soon as it is made: place it, size it, turn it.
        This is **Then also…** in Suit."""
        if self._made is None:
            raise SuitError("`then` works on what `new`, `group` or `copy` just made.")
        # `then` belongs to the step that makes the object, and so sees the slide as it
        # was then. Steps added since would not have happened yet, which surprises.
        if self._slide is not None and len(self._slide._body) != self._steps_then:
            raise SuitError(
                f"`then` places {self!r} where it is made, which is before the steps added "
                "since. Call it straight after `new`, or change it now with "
                f"`s.do(obj, {method!r}, ...)` or an animation."
            )
        call = _call_json(self._kind, method, args, settings)
        node = self._made["let_object"]["value"]
        self._made["let_object"]["value"] = self._deck._node({"chain": {"base": node, "call": call}})
        return self

    def __repr__(self):
        name = self._deck._names.get(self._var, "?")
        return f"<{self._kind} {name}{''.join(f'[{i}]' for i in self._path)}>"


def _call_json(kind, method, args, settings):
    found = _method(kind, method)
    bound = _bind(found["params"], args, settings, method)
    call = {"method": method}
    if bound:
        call["args"] = bound
    return call


# ── Binding settings to parameters ───────────────────────────────────────────


def _bind(params, args, settings, owner):
    """`args` (by position) and `settings` (by name) as the list of named arguments Suit
    keeps, each written as its parameter's type wants."""
    bound = []
    positional = [p for p in params if p["pass"] in ("positional", "variadic")]
    args = list(args)
    for param in positional:
        if not args:
            break
        if param["pass"] == "variadic":
            bound += [[param["name"], _arg(value, param["ty"], owner, param["name"])] for value in args]
            args = []
        else:
            bound.append([param["name"], _arg(args.pop(0), param["ty"], owner, param["name"])])
    if args:
        raise SuitError(f"{owner} takes {len(positional)} setting(s) by position, given more: {args!r}")
    by_name = {p["name"]: p for p in params}
    for name, value in settings.items():
        param = by_name.get(name)
        if param is None:
            raise SuitError(
                f"{owner} has no setting called {name!r}. It has: {', '.join(by_name)}."
            )
        if any(given == name for given, _ in bound):
            raise SuitError(f"{owner} is given {name!r} twice.")
        many = param["pass"] in ("variadic", "list") and isinstance(value, list)
        for one in value if many else [value]:
            bound.append([name, _arg(one, param["ty"], owner, name)])
    for param in params:
        if param["required"] and not any(given == param["name"] for given, _ in bound):
            raise SuitError(f"{owner} needs {param['name']!r}.")
    return bound


def _arg(value, ty, owner, name):
    """One argument: an object, a rule, an easing, or a formula."""
    if "value" in ty:
        ty = ty["value"]
    if "object" in ty or "mob" in ty:
        kind = ty["object"]["kind"] if "object" in ty else ty["mob"]
        if not isinstance(value, Obj):
            raise SuitError(f"{owner} wants an object for {name!r}, not {value!r}.")
        if not is_subkind(value._kind, kind):
            raise SuitError(f"{owner} wants a {kind} for {name!r}; {value!r} is a {value._kind}.")
        return {"expr": {"place": value._place()}}
    if "fun" in ty:
        if not callable(value):
            raise SuitError(f"{owner} wants a rule for {name!r}, like `lambda x: x ** 2`.")
        return _rule(value, ty["fun"][1])
    if ty == "rate":
        return {"rate": _rate(value)}
    if "union" in ty and isinstance(value, Obj):
        return {"expr": {"place": value._place()}}
    return {"expr": expr(value, _base(ty))}


_DECK_BUILDING = []  # the deck whose variables a rule's parameter is made in


def _rule(function, result):
    deck = _DECK_BUILDING[-1]
    names = list(inspect.signature(function).parameters)
    var = deck._var(names[0] if names else "x")
    body = function(Expr({"var": var}))
    if isinstance(body, (tuple, list)):
        body = point(*body)
    return {"fun": {"param": var, "body": expr(body, result)}}


def _rate(name):
    if name not in catalogue()["rates"]:
        raise SuitError(f"There is no easing called {name!r}. {_suggest(name, catalogue()['rates'])}")
    return name


# ── Animations ───────────────────────────────────────────────────────────────


class Anim:
    """One animation, or several played together or in turn."""

    def __init__(self, item, run_time=None, rate=None, lag=None):
        self.item = item
        opts = {}
        if run_time is not None:
            opts["run_time"] = float(run_time)
        if rate is not None:
            opts["rate"] = _rate(rate)
        if lag is not None:
            opts["lag"] = float(lag)
        kind = next(iter(item))
        item[kind]["opts"] = opts

    def _json(self, deck):
        kind = next(iter(self.item))
        body = self.item[kind]
        if kind in ("par", "seq"):
            body = dict(body, items=[deck._node(a._json(deck)) for a in body["items"]])
        return {kind: body}


def animation(name, *args, run_time=None, rate=None, lag=None, **settings):
    """Any animation of the catalogue by name: `animation("FadeIn", dot, shift=UP)`."""
    found = catalogue()["anims"].get(name)
    if found is None:
        raise SuitError(f"There is no animation called {name!r}. {_suggest(name, catalogue()['anims'])}")
    bound = _bind(found["params"], args, settings, name)
    return Anim({"prim": {"ctor": name, "args": bound}}, run_time, rate, lag)


def animate(obj, method, *args, run_time=None, rate=None, also=(), **settings):
    """A change of `obj` made smoothly: `animate(circle, "shift", 2 * RIGHT)`. `also` adds
    more changes made in the same movement: `also=[("scale", 1.5)]`."""
    calls = [_call_json(obj._kind, method, args, settings)]
    for extra in also:
        extra_method, *extra_args = extra
        calls.append(_call_json(obj._kind, extra_method, extra_args, {}))
    return Anim({"animate": {"target": obj._place(), "calls": calls}}, run_time, rate)


def together(*anims, lag=None, run_time=None, rate=None):
    """Several animations as one, starting together; with `lag`, one after another by
    that fraction (0 all at once, 1 strictly in turn)."""
    return Anim({"par": {"items": list(anims)}}, run_time, rate, lag)


def in_turn(*anims, run_time=None, rate=None):
    """Several animations, each starting when the one before has finished."""
    return Anim({"seq": {"items": list(anims)}}, run_time, rate)


# ── Slides and decks ─────────────────────────────────────────────────────────


class Slide:
    def __init__(self, deck, data):
        self._deck = deck
        self._data = data

    @property
    def _body(self):
        return self._data["content"]["blueprint"]["body"]

    def _step(self, command):
        node = self._deck._node(command)
        self._body.append(node)
        return node["item"]

    def _let(self, name, value_json, kind):
        var = self._deck._var(name)
        command = self._step({"let_object": {"var": var, "value": self._deck._node(value_json)}})
        return Obj(self._deck, var, kind, made=command, slide=self)

    def new(self, name, ctor, *args, **settings):
        """Makes an object called `name`. Nothing is seen until a step brings it in
        (an animation such as Write, Create or FadeIn, or `show`)."""
        _DECK_BUILDING.append(self._deck)
        try:
            bound = _bind(ctor_params(ctor), args, settings, ctor)
        finally:
            _DECK_BUILDING.pop()
        kind = catalogue()["ctors"][ctor]["kind"]
        return self._let(name, {"new": {"ctor": ctor, "args": bound}}, kind)

    def group(self, name, *objs):
        """Puts objects together, to move and animate them as one. The members are then
        used through the group (`row[0]`)."""
        if len(objs) < 2:
            raise SuitError("A group is made of two objects or more.")
        vector = all(is_subkind(o._kind, "VMobject") for o in objs)
        items = [o._place() for o in objs]
        return self._let(name, {"group": {"vector": vector, "items": items}}, "VGroup" if vector else "Group")

    def copy(self, name, obj):
        """A copy of `obj`, an object of its own from then on."""
        return self._let(name, {"copy": obj._place()}, obj._kind)

    def value(self, name, value):
        """A named value for formulas to use. It does not change during the slide."""
        var = self._deck._var(name)
        self._step({"let_value": {"var": var, "value": expr(value)}})
        return Expr({"var": var})

    def play(self, *anims, lag=None, run_time=None, rate=None):
        """One step of animation. Several animations start together; the step lasts as
        long as the longest. With `lag` they cascade. `run_time` and `rate` (an easing)
        apply to the whole step."""
        if not anims:
            raise SuitError("play needs at least one animation.")
        whole = run_time is not None or rate is not None
        if lag is not None or (whole and len(anims) > 1):
            anims = (together(*anims, lag=lag, run_time=run_time, rate=rate),)
        elif whole:
            opts = anims[0].item[next(iter(anims[0].item))]["opts"]
            if run_time is not None:
                opts["run_time"] = float(run_time)
            if rate is not None:
                opts["rate"] = _rate(rate)
        self._step({"play": {"anims": [self._deck._node(a._json(self._deck)) for a in anims]}})
        return self

    def do(self, obj, method, *args, **settings):
        """An instant change, made between two frames (**Change** in Suit)."""
        self._step({"apply": {"target": obj._place(), "call": _call_json(obj._kind, method, args, settings)}})
        return self

    def show(self, *objs):
        """Puts objects on the slide without an animation."""
        self._step({"add": {"targets": [o._place() for o in objs]}})
        return self

    def hide(self, *objs):
        """Takes objects off the slide without an animation."""
        self._step({"remove": {"targets": [o._place() for o in objs]}})
        return self

    def wait(self, seconds=1.0):
        """Holds still."""
        self._step({"wait": {"duration": float(seconds)}})
        return self

    def keep(self, obj, method, *args, **settings):
        """From this step on, `obj` does `method` again on every frame (**Keep doing…**):
        `s.keep(label, "next_to", dot, UP)`."""
        call = _call_json(obj._kind, method, args, settings)
        self._step({"attach": {"target": obj._place(), "updater": {"always": call}}})
        return self

    def keep_every(self, obj, change):
        """Like `keep`, given the time since the last frame: `change(dt)` returns the
        method and its settings, e.g. `lambda dt: ("rotate", dt * PI)`."""
        dt = self._deck._var("dt")
        method, *args = change(Expr({"var": dt}))
        call = _call_json(obj._kind, method, args, {})
        self._step({"attach": {"target": obj._place(), "updater": {"every": {"dt": dt, "call": call}}}})
        return self

    def stop(self, obj):
        """From this step on, `obj` no longer does what `keep` gave it."""
        self._step({"detach": {"target": obj._place()}})
        return self

    def python(self, code, takes=0.0):
        """A step of Python code, run as it stands in the scene's construct (`self` is the
        scene). Say how long it plays in `takes`. Objects it makes are unknown to later steps."""
        iface = {"requires": [], "ensures": [], "defines": [], "duration": float(takes)}
        self._step({"raw": {"python": code, "iface": iface}})
        return self


class Deck:
    """A whole deck. 1920×1080 at 60 frames a second on black, unless said otherwise."""

    def __init__(self, width=1920, height=1080, fps=60, background="#000000", gap=0.1):
        self._next_id = 0
        self._names = {}
        self._slides = []
        self.config = {
            "width": int(width), "height": int(height), "fps": int(fps),
            "background": colour(background).json["color"], "gap": float(gap),
            "gap_on_loops": False,
        }

    def _fresh(self):
        self._next_id += 1
        return self._next_id

    def _var(self, name):
        var = self._fresh()
        self._names[var] = name
        return var

    def _node(self, item):
        return {"id": self._fresh(), "item": item}

    def slide(self, title, notes="", loop=False, auto_next=False):
        """A new slide at the end. It starts with everything the slide before it left on
        the scene: take away what should not stay."""
        data = {
            "id": self._fresh(),
            "title": title,
            "config": {
                "looping": bool(loop), "auto_next": bool(auto_next), "skip": False,
                "playback_rate": 1.0, "reversed_playback_rate": 1.0, "notes": notes,
                "direction": "horizontal",
            },
            "content": {"blueprint": {"body": []}},
        }
        self._slides.append(data)
        return Slide(self, data)

    def json(self):
        return {
            "config": self.config,
            "slides": self._slides,
            "names": {str(var): name for var, name in sorted(self._names.items())},
            "next_id": self._next_id,
        }

    def save(self, path, check=True):
        """Writes the deck, then has suit-cli check it and list its slides. Fails (exit 1)
        when the checker finds errors, so that a broken deck is never handed over."""
        path = Path(path)
        if not path.name.endswith(".suit.json"):
            raise SuitError(f"A deck's file name ends in .suit.json: {path.name}")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(self.json(), indent=2, ensure_ascii=False) + "\n")
        print(f"wrote {path}")
        if check:
            if not check_deck(path):
                sys.exit(1)
        return path


def check_deck(path, timeline=True):
    """suit-cli's verdict on a deck file. True when there is no error."""
    cli = find_cli()
    if cli is None:
        print("suit-cli was not found, so the deck is unchecked. Set SUIT_CLI, or install Suit.")
        return True
    verdict = subprocess.run([cli, "check", str(path)], capture_output=True, text=True)
    print((verdict.stdout + verdict.stderr).strip())
    if verdict.returncode == 0 and timeline:
        times = subprocess.run([cli, "timeline", str(path)], capture_output=True, text=True)
        print(times.stdout.strip())
    return verdict.returncode == 0


# ── Constants and animations by name ─────────────────────────────────────────


def _publish():
    """Every constant (UP, PI, BLUE, …) and every animation (Write, FadeIn, …) of the
    catalogue as a name of this module."""
    names = {}
    for name in catalogue()["consts"]:
        names[name] = Expr({"const": name})
    for name in catalogue()["anims"]:
        def make(*args, _name=name, **settings):
            return animation(_name, *args, **settings)
        make.__name__ = name
        make.__doc__ = f"The {name} animation: see `suitdeck.py catalogue {name}`."
        names[name] = make
    globals().update(names)
    return list(names)


__all__ = [
    "Deck", "Slide", "Obj", "Expr", "Anim", "SuitError", "animation", "animate", "together",
    "in_turn", "point", "colour", "choose", "sin", "cos", "tan", "exp", "log", "sqrt",
    "absolute", "smaller", "larger", "norm", "equal", "both", "either", "negation",
    "check_deck", "catalogue", "find_cli", "find_python",
] + _publish()


# ── As a program ─────────────────────────────────────────────────────────────


def _describe_type(ty):
    if "value" in ty:
        return _describe_type(ty["value"])
    if "object" in ty:
        return f"object ({ty['object']['role']} {ty['object']['kind']})"
    if "mob" in ty:
        return f"object {ty['mob']}"
    if "fun" in ty:
        return f"rule {ty['fun'][0]} -> {ty['fun'][1]}"
    if "union" in ty:
        return " or ".join(_describe_type(t) for t in ty["union"])
    if ty == "rate":
        return "easing"
    return ty.get("base", str(ty))


def _describe_params(params):
    parts = []
    for p in params:
        star = "*" if p["pass"] in ("variadic", "list") else ""
        need = "" if not p["required"] else " (needed)"
        how = "" if p["pass"] in ("keyword", "list") else ", by position"
        parts.append(f"    {star}{p['name']}: {_describe_type(p['ty'])}{need}{how}")
    return "\n".join(parts) or "    (no settings of its own)"


def _print_catalogue(names):
    sig = catalogue()
    if not names:
        own = [n for n in sig["ctors"]]
        print("OBJECTS  (s.new(name, OBJECT, ...); details: catalogue NAME)")
        print("  " + ", ".join(own))
        print("ANIMATIONS  (play(Write(obj)), ...)")
        print("  " + ", ".join(sig["anims"]))
        print("ACTIONS  (s.do / animate / keep / .then)")
        print("  " + ", ".join(sig["methods"]))
        print("QUESTIONS  (obj.center, obj.value, ...)")
        print("  " + ", ".join(sig["queries"]))
        print("FUNCTIONS  sin cos tan exp log sqrt absolute smaller larger norm choose; + - * / ** < > on formulas")
        print("EASINGS  " + ", ".join(sig["rates"]))
        print("CONSTANTS  " + ", ".join(n for n, c in sig["consts"].items() if "color" not in c))
        print("COLOURS  " + ", ".join(n for n, c in sig["consts"].items() if "color" in c))
        return
    for name in names:
        if name in sig["ctors"]:
            kind = sig["ctors"][name]["kind"]
            chain = " < ".join(_ancestors(kind))
            print(f"{name}  ({chain})\n{_describe_params(ctor_params(name))}")
        if name in sig["anims"]:
            a = sig["anims"][name]
            print(f"animation {name}  usual length {a['run_time']} s, easing {a['rate']}\n{_describe_params(a['params'])}")
        if name in sig["methods"]:
            for m in sig["methods"][name]:
                print(f"action {name} on {m['receiver']}\n{_describe_params(m['params'])}")
        if name in sig["queries"]:
            for q in sig["queries"][name]:
                print(f"question {name} of {q['receiver']} -> {q['result']}\n{_describe_params(q['params'])}")
        if name not in sig["ctors"] and name not in sig["anims"] and name not in sig["methods"] and name not in sig["queries"]:
            everything = [*sig["ctors"], *sig["anims"], *sig["methods"], *sig["queries"]]
            print(f"{name}: not in the catalogue. {_suggest(name, everything)}")


def main(argv):
    if len(argv) >= 1 and argv[0] == "catalogue":
        _print_catalogue(argv[1:])
        return 0
    if len(argv) == 2 and argv[0] == "check":
        return 0 if check_deck(argv[1]) else 1
    if len(argv) == 1 and argv[0] == "where":
        print("suit-cli:", find_cli() or "not found")
        print("python with Manim:", find_python() or "not found")
        print("catalogue:", "from suit-cli" if find_cli() else f"snapshot {SNAPSHOT}")
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
