# 4. The window

![The editor with the sample deck open](images/editor.png)

The window has five parts: a bar along the top, the slides on the left, the canvas in the
middle, the steps and their properties on the right, and the problems along the bottom.
The borders between them can be dragged.

## The top bar

| Item | What it does |
|---|---|
| **Suit** *file name* | The open deck. A dot after the name means there are unsaved changes. |
| **File** | New deck, Open…, Open the sample deck, Save, Save as…, Gather pictures, Export PDF…, Export website…, Export Python ([chapter 10](10-saving-and-exporting.md)). |
| **Undo**, **Redo** | Every change to the deck can be undone. Hover to see what would be undone. |
| **Present** | Shows the deck full screen from the selected slide ([chapter 9](09-slides-and-presenting.md)). |
| **▾** next to Present | How a presentation runs, whether your notes get a window, and whether slides are made ready to play while you edit. |
| *Asking Manim…* | Manim is working out what the deck looks like. After four seconds a **Stop** button appears, in case it never finishes. |
| *Updating the slides after this one…* | The slide you are on is up to date; Manim is redoing the slides after it, which the change may have moved. **Stop** appears here too. |
| *Preview stopped: …* | Manim could not run the deck. The reason is given; **Try again** asks once more. |
| *Website: …* | A website of the deck is being made, and how far it has got. **Stop** stops it. When it is done, the bar says where it went and **Watch it** opens it in your browser. |
| on the right | Number of slides, the length of the deck, and its size and frame rate. |

## Slides

The list on the left shows every slide with a small picture of how it ends, its title and
how long it takes. A looping slide is marked ⟳, and a slide with a problem is marked ⚠.

- Click a slide to select it.
- **Add slide** puts a new one after the selected slide.
- **↑** and **↓** move the selected slide.

With a slide selected and no step selected, Properties shows the slide's own settings
([chapter 9](09-slides-and-presenting.md#a-slides-settings)).

## The Add bar and the canvas

The **Add** bar puts a new object on the slide, after the selected step:
Text, Formula, Circle, Square, Rectangle, Triangle, Star, Dot, Line, Arrow, Axes,
Number line, Picture, and **More** for the rest ([chapter 5](05-objects.md)).

The canvas shows the scene as it is after the selected step.

| On the canvas | What happens |
|---|---|
| Click an object | It is picked: outlined, with its name. Properties shows its settings. |
| Shift-click another | Both are picked. **Group these…** puts them together. |
| Drag an object | It moves. What that means is asked where it is not obvious ([chapter 6](06-steps-and-animations.md#dragging-on-the-canvas)). |
| Double-click a text | The keyboard goes to its words. |
| Click empty canvas | Nothing is picked. |

Under the canvas is **Play**, which plays the selected slide the way Manim animates it.
While it plays there is a bar to drag through the slide, and **Back to editing**.

## Steps

The top right lists the steps of the selected slide. *Start of the slide* stands for the
moment before the first step. Each step has an icon that says what kind it is, and the
time it takes if it takes any.

| Icon | Kind of step |
|---|---|
| + | A new object, or a named value |
| ▶ | An animation, or several at once |
| ✏ | A change made instantly |
| 👁 / 🚫 | An object shown or hidden without an animation |
| ⏸ | A pause |
| 🔁 / ⏹ | An object starts, or stops, doing something on every frame |
| 🐍 | Python code |

The buttons above the list act on the object that is picked, or on the selected step:

| Button | What it does |
|---|---|
| **Animate** | Adds an animation of the picked object: it fades in if it is not on the slide, and is pointed out if it is. |
| **▾** | Every animation that suits the object as it is now, sorted by what it is for. |
| **Change** | Changes the object instantly, between two frames. |
| **Show**, **Hide** | Puts the object on the slide, or takes it off, without an animation. |
| **Pause** | Holds still for a moment. |
| **More** | Keep doing…, Stop keeping on, Duplicate, Python code, Group… |
| **↑**, **↓** | Move the selected step. |
| **Delete** | Deletes the selected step. |

A button is greyed out when there is nothing it could apply to.

## Properties

The bottom right shows what is selected, and is where it is changed:

- **A step**: its settings. For a new object these are the object's settings
  ([chapter 5](05-objects.md#settings)); for an animation, which animation, of what, for
  how long ([chapter 6](06-steps-and-animations.md#the-settings-of-an-animation)).
- **A picked object**: the settings it was made with, the slide and step that made it,
  and **Go to that step**.
- **A slide**: its title, what happens at its end, and your notes.

**As text** at the bottom shows the step written out, which is a compact way to read
what a step does exactly.

## Problems

The list along the bottom says what is wrong with the deck, or that nothing is. Click a
problem to go to its step. [Chapter 11](11-problems.md) explains each kind.

## The keyboard

| Key | In the editor |
|---|---|
| Ctrl+Z | Undo |
| Ctrl+Shift+Z or Ctrl+Y | Redo |
| Ctrl+S | Save |
| ↑ / ↓ | Select the step above or below |
| Page Up / Page Down | Select the previous or next slide |
| Delete or Backspace | Delete the selected step |
| Space | Play the slide, pause, carry on |
| Esc | Stop playing |
| F5 | Present |

Plain keys go to a text field while one is being typed in. On a Mac, running Suit
directly, Ctrl is ⌘. In Docker the keys go through the web browser, which keeps some for
itself: use the **Present** button if F5 reloads the page.

The keys while presenting are in [chapter 9](09-slides-and-presenting.md#keys).

Next: [5. Objects](05-objects.md)
