# 15. Troubleshooting

## Docker

**`docker compose up` fails while building.**
Mostly a download that broke. Run it again: Docker keeps the stages that finished and
carries on from there. If it keeps failing at the same place, the message above the
failure says which download or which command it was.

**`docker: 'compose' is not a docker command`, or `docker-compose: command not found`.**
Compose is missing. With Docker Desktop it is included. On Linux, install the
`docker-compose-plugin` package and use `docker compose` (with a space).

**The browser says it cannot connect, or noVNC shows *Failed to connect to server*.**
The container is not running, or not yet. Look at it:

```bash
docker compose ps
docker compose logs
```

The address works once the log says *Suit is starting*. If the container has stopped,
the log ends with the reason.

**`port is already allocated`.**
Something else on your computer uses port 6080. Choose another, in a file `.env` next to
`docker-compose.yml`:

```
SUIT_PORT=6081
```

and open `http://localhost:6081`.

**The container does not start on Windows, and the log mentions `bash\r` or
`no such file or directory`.**
The start script was checked out with Windows line endings. The repository tells Git not
to do that, so this only happens with a copy made in another way. Clone again with
`git clone`, and rebuild with `docker compose build`.

**Keys do nothing.**
Click once inside the picture: the keyboard goes to Suit only while the picture has it.
Keys the browser keeps for itself (F5, Ctrl+W, sometimes Ctrl+S) never arrive; use the
**Present** button and the **File** menu.

**The picture is blurred, or everything is too small.**
The browser scales the screen inside the container to its window. Set `SUIT_SCREEN` in
`.env` to the size of your browser window for a sharp picture, or to something smaller,
like `1280x720`, to make everything larger. Then `docker compose up` again.

**A deck I saved is gone.**
It was saved outside `/decks`, and went with the container. Only the `decks` folder is
kept ([chapter 2](02-installing.md#where-your-decks-are)).

**The files in `decks` belong to root (Linux).**
This happens if the `decks` folder did not exist and Docker made it. Take it back, and
it stays yours:

```bash
sudo chown -R "$USER" decks
```

## The canvas stays empty

**The top bar says *Preview stopped: … has no Manim*.**
Suit found a Python that does not have Manim. Without Docker, start Suit with
`./run.sh`, which looks for the right one and says what is missing, or name the Python in
`SUIT_PYTHON`. If `run.sh` reports that the `.venv` was made for another Python version,
the `.venv` was created on another system or before an upgrade: delete it and make it
again ([chapter 2](02-installing.md#3-suit-and-manim)).

**It says *Asking Manim…* for a long time.**
The very first time, LaTeX typesets every formula and builds its font caches, which
takes a while. Later runs reuse the results. If it never ends, **Stop** appears
after four seconds; press it, and **Try again**.

**It says *Preview stopped* with a LaTeX error.**
A formula cannot be typeset. The message names it. Common causes are an unclosed brace
and a command from a LaTeX package that is not loaded. Correct the formula; the preview
starts again by itself.

**It says *Preview stopped* with a Python error.**
If the deck has a **Python code** step, look there first. Otherwise a formula probably
divides by zero.

**The canvas says *Manim stopped before this, so the picture is an earlier one*.**
Manim could not run the deck as it is now, and the canvas kept what it had. The top bar
and Problems say why.

## Things that look wrong but are Manim

**A fill colour does not show.**
Manim fills nothing unless given a fill opacity, and a line has no inside at all. Suit
warns and offers the correction ([chapter 5](05-objects.md#colours)).

**A colour given to Axes or a Grid does not show.**
Manim builds their lines separately, and a setting of the whole reaches none of them. Suit
does not offer such a setting, and warns of one that is there. Use **Then also… → set
color** ([chapter 5](05-objects.md#colours)).

**A graph does not line up with the ticks of the Axes.**
A graph is drawn in scene units, and Manim's Axes are not. Use a **Grid** behind a graph
([chapter 7](07-graphs-and-tangents.md#units-and-what-to-draw-it-on)).

**An object rotates by shrinking and growing instead of turning.**
That is how Manim animates a changed *rotate*. Use the **Rotate** animation
([chapter 6](06-steps-and-animations.md#animating-a-change)).

**A slide starts with the objects of the slide before.**
Slides share one scene. Begin a slide with FadeOut steps for what should go
([chapter 9](09-slides-and-presenting.md#slides)).

## Speed

**Every edit takes about half a second to show.**
That is Manim working out the scene. Slides before the one you are editing are not run
again, so a long deck is no slower than a short one.

**Play does not start at once.**
The slide has not been played through by Manim yet, or has just changed. Suit does this
in the background when the deck is left alone for a moment.

**Presenting stutters.**
Suit draws every frame as it shows it. For a talk, export the deck and present the
rendered result ([chapter 10](10-saving-and-exporting.md#rendering-and-presenting-the-export)).

## Starting afresh

Suit keeps nothing but a cache of typeset texts. Deleting it is always safe:

```bash
rm -rf ~/.cache/suit                 # without Docker
docker compose down --volumes        # with Docker
```

## Reporting a problem

Open an issue at <https://github.com/M-Akram-H/Suit/issues> with:

- what you did, what you expected, and what happened instead;
- the message from the top bar or from Problems, word for word;
- the deck file, if you can share it;
- how you run Suit (Docker or directly), and on which system.

For a problem with starting it, add the output of `docker compose logs`, or of `./run.sh`.

Back to the [contents](README.md)
