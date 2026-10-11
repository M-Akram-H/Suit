<img src="images/logo.png" width="96" align="right" alt="Suit's logo">

# Suit documentation

Suit is a visual editor for [Manim](https://www.manim.community/) slides. These chapters
explain how to use it and how it works inside. They are written to be read in order the
first time, and looked things up in afterwards.

## Using Suit

| Chapter | What it covers |
|---|---|
| [1. What Suit is](01-what-suit-is.md) | Decks, slides, steps and objects; what Manim does and what Suit does |
| [2. Installing](02-installing.md) | Docker on any machine, or straight on Linux; settings; updating |
| [3. Your first deck](03-first-deck.md) | A walk from an empty deck to a presented one |
| [4. The window](04-the-window.md) | Every panel and button, and the keyboard |
| [5. Objects](05-objects.md) | Everything that can be put on a slide, and its settings |
| [6. Steps and animations](06-steps-and-animations.md) | What happens on a slide, in what order, and for how long |
| [7. Graphs and tangents](07-graphs-and-tangents.md) | Functions, where they have a value, asymptotes, and lines that follow a curve |
| [8. Formulas](08-formulas.md) | Settings that are computed instead of typed |
| [9. Slides and presenting](09-slides-and-presenting.md) | Loops, notes, playing a slide, and presenting the deck |
| [10. Saving and exporting](10-saving-and-exporting.md) | Deck files, pictures, Python for manim-slides, the command line |
| [11. Problems](11-problems.md) | What the list at the bottom tells you, and what to do about it |
| [15. Troubleshooting](15-troubleshooting.md) | When something does not work |

## How Suit works

| Chapter | What it covers |
|---|---|
| [12. How it works](12-how-it-works.md) | The parts of the program and what passes between them |
| [13. The deck file](13-deck-file-format.md) | What is inside a `.suit.json` file |
| [14. Developing](14-developing.md) | Building, testing, the tools, and adding to the catalogue |
| [The specification](spec/manim-core.md) | *Manim Core*: the formal definition of what a deck means |

## Two example decks

They are in [`examples/`](../examples):

- `sample.suit.json`: six slides that use most of what Suit can do. It is the deck Suit
  opens when it is started without one.
- `tangent.suit.json`: the graph of 1/x and a tangent that glides along it, with its slope
  shown as a number that keeps up.
