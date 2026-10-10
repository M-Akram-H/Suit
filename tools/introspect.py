"""Print the real signatures of the Manim names the catalogue uses."""
import inspect
import sys

import manim


def sig(obj):
    try:
        return str(inspect.signature(obj))
    except (TypeError, ValueError) as error:
        return f"<{error}>"


def bases(cls):
    return " < ".join(c.__name__ for c in cls.__mro__[1:5])


for name in sys.argv[1:]:
    if "." in name:
        owner, attr = name.split(".", 1)
        try:
            member = inspect.getattr_static(getattr(manim, owner), attr)
        except AttributeError:
            print(f"{name}: MISSING")
            continue
        kind = "property" if isinstance(member, property) else "method"
        target = member.fget if isinstance(member, property) else getattr(getattr(manim, owner), attr)
        print(f"{name} [{kind}] {sig(target)}")
        continue
    obj = getattr(manim, name, None)
    if obj is None:
        print(f"{name}: MISSING")
    elif inspect.isclass(obj):
        print(f"{name}{sig(obj.__init__)}   [{bases(obj)}]")
    else:
        print(f"{name} = {obj!r}"[:150])
