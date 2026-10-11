"""Check the catalogue (`suit-cli signature`) against the installed Manim.

usage: suit-cli signature | check_catalog.py

Every kind must be a Manim class with the declared parent; every constructor, method,
query and animation parameter must exist under that name; every rate function and
constant must resolve. Exits non-zero on the first run that finds a problem.
"""
import inspect
import json
import sys
import tempfile

import manim
import numpy as np
from manim.utils import rate_functions

# Making a text or a formula writes files, which are not to be left where this is run.
manim.config.media_dir = tempfile.mkdtemp(prefix="suit-catalogue-")
manim.logger.setLevel("ERROR")

problems = []
checked = 0

# These pick their own length from the object when none is given, so their signature does
# not state the default. Export always passes run_time, which makes the catalogue's value
# the one that is rendered.
OWN_RUN_TIME = {"Write", "Unwrite"}


def problem(text):
    problems.append(text)


# What the catalogue has that Manim has not is checked against its own definition, which
# is what the export writes into the files that use it.
OWN = {}
_here = __import__("pathlib").Path(__file__).resolve().parent
exec(
    "import numpy as np\nfrom manim import *\n"
    + (_here.parent / "crates/codegen/src/plot.py").read_text()
    + "\n\n"
    + (_here.parent / "crates/codegen/src/tangent.py").read_text(),
    OWN,
)


def find(name):
    return OWN.get(name) or getattr(manim, name, None) or getattr(manim.mobject.geometry.polygram, name, None)


def accepted_names(cls, method="__init__"):
    """Parameter names `cls.method` accepts, following **kwargs up the class hierarchy."""
    names, variadic = set(), False
    for klass in cls.__mro__:
        function = klass.__dict__.get(method)
        if function is None:
            continue
        parameters = inspect.signature(function).parameters.values()
        names |= {p.name for p in parameters if p.kind in (p.POSITIONAL_OR_KEYWORD, p.KEYWORD_ONLY, p.VAR_POSITIONAL)}
        variadic |= any(p.kind is p.VAR_POSITIONAL for p in parameters)
        if not any(p.kind is p.VAR_KEYWORD for p in parameters):
            break
    return names, variadic


def check_params(owner, cls, params, method="__init__"):
    global checked
    names, variadic = accepted_names(cls, method)
    for param in params:
        checked += 1
        name, how = param["name"], param["pass"]
        if how in ("keyword", "list") and name not in names:
            problem(f"{owner}: no parameter named {name!r}")
        if how == "variadic" and not variadic:
            problem(f"{owner}: {name!r} is variadic here but Manim takes no *args")
        # A positional parameter may be reached through *args (Arrow passes start and end on).
        if how == "positional" and name not in names and not variadic:
            problem(f"{owner}: no parameter named {name!r}")


sig = json.load(sys.stdin)

for name, kind in sig["kinds"].items():
    checked += 1
    cls = find(name)
    if not inspect.isclass(cls):
        problem(f"kind {name}: not a Manim class")
        continue
    if kind["parent"]:
        parent = find(kind["parent"])
        if not issubclass(cls, parent):
            problem(f"kind {name}: not a subclass of {kind['parent']}")
    check_params(f"kind {name}", cls, kind["params"])

for name, ctor in sig["ctors"].items():
    check_params(f"{name}()", find(name), ctor["params"])


def sample(ty):
    """Something of the type `ty`, to make an object with."""
    if "mob" in ty:
        return made(ty["mob"]) if ty["mob"] in sig["ctors"] else manim.Square()
    if "fun" in ty:
        return (lambda t: np.array([t, t, 0.0])) if ty["fun"][1] == "vec" else (lambda x: x)
    return {"str": "ab", "real": 1.0, "int": 1, "bool": True}[ty["base"]]


