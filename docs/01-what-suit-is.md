# 1. What Suit is

[Manim](https://www.manim.community/) makes mathematical animations from Python code.
[manim-slides](https://manim-slides.eertmans.be/) turns such animations into slides that
wait for a key. Both are written for people who program.

Suit is a window in which the same slides are made by clicking: you put objects on a
slide, say what happens to them step by step, and watch the result. No Python is written.
The drawing is still done by Manim itself, so what you see is what Manim makes.

![The editor with the sample deck open](images/editor.png)

## The words Suit uses

| Word | Meaning |
|---|---|
| **Deck** | The whole presentation. One file, ending in `.suit.json`. |
| **Slide** | One part of the deck. While presenting, a slide plays and then waits for you. |
| **Step** | One thing that happens on a slide: an object is made, an animation plays, the slide pauses. A slide is a list of steps, from top to bottom. |
| **Object** | Something that can be seen: a text, a formula, a circle, a graph, a picture. Every object has a name. |
| **Setting** | One property of an object or of a step: a radius, a colour, how long an animation takes. |
| **Formula** | A setting that is computed (`2 * radius`, `tangent.slope`) instead of typed as a plain value. |

An object lives on from the step that makes it. It is still there on the next slide unless
a step took it away, exactly as in a Manim scene.

## What Manim does and what Suit does

Suit never guesses what Manim would draw. Whenever the deck changes, Suit runs the deck in
a real Manim in the background and asks what is on the scene after every step. The canvas
shows the answer. When you press **Play**, Manim plays the slide through and Suit shows its
frames.

This has three consequences worth knowing:

- **What the canvas shows is true.** A formula is typeset by LaTeX, a text by Pango, a
  morph by Manim's own `Transform`.
- **An edit takes a moment to show.** After a change the bar says *Asking Manim…*. The
  slide you are on shows again within about a second; the slides after it, which the
  change may have moved, are brought up to date after that (*Updating the slides after
  this one…*). Slides before the one you
  changed are not run again.
- **Manim has to be installed.** With [Docker](02-installing.md) it comes in the box.

Suit adds what Manim cannot do, which is to look at a deck *before* running it. A checker
reads the steps and reports mistakes that Manim would only show as a wrong picture, or not
at all: an animation of something that is not on the slide, a fill colour that nothing
shows, a looping slide that would jump. [Chapter 11](11-problems.md) lists them.

## What comes out

- **A deck file** you can open again and keep editing.
- **A presentation, straight from Suit**: full screen, with the keys of manim-slides, and
  your notes in a second window.
- **A Python file for manim-slides**, if you want to render videos, present with
  manim-slides itself, or make an HTML page of the talk. The file is ordinary Manim code,
  one call per step.

## What Suit is not

- It is not all of Manim. Suit knows a *catalogue* of 32 kinds of object and 31
  animations, each checked against Manim. For anything else there is a step that holds
  Python code ([chapter 6](06-steps-and-animations.md#python-code)).
- It is not a video editor. There is no timeline to cut; a slide is as long as its steps.
- It does not render video files itself. Exporting gives the Python file, and
  manim-slides renders it.

Next: [2. Installing](02-installing.md)
