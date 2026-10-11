# 5. Objects

An object is anything that can be seen on a slide. This chapter lists what there is, and
explains the settings all objects share.

## Adding an object

Click its name in the **Add** bar. Two steps are added after the selected step: one that
makes the object, and one that brings it onto the slide (*Write* for texts and numbers,
*Create* for shapes, *FadeIn* for pictures). Delete the second step if you want to bring
the object in some other way.

A new text, formula or graph takes the keyboard straight away, so its words or its rule
can be typed at once.

## What there is

### In the Add bar

| Button | What it makes | Starts as |
|---|---|---|
| **Text** | Plain text, typeset with the fonts of the system | the word *Text* |
| **Formula** | A LaTeX formula (Manim's `MathTex`) | x² |
| **Circle**, **Square**, **Rectangle**, **Triangle**, **Star**, **Dot** | The shape | Manim's usual size |
| **Line**, **Arrow** | From a start to an end, each a point or another object | one unit either side of the middle |
| **Axes** | A pair of axes with ticks | Manim's usual ranges |
| **Number line** | One axis | |
| **Picture** | An image from a file ([below](#pictures)) | |

### Under More

| Entry | What it makes |
|---|---|
| **Ellipse**, **Rounded rectangle**, **Regular polygon**, **Polygon**, **Arc** | More shapes. A polygon is given its corners one by one. |
| **Dashed line**, **Double arrow** | More lines |
| **Brace under…** | A curly brace along another object |
| **Frame around…** | A rectangle drawn around another object |
| **Number**, **Whole number** | A number shown as digits, which a step can change |
| **LaTeX text** | Text typeset by LaTeX (Manim's `Tex`) |
| **Styled text** | Text with bold, slanted and coloured parts (Pango markup) |
| **Grid** | A coordinate grid (Manim's `NumberPlane`) |
| **Graph of a function**, **Tangent to a graph** | [Chapter 7](07-graphs-and-tangents.md) |
| **Curve** | A curve traced by a point that depends on `t` |
| **Number that can change** | A number that is not seen, for other things to follow ([chapter 6](06-steps-and-animations.md#keep-doing)) |
| **Named value** | A number with a name, for formulas to use ([chapter 8](08-formulas.md)) |

*Brace under…*, *Frame around…* and *Tangent to a graph* are made around another object:
the one that is picked if it suits, otherwise the newest one that does. If there is none,
Suit says so.

## Settings

Select the step that makes an object, or pick the object on the canvas. Properties shows
its **name** and the settings it was made with.

- **Add setting** lists the settings that can still be given. One that is not given has
  Manim's usual value.
- **×** next to a setting removes it again.
- **fx** turns a setting into a formula ([chapter 8](08-formulas.md)).
- **Then also…** does something to the object as soon as it is made: move it to a point,
  put it next to another object, scale it, turn it. This is how an object is *placed*.

The name is what the steps and formulas call the object. Change it freely; everything
that mentions the object follows.

### Where things are

The frame is 8 units high and, at 16:9, about 14.2 units wide. The middle is (0, 0); x
grows to the right and y upwards. A point is written `(x, y)`.

A setting that is a direction or a position can be given in three ways, chosen from a
small menu next to it:

| Form | Example | Meaning |
|---|---|---|
| direction | `RIGHT` | One unit that way. Also `UP`, `DOWN`, `LEFT`, and the diagonals `UL`, `UR`, `DL`, `DR`. |
| distance | `2.5 × LEFT` | That many units that way |
| point | `(3, -1)` | Exactly there |

### Colours

Every object can be given a **color** (Axes and a Grid in a way of their own, below).
Shapes have more:

| Setting | What it colours |
|---|---|
| **color** | The whole object. For a shape that is its outline, and its inside once that is filled. This is the one you usually want. |
| **fill color**, **fill opacity** | The inside. Manim leaves the inside of a shape fully transparent unless a fill opacity is given, so Suit adds the opacity when you add the colour. |
| **stroke color**, **stroke opacity**, **stroke width** | The outline |

Lines, arrows, graphs and tangents have no inside, so a fill is not offered for them.

A colour is picked from Manim's named colours (`BLUE`, `YELLOW`, `GRAY`, and their shades
such as `BLUE_A` to `BLUE_E`), or written as a formula like `#58c4dd`.

**Axes** and a **Grid** are offered none of these settings. Manim builds their lines
separately, and a colour or an outline given to the whole reaches none of them. Colour
them with **Then also… → set color**, and change the width or the opacity of their lines
with **Then also… → set stroke**: those go to every line.

If a deck gives them such a setting all the same, Suit warns that it shows nothing, and
Properties offers **Use set color instead** (or **set stroke**, or both), which moves the
setting to where it shows. A fill has nowhere to go, since there is no inside: take it
away with **×**.

## Pictures

**Picture** in the Add bar opens a file browser. Choose a PNG, JPEG, GIF, BMP or WebP file.

A deck remembers a picture by where it is *relative to the deck file*. Keep the pictures
in the deck's folder, or below it, and the deck can be moved or sent to someone together
with them.

**File → Gather pictures** copies every picture that lives somewhere else into a folder
next to the deck, and points the deck at the copies.

## Groups

A group is several objects that move and are animated as one.

- Shift-click the objects on the canvas and choose **Group these…**, or
- choose **More → Group…** above the steps and tick the objects.

Grouping is a step: from that step on the group has a name of its own, and its members
are used through the group.

## Copies

**More → Duplicate** above the steps makes a copy of the picked object, next to it. The
copy is an object of its own from then on.

## The whole catalogue

Every kind of object Suit knows, with the settings of its own. All of them also take the
colour settings above, except Axes and NumberPlane.

| Object | Settings of its own |
|---|---|
| Arc | radius, start angle, angle, arc center |
| Arrow, DoubleArrow | start, end, buff |
| Axes | x range, y range, x length, y length, tips |
| Brace | object, direction, buff |
| Circle | radius |
| DashedLine | start, end, buff, dash length, dashed ratio |
| DecimalNumber | number, decimals, include sign, font size |
| Dot | point, radius |
| Ellipse | width, height |
| FunctionGraph | function, x range |
| ImageMobject (picture) | file |
| Integer | number, font size |
| Line | start, end, buff, path arc |
| MarkupText | text, font size, font |
| MathTex, Tex | formula, font size |
| NumberLine | x range, length, include numbers, include ticks, include tip |
| NumberPlane (grid) | x range, y range, x length, y length |
| ParametricFunction (curve) | function, t range |
| Plot (graph) | function, on, step, asymptotes |
| Polygon | vertices |
| Rectangle | width, height |
| RegularPolygon | n |
| RoundedRectangle | corner radius, width, height |
| Square | side length |
| Star | n, outer radius, inner radius |
| SurroundingRectangle (frame) | objects, buff, corner radius |
| Tangent | graph, at x, length, mark the point |
| Text | text, font size, font, weight, slant, line spacing |
| Triangle | |
| ValueTracker | value |

The names are Manim's, so Manim's own documentation says what each setting does.

Next: [6. Steps and animations](06-steps-and-animations.md)