def made(name, **settings):
    """A new `name`, given what it cannot do without and `settings`."""
    given = []
    for param in sig["ctors"][name]["params"]:
        if param["pass"] == "variadic" and param["ty"] == {"base": "vec"}:
            given += [manim.UP, manim.LEFT, manim.RIGHT]
        elif param["pass"] == "variadic" or param["required"]:
            given.append(sample(param["ty"]))
    return find(name)(*given, **settings)


def look(mobject):
    """What can be seen of the look of an object: of each part, the fill if it is not
    fully transparent, and the stroke if there is one."""
    seen = []
    for part in mobject.get_family():
        if isinstance(part, manim.VMobject) and part.has_points():
            filled = part.get_fill_opacity() > 0
            stroked = part.get_stroke_width() > 0 and part.get_stroke_opacity() > 0
            seen.append(
                (
                    part.get_fill_color().to_hex() if filled else None,
                    round(float(part.get_fill_opacity()), 3),
                    part.get_stroke_color().to_hex() if stroked else None,
                    round(float(part.get_stroke_width()), 3) if stroked else None,
                    round(float(part.get_stroke_opacity()), 3) if stroked else None,
                )
            )
    return seen


def inherited(name, what):
    """What the kind `name` says of `what`, or else the nearest kind above it that does."""
    while name is not None:
        if sig["kinds"][name][what] is not None:
            return sig["kinds"][name][what]
        name = sig["kinds"][name]["parent"]
    return None


def below(name, other):
    while name is not None and name != other:
        name = sig["kinds"][name]["parent"]
    return name is not None


def unseen(ctor):
    """The settings of a constructor that reach none of its parts: of what is made of
    parts, as a group is, those it inherits (`unseen_params` in crates/sig/src/types.rs)."""
    if not below(ctor["kind"], "VGroup"):
        return []
    own = {param["name"] for param in ctor["params"]}
    found, kind = [], ctor["kind"]
    while kind is not None:
        found += [param for param in sig["kinds"][kind]["params"] if param["name"] not in own]
        kind = sig["kinds"][kind]["parent"]
    return found


# The call that does to a finished object what a setting says of a new one: its method,
# and the parameter that takes the setting's value (`call_for`, in the same file).
CALL_FOR = {
    "color": ("set_color", "color"),
    "stroke_color": ("set_stroke", "color"),
    "stroke_width": ("set_stroke", "width"),
    "stroke_opacity": ("set_stroke", "opacity"),
}


def other(param):
    """A value for a setting of the look that is not the one any object starts with."""
    return {"color": manim.ORANGE, "real": 0.5}[param["ty"]["base"]]


# What a fill colour does to a new object is what the catalogue says of its inside: the
# editor offers settings by it, and the checker warns by it.
for name, ctor in sig["ctors"].items():
    inside = inherited(ctor["kind"], "inside")
    if inside is None:
        continue
    plain = look(made(name))
    checked += 1
    if (look(made(name, fill_color=manim.ORANGE)) != plain) != (inside == "filled"):
        shows = "nothing" if inside == "filled" else "something"
        problem(f"{name}: its inside is {inside}, but a fill colour alone shows {shows}")
    if inside == "clear":
        checked += 1
        if look(made(name, fill_color=manim.ORANGE, fill_opacity=1.0)) == plain:
            problem(f"{name}: its inside is clear, but a fill colour with an opacity shows nothing")
    # `color` is the one that shows on everything, and what the checker says to use where
    # there is no inside. Except on what is made of parts.
    parts = unseen(ctor)
    if not parts:
        checked += 1
        if look(made(name, color=manim.ORANGE)) == plain:
            problem(f"{name}: `color` shows nothing")
    # There, nothing that is given for a look shows, alone or with the rest: the editor
    # does not offer it, and the checker warns of it. The calls offered instead do show.
    for param in parts:
        setting, value = param["name"], other(param)
        checked += 1
        if look(made(name, **{setting: value})) != plain:
            problem(f"{name}: is made of parts, but `{setting}` shows something")
        if setting not in CALL_FOR:
            continue
        method, takes = CALL_FOR[setting]
        checked += 1
        offered = [
            overload
            for overload in sig["methods"].get(method, [])
            if below(ctor["kind"], overload["receiver"])
            and any(taken["name"] == takes for taken in overload["params"])
        ]
        if not offered:
            problem(f"{name}: the catalogue has no {method}({takes}) to offer for `{setting}`")
            continue
        later = made(name)
        getattr(later, method)(**{takes: value})
        if look(later) == plain:
            problem(f"{name}: {method}({takes}) shows nothing")
    if parts:
        checked += 1
        if look(made(name, **{param["name"]: other(param) for param in parts})) != plain:
            problem(f"{name}: is made of parts, but its look settings together show something")

