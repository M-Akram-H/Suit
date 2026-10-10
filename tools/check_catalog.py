"""Check the catalogue (`suit signature`) against the installed Manim.

usage: suit signature | check_catalog.py

Every kind must be a Manim class with the declared parent; every constructor, method,
query and animation parameter must exist under that name; every rate function and
constant must resolve. Exits non-zero on the first run that finds a problem.
"""
import inspect
import json
import sys

import manim
import numpy as np
from manim.utils import rate_functions

problems = []
checked = 0

# These pick their own length from the object when none is given, so their signature does
# not state the default. Export always passes run_time, which makes the catalogue's value
# the one that is rendered.
OWN_RUN_TIME = {"Write", "Unwrite"}


def problem(text):
    problems.append(text)


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
        if how == "keyword" and name not in names:
            problem(f"{owner}: no parameter named {name!r}")
        if how == "variadic" and not variadic:
            problem(f"{owner}: {name!r} is variadic here but Manim takes no *args")
        # A positional parameter may be reached through *args (Arrow passes start and end on).
        if how == "positional" and name not in names and not variadic:
            problem(f"{owner}: no parameter named {name!r}")


sig = json.load(sys.stdin)

for name, kind in sig["kinds"].items():
    checked += 1
    cls = getattr(manim, name, None) or getattr(manim.mobject.geometry.polygram, name, None)
    if not inspect.isclass(cls):
        problem(f"kind {name}: not a Manim class")
        continue
    if kind["parent"]:
        parent = getattr(manim, kind["parent"], None) or getattr(manim.mobject.geometry.polygram, kind["parent"])
        if not issubclass(cls, parent):
            problem(f"kind {name}: not a subclass of {kind['parent']}")
    check_params(f"kind {name}", cls, kind["params"])

for name, ctor in sig["ctors"].items():
    check_params(f"{name}()", getattr(manim, name), ctor["params"])

for name, overloads in sig["methods"].items():
    for method in overloads:
        cls = getattr(manim, method["receiver"])
        if not callable(getattr(cls, name, None)):
            problem(f"{method['receiver']}.{name}: no such method")
            continue
        check_params(f"{method['receiver']}.{name}", cls, method["params"], method=name)

for name, overloads in sig["queries"].items():
    for query in overloads:
        checked += 1
        cls = getattr(manim, query["receiver"])
        member = inspect.getattr_static(cls, name, None)
        if member is None:
            problem(f"{query['receiver']}.{name}: no such member")
        elif isinstance(member, property) != query["property"]:
            problem(f"{query['receiver']}.{name}: property flag is wrong")

for name, anim in sig["anims"].items():
    cls = getattr(manim, name, None)
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
