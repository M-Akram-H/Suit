//! A small deck that exercises most of the language; `suit sample` writes it out.

use suit_core::{Anim, Arg, Call, Deck, DeckConfig, Expr, Object, Place};

fn text(content: &str, size: f64) -> Object {
    Object::new("Text")
        .arg("text", content)
        .arg("font_size", size)
}

fn next_to(target: &Place, direction: &str) -> Call {
    Call::new("next_to")
        .arg("mobject_or_point", target)
        .arg("direction", Expr::konst(direction))
}

fn scaled(factor: f64, direction: &str) -> Expr {
    Expr::call("*", vec![Expr::Real(factor), Expr::konst(direction)])
}

fn fade_in(place: &Place) -> Anim {
    Anim::prim("FadeIn").arg("mobject", place)
}

fn fade_out(place: &Place) -> Anim {
    Anim::prim("FadeOut").arg("mobject", place)
}

pub fn deck() -> Deck {
    let mut deck = Deck::new(DeckConfig::default());

    let mut slide = deck.add_slide("Title");
    slide.config(|config| config.notes = "Welcome everyone.".to_owned());
    let title = slide.object("title", text("Manim, without the code", 56.0));
    let subtitle = slide.object(
        "subtitle",
        text("made with Suit", 30.0)
            .arg("color", Expr::konst("GRAY"))
            .then(next_to(&title, "DOWN")),
    );
    slide.play(vec![
        Anim::prim("Write").arg("vmobject", &title).run_time(1.5),
    ]);
    slide.play(vec![fade_in(&subtitle).arg("shift", scaled(0.3, "UP"))]);

    let mut slide = deck.add_slide("Shapes");
    slide.play(vec![
        fade_out(&title).arg("shift", Expr::konst("UP")),
        fade_out(&subtitle),
    ]);
    let circle = slide.object(
        "circle",
        Object::new("Circle")
            .arg("radius", 1.2)
            .arg("color", Expr::konst("BLUE"))
            .arg("fill_opacity", 0.4),
    );
    let label = slide.object("label", text("a shape", 28.0));
    // The label follows the circle wherever it goes.
    slide.always(&label, next_to(&circle, "DOWN"));
    slide.play(vec![
        Anim::prim("Create").arg("mobject", &circle),
        fade_in(&label),
    ]);
    slide.play(vec![Anim::animate(
        &circle,
        vec![Call::new("shift").arg("vectors", scaled(2.0, "LEFT"))],
    )]);
    let square = slide.object(
        "square",
        Object::new("Square")
            .arg("side_length", 2.2)
            .arg("color", Expr::konst("GREEN"))
            .arg("fill_opacity", 0.4)
            .then(Call::new("move_to").arg("point_or_mobject", &circle)),
    );
    slide.detach(&label);
    slide.always(&label, next_to(&square, "DOWN"));
    slide.play(vec![
        Anim::prim("ReplacementTransform")
            .arg("mobject", &circle)
            .arg("target_mobject", &square),
    ]);
    slide.play(vec![Anim::animate(
        &square,
        vec![Call::new("move_to").arg("point_or_mobject", Expr::konst("ORIGIN"))],
    )]);

    let mut slide = deck.add_slide("Dots");
    slide.play(vec![fade_out(&square), fade_out(&label)]);
    let dots: Vec<Place> = (1..=5)
        .map(|n| {
            let dot = Object::new("Dot")
                .arg("radius", 0.18)
                .arg("color", Expr::konst("YELLOW"));
            slide.object(&format!("dot {n}"), dot)
        })
        .collect();
    let row = slide.object(
        "row",
        Object::vgroup(&dots.iter().collect::<Vec<_>>()).then(
            Call::new("arrange")
                .arg("direction", Expr::konst("RIGHT"))
                .arg("buff", 0.7),
        ),
    );
    let drop_in = |dot: &Place| fade_in(dot).arg("shift", scaled(0.5, "DOWN")).run_time(0.6);
    slide.play(vec![Anim::par(dots.iter().map(drop_in).collect()).lag(0.3)]);

    let mut slide = deck.add_slide("Pulse");
    slide.config(|config| config.looping = true);
    let pulse = |dot: &Place| Anim::prim("Indicate").arg("mobject", dot).run_time(0.6);
    slide.play(vec![Anim::par(dots.iter().map(pulse).collect()).lag(0.3)]);

    let mut slide = deck.add_slide("Graph");
    slide.play(vec![fade_out(&row)]);
    let axes = slide.object(
        "axes",
        Object::new("Axes")
            .arg("x_range", Expr::vec3(-4.0, 4.0, 1.0))
            .arg("y_range", Expr::vec3(-2.0, 2.0, 1.0))
            .arg("x_length", 8.0)
            .arg("y_length", 4.0)
            .arg("tips", false),
    );
    let x = slide.var("x");
    let sine = slide.object(
        "sine",
        Object::new("FunctionGraph")
            .arg(
                "function",
                Arg::fun(x, Expr::call("sin", vec![Expr::Var(x)])),
            )
            .arg("x_range", Expr::vec3(-4.0, 4.0, 0.05))
            .arg("color", Expr::konst("TEAL")),
    );
    slide.play(vec![
        Anim::prim("Create").arg("mobject", &axes).run_time(1.5),
    ]);
    slide.play(vec![
        Anim::prim("Create").arg("mobject", &sine).run_time(2.0),
    ]);
    // A tracker holds a number; the dot reads it every frame.
    let t = slide.object("t", Object::new("ValueTracker").arg("value", -4.0));
    let dot = slide.object(
        "tracer",
        Object::new("Dot").arg("color", Expr::konst("RED")),
    );
    let now = || Expr::query(&t, "get_value");
    let on_curve = Expr::Vec3(Box::new([
        now(),
        Expr::call("sin", vec![now()]),
        Expr::Real(0.0),
    ]));
    slide.always(&dot, Call::new("move_to").arg("point_or_mobject", on_curve));
    slide.add(&[&t, &dot]);
    slide.play(vec![
        Anim::animate(&t, vec![Call::new("set_value").arg("value", 4.0)])
            .run_time(4.0)
            .rate("linear"),
    ]);

    let mut slide = deck.add_slide("End");
    slide.play(vec![fade_out(&axes), fade_out(&sine), fade_out(&dot)]);
    let thanks = slide.object("thanks", text("Thank you", 64.0));
    slide.play(vec![Anim::prim("Write").arg("vmobject", &thanks)]);

    deck
}