for name, overloads in sig["methods"].items():
    for method in overloads:
        cls = find(method["receiver"])
        if not callable(getattr(cls, name, None)):
            problem(f"{method['receiver']}.{name}: no such method")
            continue
        check_params(f"{method['receiver']}.{name}", cls, method["params"], method=name)

for name, overloads in sig["queries"].items():
    for query in overloads:
        checked += 1
        cls = find(query["receiver"])
        member = inspect.getattr_static(cls, name, None)
        if member is None:
            problem(f"{query['receiver']}.{name}: no such member")
        elif isinstance(member, property) != query["property"]:
            problem(f"{query['receiver']}.{name}: property flag is wrong")

for name, anim in sig["anims"].items():
    cls = find(name)
    if not (inspect.isclass(cls) and issubclass(cls, manim.Animation)):
        problem(f"animation {name}: not a Manim animation")
        continue
    check_params(f"animation {name}", cls, anim["params"])
    # Defaults the catalogue states must be the ones Manim uses.
    parameters = {}
    for klass in reversed(cls.__mro__):
        if "__init__" in klass.__dict__:
            parameters.update(inspect.signature(klass.__dict__["__init__"]).parameters)
    checked += 1
    default = parameters["run_time"].default if "run_time" in parameters else None
    if isinstance(default, (int, float)) and default != anim["run_time"] and name not in OWN_RUN_TIME:
        problem(f"animation {name}: default run_time is {default}, catalogue says {anim['run_time']}")
    default = parameters["rate_func"].default if "rate_func" in parameters else None
    if callable(default) and default.__name__ != anim["rate"]:
        problem(f"animation {name}: default rate is {default.__name__}, catalogue says {anim['rate']}")
    default = parameters["lag_ratio"].default if "lag_ratio" in parameters else None
    if isinstance(default, (int, float)) and default != anim["lag"]:
        problem(f"animation {name}: default lag_ratio is {default}, catalogue says {anim['lag']}")

scope = {**vars(manim), "rate_functions": rate_functions}
for name, spelling in sig["rates"].items():
    checked += 1
    try:
        if not callable(eval(spelling, scope)):
            problem(f"rate {name}: {spelling} is not callable")
    except Exception as error:  # noqa: BLE001 - any failure to resolve is the finding
        problem(f"rate {name}: {spelling} does not resolve ({error})")

for name, constant in sig["consts"].items():
    checked += 1
    actual = getattr(manim, name, None)
    (tag, value), = constant.items()
    if actual is None:
        problem(f"constant {name}: not exported by Manim")
    elif tag == "real" and not np.isclose(actual, value):
        problem(f"constant {name}: Manim has {actual}, catalogue {value}")
    elif tag == "vec" and not np.allclose(actual, value):
        problem(f"constant {name}: Manim has {actual}, catalogue {value}")
    elif tag == "color" and manim.ManimColor(actual).to_hex().lower() != value.lower():
        problem(f"constant {name}: Manim has {manim.ManimColor(actual).to_hex()}, catalogue {value}")

print(f"manim {manim.__version__}: {checked} catalogue facts checked, {len(problems)} problem(s)")
for text in problems:
    print("  -", text)
sys.exit(1 if problems else 0)
