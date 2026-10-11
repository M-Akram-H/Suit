"""Keeps Manim imported and runs probe scripts on request.

One request per line on stdin: the script's path, a tab, the path for its result. One reply
per request on stdout: a line starting with the record separator. Everything a script prints
goes to stderr, so it cannot be mistaken for a reply.

Scripts run one at a time per working directory, also across processes: Manim rewrites a
cached text file every time it uses it (`PangoUtils.remove_last_M`), so two Manims making
the same text at the same moment read each other's half-written files.
"""
import sys
import traceback

try:
    import fcntl
except ImportError:  # no such thing on Windows; there the directory is shared unguarded
    fcntl = None

import manim  # noqa: F401 - importing it is the point: it takes seconds, once

replies = sys.stdout
sys.stdout = sys.stderr
turn = open("manim.lock", "w")  # noqa: SIM115 - held for as long as the process lives

for line in sys.stdin:
    script, _, result = line.rstrip("\n").partition("\t")
    sys.argv = [script, result]
    if fcntl:
        fcntl.flock(turn, fcntl.LOCK_EX)
    try:
        with open(script, encoding="utf-8") as source:
            code = compile(source.read(), script, "exec")
        exec(code, {"__name__": "__main__"})
        status = "ok"
    except BaseException as error:  # noqa: BLE001 - a script must never take the server down
        traceback.print_exc()
        status = f"failed {type(error).__name__}: {error}".replace("\n", " ")
    finally:
        if fcntl:
            fcntl.flock(turn, fcntl.LOCK_UN)
    replies.write(f"\x1e{status}\n")
    replies.flush()
