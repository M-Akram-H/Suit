# 14. Developing

For changing Suit itself. [Chapter 12](12-how-it-works.md) explains the design; this one
is the practical side.

## What you need

- **Rust** 1.90 or newer ([rustup.rs](https://rustup.rs)). The code uses the 2024 edition.
- **Python** (3.12 is what the tests run on) with **Manim 0.21.0** and **manim-slides 5.7.0**, and what
  Manim needs from the system: Cairo, Pango and LaTeX.

On Ubuntu or Debian:

```bash
sudo apt install build-essential pkg-config python3-dev python3-venv \
    libcairo2-dev libpango1.0-dev \
    texlive-latex-extra texlive-fonts-recommended texlive-science cm-super dvisvgm
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
```

`requirements.txt` pins the two versions the catalogue is written against.
`docker/constraints.txt` lists every package of an environment in which all tests pass,
for an exact copy: `pip install -r requirements.txt -c docker/constraints.txt`.

Suit looks for its Python in this order: the `SUIT_PYTHON` environment variable; a
`.venv` next to the deck or in a folder above it; a `.venv` above the working directory;
the environment an installed Suit sets up for its user (`~/.local/share/suit/venv`, on
Windows `%LOCALAPPDATA%\Suit\venv`); and finally `python3` (on Windows `python`).

## Running

```bash
./run.sh                      # builds what changed and starts the editor on the sample deck
./run.sh --new                # an empty deck
./run.sh talk.suit.json       # a deck of your own
cargo run -p suit-cli -- check examples/sample.suit.json
```

`run.sh` checks first that its Python can import Manim, and says what to do if not.

## The layout

```
apps/suit            the window: command line, then hands over to crates/ui
apps/suit-cli        check, timeline, export, pdf, website, probe, signature, sample
crates/core          the deck's data types
crates/sig           the catalogue (builtin.rs), its types, Manim's colours
crates/check         the checker
crates/codegen       export and probes; formula.rs; probe.py, plot.py, tangent.py
crates/render        the Python sidecar; server.py
crates/export        the PDF (pdf.rs), the website (site.rs, render.py, assets/site/)
crates/engine        the session: edit.rs, preview.rs, present.rs, pictures.rs
crates/ui            the window: one file per panel
docs/                these chapters, and spec/manim-core.md
examples/            example decks; sample.suit.json is compiled into the editor
tools/               checks against the installed Manim, and the smoke test
docker/              what the Docker image needs besides the programs
```

## Tests

```bash
cargo test --workspace
```

runs the tests that need no Manim: about 130, in a second or two. They cover the checker
rule by rule, the export text, the formula language, every kind of edit with undo, the
pages of a PDF, and the window driven by simulated clicks and keys.

```bash
SUIT_PYTHON="$PWD/.venv/bin/python" cargo test --workspace -- --ignored
```

runs the 27 that use the real Manim, in two to three minutes: the probe, the
incremental preview against a run from scratch, recordings against predicted frame
counts, stopping a Manim that hangs, a website made, made again and stopped, and the
window from start to presenting.

Before a commit:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
```

Lines are 100 columns wide (`rustfmt.toml`), and `unsafe` code is forbidden in the whole
workspace.

## The tools

All of them need the Python with Manim.

| Tool | What it does |
|---|---|
| `tools/check_catalog.py` | Checks the catalogue against the installed Manim: every kind and its parent, every setting's name, default lengths and easings, constants, what a fill colour and a stroke colour do to each object, and that what is made of parts (Axes, a Grid) shows nothing of the look it is made with while `set color` and `set stroke` do, nor a picture of a colour. |
| `tools/verify_quirks.py` | Confirms the Manim behaviours the specification's section 6 relies on, one verdict per claim. |
| `tools/verify_render.py` | Compares the frames of a manim-slides render with the numbers the checker predicted. |
| `tools/contact_sheet.py` | Tiles sampled frames of every slide of a render into one picture, for a look at a whole deck. |
| `tools/introspect.py` | Prints Manim's real signatures for the names the catalogue uses. For when the catalogue is extended. |
| `tools/gen_colors.py` | Regenerates `crates/sig/src/colors.rs` from Manim's colour constants. |
| `tools/smoke.sh` | Starts the real window on a hidden display, presses the keys a presenter would, and saves a picture after each. Needs Xvfb, openbox, xdotool, ImageMagick and a software Vulkan driver. |

The catalogue check is the one to run after any change to `crates/sig`:

```bash
cargo run -q -p suit-cli -- signature | .venv/bin/python tools/check_catalog.py
```

```
manim 0.21.0: 690 catalogue facts checked, 0 problem(s)
```

A picture of the window, for documentation or for a bug report:

```bash
cargo run -p suit -- examples/sample.suit.json --select 2:6 --screenshot editor.png
```

## Adding to the catalogue

Say Manim has an object or an animation that Suit should offer.

1. **Look at what Manim takes.** `tools/introspect.py` prints the real signature.
2. **Describe it in `crates/sig/src/builtin.rs`**: its kind and parent, its settings with
   their types, and for an animation the *role* of each object it takes (does it bring
   the object in, change it, use it up?). The roles are what the checker reasons with;
   the specification's section 5.4 defines them.
3. **Run the catalogue check.** It fails if a name or a default is not Manim's.
4. **For an object**, give it a place in the window: the `PALETTE` or `MORE` list in
   `crates/ui/src/stage.rs`, an everyday name and what it starts with in
   `crates/engine/src/edit.rs`. Friendlier labels for settings go in `label()` in
   `crates/ui/src/editors.rs`. An animation needs none of this: the **▾** menu is
   filled from the catalogue.
5. **Add it to Appendix A** of the specification.

Nothing in the checker or the code generator changes: they work from the catalogue.

Something Manim does not have, like the graph that looks before it draws, is written in
Python next to `crates/codegen/src/plot.py`, added to the catalogue the same way, and
emitted into probes and exports by `crates/codegen/src/lib.rs`.

## The specification comes first

`docs/spec/manim-core.md` defines the language: syntax, typing rules, meaning,
translation. A change to what a deck *means* is a change to that document first, and to
the code second. A claim in it about Manim is marked **[src]** if it was read in Manim's
source and **[run]** if a tool confirms it by running Manim.

Changes that are only about the window need no change to the specification.

## Building the installers

Both are built on Linux and land in `out/dist/`. Neither holds Manim: each brings a
set-up step that installs it for one user (`packaging/linux/suit-setup`,
`packaging/windows/setup-manim.ps1`).

**The Fedora package** is built in a Fedora container, so that it is linked against
Fedora's own libraries. It needs Docker and nothing else:

```bash
packaging/linux/build-rpm.sh              # for Fedora 44
FEDORA=43 packaging/linux/build-rpm.sh    # for another version
```

`packaging/linux/suit.spec` says what the package contains and requires. Its LaTeX
requirements are the smallest set with which Manim's usual formulas typeset; the test
for a change to them is to install the package in a clean `fedora` container and run
`suit-setup` there.

**The Windows setup program** is cross-compiled and then packed with
[NSIS](https://nsis.sourceforge.io):

```bash
rustup target add x86_64-pc-windows-gnu
sudo apt install mingw-w64 nsis           # Fedora: mingw64-gcc mingw32-nsis
packaging/windows/build-windows.sh
```

`packaging/windows/suit.nsi` is the setup program's script. `apps/suit/build.rs` puts
the icon into `suit.exe`. The program can be tried under Wine; give Wine a prefix of its
own and switch off `winemenubuilder`, or it writes shortcuts into your real desktop:

```bash
export WINEPREFIX=/tmp/suit-wine WINEDLLOVERRIDES=winemenubuilder.exe=d
WGPU_BACKEND=vulkan wine target/windows/stage/suit.exe
```

Neither installer is signed. Windows therefore warns before running the setup program.

## The Docker image

```bash
docker compose build          # after changing the code
docker compose up
```

`Dockerfile` has three stages: the editor built from source with the official Rust image,
Manim installed into a Python environment, and the image that runs, which takes the
results of the other two and adds LaTeX and a screen that is reached through a browser.
`docker/start.sh` is what runs when the container starts.

Next: [15. Troubleshooting](15-troubleshooting.md)
