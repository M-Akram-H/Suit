# 3. Your first deck

A walk from an empty deck to one you present: a title, a shape that moves, and a graph
with a tangent. It takes about ten minutes. Suit is assumed to be running
([chapter 2](02-installing.md)).

## Start empty

Suit opens the sample deck. Choose **File → New deck** for an empty one. There is one
slide, called *Slide 1*, and it has no steps.

## A title

1. In the **Add** bar above the canvas, click **Text**.
2. A text appears and the keyboard is already in its field under **Properties**: type
   `Slopes`.

Two steps were added to the list on the right:

- **New text (Text)** makes the object.
- **Write text** is the animation that brings it onto the slide.

The top bar says *Asking Manim…* for a moment, and then the canvas shows the text as Manim
draws it.

To make it bigger, select the step **New text**, click **Add setting**, choose
**font size**, and set it to 72.

## A shape that moves

1. Click **Circle** in the Add bar. It arrives with a **Create circle** step.
2. Drag the circle on the canvas to the left. Because the circle has not moved since it was
   made, this simply changes where it is made.
3. With the circle still picked, click **Animate ▾** (the small arrow next to *Animate*)
   and choose **Change it → Move, turn, resize, recolour…**. A step *Change circle: shift*
   appears.
4. In Properties the change reads *shift by direction RIGHT*, which is one unit to the
   right. Switch **direction** to **distance** and make it 4 × RIGHT.

Press **Play** under the canvas, or the space bar. Manim plays the slide through and the
canvas shows it: the title is written, the circle is drawn, the circle glides to the
right. **Back to editing**, or Esc, returns to the steps.

> The first time a slide is played, Manim has to render it, which takes a few seconds.
> Suit does this in the background while you edit, so usually Play starts at once.

## Colour it

Select **New circle (Circle)**, click **Add setting** and choose **color**. Pick a colour
from the list. To fill the circle, add **fill color** as well: Suit adds **fill opacity**
with it, because Manim fills nothing unless told how much.

## A second slide

1. Click **Add slide** on the left. The new slide starts with everything the first one
   left on the scene, as a Manim scene would.
2. Clear the stage: click the circle on the canvas, then **Animate ▾ → Take it away →
   FadeOut**. Do the same for the text.

   To have both fade at the same moment, select the first FadeOut step, click
   **Add animation** in Properties, and choose the other object there. Then delete the
   second step.

## A graph and its tangent

1. **More → Graph of a function**. A sine wave appears, and the keyboard is in its rule:
   type `1 / x`. Suit works out where the rule has a value and draws the two branches
   separately.
2. Tick **Show asymptotes** in Properties to get dashed lines at x = 0 and y = 0.
3. With the graph picked, **More → Tangent to a graph**. A line touches the graph, with a
   dot at the point of contact.
4. Drag the tangent: it slides along the curve. Let go and it stays where you put it.
5. Click **Animate ▾ → Change it → Glide** and set **to x** to 3. Set **takes** to 4 s.

Press **Play**. The tangent travels along the curve and stays tangent the whole way.

![A tangent on the graph of 1/x, with its slope shown](images/tangent.png)

The example deck `examples/tangent.suit.json` is this slide with one thing more: a number
that keeps showing the tangent's slope. [Chapter 7](07-graphs-and-tangents.md) explains
how.

## Save it

**File → Save** (or Ctrl+S). Suit asks where: walk to a folder, type a name, and click
**Save**. The name ends in `.suit.json`. In Docker, save into the folder the browser
opens in, `/decks`: it is the `decks` folder next to `docker-compose.yml` on your
computer, and anything saved elsewhere is gone when the container stops.

## Present it

Click **Present**, or press F5. The deck takes the whole window.

| Key | What it does |
|---|---|
| → or Enter, or a click | Next slide |
| ← | Previous slide |
| Space | Hold the slide, and carry on |
| R | Play the slide again |
| N | Your notes, in a second window |
| Q or Esc | Back to the editor |

[Chapter 9](09-slides-and-presenting.md) has the rest.

## Where to go from here

- [5. Objects](05-objects.md) for everything that can be put on a slide.
- [6. Steps and animations](06-steps-and-animations.md) for everything that can happen
  to it.
- [10. Saving and exporting](10-saving-and-exporting.md) to turn the deck into a
  manim-slides presentation or an HTML page.

Next: [4. The window](04-the-window.md)
