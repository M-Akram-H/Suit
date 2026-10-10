"""Keeps Manim imported and runs probe scripts on request.

One request per line on stdin: the script's path, a tab, the path for its result. One reply
per request on stdout: a line starting with the record separator. Everything a script prints
goes to stderr, so it cannot be mistaken for a reply.
"""
import sys
import traceback

import manim  # noqa: F401 - importing it is the point: it takes seconds, once

replies = sys.stdout
sys.stdout = sys.stderr

for line in sys.stdin:
    script, _, result = line.rstrip("\n").partition("\t")
    sys.argv = [script, result]
    try:
        with open(script, encoding="utf-8") as source:
            code = compile(source.read(), script, "exec")
        exec(code, {"__name__": "__main__"})
        status = "ok"
    except BaseException as error:  # noqa: BLE001 - a script must never take the server down
        traceback.print_exc()
        status = f"failed {type(error).__name__}: {error}".replace("\n", " ")
    replies.write(f"\x1e{status}\n")
    replies.flush()
