//! One test per rule of docs/spec/manim-core.md §5.

use suit_check::{Checked, Severity, State, check};
use suit_core::{Anim, Arg, Call, Deck, DeckConfig, Expr, Object, Place, Presence, Time};
use suit_sig::Signature;

fn deck() -> Deck {
    Deck::new(DeckConfig {
        gap: Time::ZERO,
        ..DeckConfig::default()
    })
}

fn run(deck: &Deck) -> Checked {
    check(deck, &Signature::builtin())
}

/// The codes of all diagnostics, errors and warnings alike.
fn codes(deck: &Deck) -> Vec<&'static str> {
    run(deck).diagnostics.iter().map(|d| d.code).collect()
}

fn circle() -> Object {
    Object::new("Circle")
}

fn fade_in(place: &Place) -> Anim {
    Anim::prim("FadeIn").arg("mobject", place)
}

fn fade_out(place: &Place) -> Anim {
    Anim::prim("FadeOut").arg("mobject", place)
}

fn shift(by: &str) -> Call {
    Call::new("shift").arg("vectors", Expr::konst(by))
}

#[test]
fn a_well_formed_deck_has_no_diagnostics() {
    let mut deck = deck();
    let mut slide = deck.add_slide("Intro");
    let c = slide.object(
        "circle",
        circle()
            .arg("radius", 1.5)
            .arg("color", Expr::konst("BLUE")),
    );
    let label = slide.object(
        "label",
        Object::new("Text").arg("text", "Hello").then(
            Call::new("next_to")
                .arg("mobject_or_point", &c)
                .arg("direction", Expr::konst("DOWN")),
        ),
    );
    slide.play(vec![
        Anim::prim("Create").arg("mobject", &c),
        fade_in(&label),
    ]);
    slide.wait(0.5);
    slide.play(vec![Anim::animate(&c, vec![shift("LEFT")]).run_time(2.0)]);
    let mut slide = deck.add_slide("Outro");
    slide.play(vec![fade_out(&c), fade_out(&label)]);

    let checked = run(&deck);
    assert_eq!(checked.diagnostics, []);
    assert_eq!(checked.slides[0].frames, 60 + 30 + 120);
    assert_eq!(checked.slides[1].frames, 60);
    // The first slide leaves both objects on scene; that is the second slide's interface.
    assert!(
        checked.slides[0]
            .exit
            .iter()
            .all(|o| o.state == State::Is(Presence::On))
    );
    assert_eq!(checked.slides[1].entry, checked.slides[0].exit);
    assert!(
        checked.slides[1]
            .exit
            .iter()
            .all(|o| o.state == State::Is(Presence::Off))
    );
}

#[test]
fn animating_something_that_is_not_on_scene() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let c = slide.object("circle", circle());
    slide.play(vec![Anim::prim("Indicate").arg("mobject", &c)]);
    assert_eq!(codes(&deck), ["presence"]);

    let checked = run(&deck);
    assert_eq!(checked.diagnostics[0].severity, Severity::Error);
    assert!(
        checked.diagnostics[0]
            .message
            .contains("“circle” is not on the slide yet")
    );
}

#[test]
fn introducing_something_twice() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let c = slide.object("circle", circle());
    slide.play(vec![fade_in(&c)]);
    slide.play(vec![fade_in(&c)]);
    assert_eq!(codes(&deck), ["presence"]);
}

#[test]
fn an_outro_leaves_the_object_usable_but_a_consume_does_not() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let c = slide.object("circle", circle());
    let s = slide.object("square", Object::new("Square"));
    slide.play(vec![fade_in(&c)]);
    slide.play(vec![fade_out(&c)]);
    slide.play(vec![fade_in(&c)]);
    slide.play(vec![
        Anim::prim("ReplacementTransform")
            .arg("mobject", &c)
            .arg("target_mobject", &s),
    ]);
    assert_eq!(codes(&deck), [] as [&str; 0]);

    // The circle is spent now; the square took its place.
    deck.add_slide("t")
        .play(vec![Anim::prim("Indicate").arg("mobject", &s), fade_in(&c)]);
    assert_eq!(codes(&deck), ["spent"]);
}

#[test]
fn one_object_one_animation_at_a_time() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let c = slide.object("circle", circle());
    slide.add(&[&c]);
    slide.play(vec![
        Anim::animate(&c, vec![shift("UP")]),
        Anim::prim("Indicate").arg("mobject", &c),
    ]);
    assert_eq!(codes(&deck), ["overlap"]);
}

#[test]
fn reading_a_target_is_not_a_conflict_but_changing_it_is() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let c = slide.object("circle", circle());
    let s = slide.object("square", Object::new("Square"));
    let d = slide.object("dot", Object::new("Dot"));
    slide.add(&[&c, &d]);
    let onto_square = |source: &Place| {
        Anim::prim("Transform")
            .arg("mobject", source)
            .arg("target_mobject", &s)
    };
    slide.play(vec![onto_square(&c), onto_square(&d)]);
    assert_eq!(codes(&deck), [] as [&str; 0]);

    let mut slide = deck.add_slide("t");
    slide.add(&[&s]);
    slide.play(vec![onto_square(&c), Anim::animate(&s, vec![shift("UP")])]);
    assert_eq!(codes(&deck), ["overlap"]);
}

#[test]
fn animations_are_checked_against_kinds() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let image = slide.object(
        "photo",
        Object::new("ImageMobject").arg("filename_or_array", "photo.png"),
    );
    let text = slide.object("title", Object::new("Text").arg("text", "Hi"));
    let formula = slide.object(
        "formula",
        Object::new("MathTex")
            .arg("tex_strings", "a")
            .arg("tex_strings", "b"),
    );
    slide.add(&[&text]);
    slide.play(vec![Anim::prim("Create").arg("mobject", &image)]);
    slide.play(vec![
        Anim::prim("TransformMatchingTex")
            .arg("mobject", &text)
            .arg("target_mobject", &formula),
    ]);
    assert_eq!(codes(&deck), ["kind", "kind"]);
}

#[test]
fn transform_weakens_the_kind_and_seals_the_parts() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let c = slide.object("circle", circle());
    slide.add(&[&c]);
    slide.play(vec![
        Anim::prim("Transform")
            .arg("mobject", &c)
            .arg("target_mobject", Object::new("Square")),
    ]);
    let checked = run(&deck);
    assert_eq!(checked.diagnostics, []);
    assert_eq!(&*checked.slides[0].exit[0].kind, "VMobject");

    deck.add_slide("t")
        .play(vec![Anim::prim("Indicate").arg("mobject", &c.child(0))]);
    assert_eq!(codes(&deck), ["sealed"]);
}

#[test]
fn an_unnamed_object_can_only_be_read() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let c = slide.object("circle", circle());
    slide.add(&[&c]);
    slide.play(vec![
        Anim::prim("ReplacementTransform")
            .arg("mobject", &c)
            .arg("target_mobject", Object::new("Square")),
    ]);
    assert_eq!(codes(&deck), ["unnamed"]);
}

#[test]
fn grouping_moves_its_operands() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let a = slide.object("a", circle());
    let b = slide.object("b", Object::new("Square"));
    let row = slide.object(
        "row",
        Object::vgroup(&[&a, &b]).then(Call::new("arrange").arg("direction", Expr::konst("RIGHT"))),
    );
    slide.play(vec![fade_in(&row)]);
    // The old name still works, and now means a place inside the group.
    slide.play(vec![Anim::prim("Indicate").arg("mobject", &a)]);
    assert_eq!(codes(&deck), [] as [&str; 0]);

    // …so the group and its member cannot be animated together,
    deck.add_slide("t").play(vec![
        Anim::prim("Indicate").arg("mobject", &a),
        Anim::animate(&row, vec![shift("UP")]),
    ]);
    assert_eq!(codes(&deck), ["overlap"]);
    deck.slides.pop();
    // an object cannot join a second group,
    deck.add_slide("u").object("again", Object::group(&[&a]));
    assert_eq!(codes(&deck), ["group", "empty-slide"]);
    deck.slides.pop();
    // and a group that is only partly on scene cannot be faded out as a whole.
    let mut slide = deck.add_slide("v");
    slide.remove(&[&row.child(1)]);
    slide.play(vec![fade_out(&row)]);
    assert_eq!(codes(&deck), ["presence"]);
    let checked = run(&deck);
    assert!(
        checked.diagnostics[0]
            .message
            .contains("Only part of “row”")
    );
    // Putting the member back makes the group whole again.
    deck.slides.pop();
    let mut slide = deck.add_slide("w");
    slide.remove(&[&b]);
    slide.add(&[&row.child(1)]);
    slide.play(vec![fade_out(&row)]);
    assert_eq!(codes(&deck), [] as [&str; 0]);
}

#[test]
fn group_members_must_agree_and_fit() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let a = slide.object("a", circle());
    let b = slide.object("b", Object::new("Square"));
    let image = slide.object(
        "photo",
        Object::new("ImageMobject").arg("filename_or_array", "p.png"),
    );
    slide.add(&[&a]);
    slide.object("mixed", Object::group(&[&a, &b]));
    slide.object("vector", Object::vgroup(&[&image]));
    slide.wait(1.0);
    assert_eq!(codes(&deck), ["group", "kind"]);
}

#[test]
fn a_sequence_sees_what_its_earlier_steps_did() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let c = slide.object("circle", circle());
    slide.play(vec![Anim::seq(vec![
        fade_in(&c),
        Anim::prim("Indicate").arg("mobject", &c),
        fade_out(&c).run_time(0.5),
    ])]);
    let checked = run(&deck);
    assert_eq!(checked.diagnostics, []);
    assert_eq!(checked.slides[0].exit[0].state, State::Is(Presence::Off));
    assert_eq!(checked.slides[0].frames, 150);

    // In parallel, the same three would all need the circle in different states at once.
    let mut slide = deck.add_slide("t");
    slide.play(vec![Anim::par(vec![
        fade_in(&c),
        Anim::prim("Indicate").arg("mobject", &c),
    ])]);
    assert!(codes(&deck).contains(&"overlap"));
}

#[test]
fn arguments_are_typed() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    slide.object("a", circle().arg("radius", "big"));
    slide.object("b", circle().arg("diameter", 2.0));
    slide.object("c", Object::new("Text"));
    slide.object("d", Object::new("Hexagon"));
    let dot = slide.object(
        "e",
        Object::new("Dot").arg(
            "point",
            Expr::call("+", vec![Expr::konst("UP"), Expr::Real(1.0)]),
        ),
    );
    slide.apply(
        &dot,
        Call::new("set_fill")
            .arg("opacity", 0.5)
            .arg("opacity", 1.0),
    );
    slide.apply(&dot, Call::new("explode"));
    slide.wait(1.0);
    assert_eq!(
        codes(&deck),
        [
            "type",
            "unknown",
            "arguments",
            "unknown",
            "type",
            "arguments",
            "unknown"
        ]
    );
}

#[test]
fn points_objects_and_formulas_as_arguments() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let dot = slide.object("dot", Object::new("Dot"));
    let x = slide.var("x");
    // An object is accepted where a point is, a whole number where a number is.
    slide.object(
        "line",
        Object::new("Line")
            .arg("start", &dot)
            .arg("end", Expr::vec3(1.0, 2.0, 0.0))
            .arg("buff", 1_i64),
    );
    slide.object(
        "graph",
        Object::new("FunctionGraph").arg(
            "function",
            Arg::fun(x, Expr::call("sin", vec![Expr::Var(x)])),
        ),
    );
    slide.object("ring", circle().arg("radius", Expr::query(&dot, "width")));
    slide.wait(1.0);
    assert_eq!(codes(&deck), [] as [&str; 0]);

    // A formula must produce what the parameter expects: a graph needs numbers, not points.
    let mut slide = deck.add_slide("t");
    slide.object(
        "bad",
        Object::new("FunctionGraph").arg("function", Arg::fun(x, Expr::konst("UP"))),
    );
    slide.wait(1.0);
    assert_eq!(codes(&deck), ["type"]);
}

#[test]
fn slides_must_show_something() {
    let mut deck = deck();
    deck.add_slide("s").object("circle", circle());
    assert_eq!(codes(&deck), ["empty-slide"]);
    assert_eq!(codes(&Deck::new(DeckConfig::default())), ["empty-deck"]);
}

#[test]
fn a_loop_should_end_the_way_it_starts() {
    let mut deck = deck();
    let mut slide = deck.add_slide("first");
    let c = slide.object("circle", circle());
    slide.play(vec![fade_in(&c)]);
    let mut slide = deck.add_slide("loop");
    slide.config(|config| config.looping = true);
    slide.play(vec![Anim::prim("Indicate").arg("mobject", &c)]);
    deck.add_slide("last").play(vec![fade_out(&c)]);
    assert_eq!(codes(&deck), [] as [&str; 0]);

    // Fading the circle out inside the loop makes it jump back in on every round.
    let mut jumping = deck.clone();
    jumping.slides.truncate(2);
    jumping
        .add_slide("loop")
        .config(|config| config.looping = true)
        .play(vec![fade_out(&c)]);
    jumping.add_slide("last").wait(1.0);
    let checked = run(&jumping);
    let codes: Vec<_> = checked
        .diagnostics
        .iter()
        .map(|d| (d.code, d.severity))
        .collect();
    assert_eq!(codes, [("loop-jump", Severity::Warning)]);
}

#[test]
fn animate_rotate_is_flagged() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let s = slide.object("square", Object::new("Square"));
    slide.add(&[&s]);
    slide.play(vec![Anim::animate(
        &s,
        vec![Call::new("rotate").arg("angle", Expr::konst("PI"))],
    )]);
    let checked = run(&deck);
    assert_eq!(checked.diagnostics.len(), 1);
    assert_eq!(
        (checked.diagnostics[0].code, checked.diagnostics[0].severity),
        ("animate-rotate", Severity::Warning)
    );
    assert!(!checked.has_errors());
}

#[test]
fn timing_follows_manim_frame_for_frame() {
    let mut deck = deck();
    deck.config.gap = Time::from_secs(0.1);
    let mut slide = deck.add_slide("s");
    let a = slide.object("a", circle());
    let b = slide.object("b", circle());
    let c = slide.object("c", circle());
    // Staggered by a quarter: 1 + 0.25 + 0.25 = 1.5 s.
    slide.play(vec![
        Anim::par(vec![fade_in(&a), fade_in(&b), fade_in(&c)]).lag(0.25),
    ]);
    // A play lasts as long as its longest animation.
    slide.play(vec![
        fade_out(&a).run_time(0.5),
        fade_out(&b).run_time(23.0 / 60.0),
    ]);
    // Still: rounds down. 23/60 s is 23 frames here, though 24 when something moves.
    slide.wait(23.0 / 60.0);
    slide.play(vec![fade_out(&c).run_time(23.0 / 60.0)]);

    let checked = run(&deck);
    assert_eq!(checked.diagnostics, []);
    let frames: Vec<u32> = slide_notes(&deck, &checked)
        .into_iter()
        .filter(|&f| f > 0)
        .collect();
    assert_eq!(frames, [90, 30, 23, 24]);
    assert_eq!(checked.slides[0].frames, 167);
    assert_eq!(checked.slides[0].gap_frames, 6);
    assert!(checked.slides[0].exact);
}

#[test]
fn updaters_keep_waits_live() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let dot = slide.object("dot", Object::new("Dot"));
    let label = slide.object("label", Object::new("Text").arg("text", "x"));
    slide.always(
        &label,
        Call::new("next_to")
            .arg("mobject_or_point", &dot)
            .arg("direction", Expr::konst("UP")),
    );
    slide.wait(23.0 / 60.0); // nothing with an updater is on scene yet: still
    slide.add(&[&dot, &label]);
    slide.wait(23.0 / 60.0); // live: rendered frame by frame
    slide.detach(&label);
    slide.wait(23.0 / 60.0);
    slide.every(&dot, |dt| {
        Call::new("shift").arg(
            "vectors",
            Expr::call("*", vec![Expr::Var(dt), Expr::konst("RIGHT")]),
        )
    });
    slide.wait(1.0);

    let checked = run(&deck);
    assert_eq!(checked.diagnostics, []);
    let frames: Vec<u32> = slide_notes(&deck, &checked)
        .into_iter()
        .filter(|&f| f > 0)
        .collect();
    assert_eq!(frames, [23, 24, 23, 60]);
}

/// Frames of each command of the first slide, in order.
fn slide_notes(deck: &Deck, checked: &Checked) -> Vec<u32> {
    let suit_core::SlideContent::Blueprint(blueprint) = &deck.slides[0].content else {
        panic!()
    };
    blueprint
        .body
        .iter()
        .map(|node| checked.notes[&node.id].frames)
        .collect()
}

#[test]
fn an_updater_should_not_outlive_what_it_follows() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let c = slide.object("circle", circle());
    let s = slide.object("square", Object::new("Square"));
    let label = slide.object("label", Object::new("Text").arg("text", "shape"));
    let follow = |target: &Place| {
        Call::new("next_to")
            .arg("mobject_or_point", target)
            .arg("direction", Expr::konst("DOWN"))
    };
    slide.always(&label, follow(&c));
    slide.add(&[&c, &label]);
    let morph = Anim::prim("ReplacementTransform")
        .arg("mobject", &c)
        .arg("target_mobject", &s);
    slide.play(vec![morph.clone()]);
    assert_eq!(codes(&deck), ["stale-updater"]);

    // Re-pointing the label before the morph is the fix the message suggests.
    let suit_core::SlideContent::Blueprint(blueprint) = &mut deck.slides[0].content else {
        panic!()
    };
    blueprint.body.pop();
    let mut slide = deck.add_slide("t");
    slide.detach(&label);
    slide.always(&label, follow(&s));
    slide.play(vec![morph]);
    assert_eq!(codes(&deck), ["empty-slide"]);
}

#[test]
fn a_change_cannot_build_on_an_earlier_one_in_the_same_sequence() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let c = slide.object("circle", circle());
    // Bringing it in first is fine: fading in does not change where the circle is.
    slide.play(vec![Anim::seq(vec![
        fade_in(&c),
        Anim::animate(&c, vec![shift("UP")]),
    ])]);
    assert_eq!(codes(&deck), [] as [&str; 0]);

    // Manim works out both targets before either runs, so "up, then down" cannot be said
    // in one sequence; neither can "turn, then move".
    let mut slide = deck.add_slide("t");
    slide.play(vec![Anim::seq(vec![
        Anim::animate(&c, vec![shift("UP")]),
        Anim::animate(&c, vec![shift("DOWN")]),
    ])]);
    slide.play(vec![Anim::seq(vec![
        Anim::prim("Rotate").arg("mobject", &c),
        Anim::animate(&c, vec![shift("UP")]),
    ])]);
    // The other order is fine: Rotate works out its target when it begins.
    slide.play(vec![Anim::seq(vec![
        Anim::animate(&c, vec![shift("UP")]),
        Anim::prim("Rotate").arg("mobject", &c),
    ])]);
    assert_eq!(codes(&deck), ["animate-order", "animate-order"]);
}

/// What is said of settings that nothing shows, with the help that comes with it; and
/// nothing in the deck is worse than a warning.
fn unseen(deck: &Deck) -> Vec<(String, Option<String>)> {
    let unseen = |d: &&suit_check::Diagnostic| d.code == "unseen";
    let diagnostics = run(deck).diagnostics;
    assert!(diagnostics.iter().all(|d| d.severity == Severity::Warning));
    let said = diagnostics.iter().filter(unseen);
    said.map(|d| (d.message.clone(), d.help.clone())).collect()
}

#[test]
fn a_fill_colour_that_nothing_shows_is_warned_of() {
    let orange = || Expr::konst("ORANGE");
    let filled = |ctor: &str| Object::new(ctor).arg("fill_color", orange());

    // A line has no inside: the colour meant is the line's own.
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let line = slide.object("line", filled("Line"));
    slide.play(vec![fade_in(&line)]);
    assert_eq!(
        unseen(&deck),
        [(
            "“line” is given a fill colour, but has no inside to fill.".to_owned(),
            Some("Use “color” instead: that colours the line itself.".to_owned())
        )]
    );

    // A circle has one, which Manim leaves fully transparent unless told otherwise.
    let mut deck = self::deck();
    let mut slide = deck.add_slide("s");
    let c = slide.object("circle", filled("Circle"));
    slide.play(vec![fade_in(&c)]);
    assert_eq!(
        unseen(&deck),
        [(
            "“circle” is given a fill colour that cannot be seen: its inside starts fully \
             transparent."
                .to_owned(),
            Some("Add the setting “fill opacity” as well.".to_owned())
        )]
    );

    // Told how much to fill, there is nothing to say: at once, straight after, or later.
    let opacity = |method: &str| Call::new(method).arg("opacity", 0.5);
    let mut deck = self::deck();
    let mut slide = deck.add_slide("s");
    let at_once = slide.object("circle", filled("Circle").arg("fill_opacity", 0.5));
    let after = slide.object("square", filled("Square").then(opacity("set_fill")));
    let later = slide.object("star", filled("Star"));
    let never = slide.object("triangle", filled("Triangle"));
    slide.play(vec![
        fade_in(&at_once),
        fade_in(&after),
        fade_in(&later),
        fade_in(&never),
    ]);
    slide.play(vec![Anim::animate(&later, vec![opacity("set_opacity")])]);
    // Recolouring the inside does not show it.
    let recolour = Call::new("set_fill").arg("color", Expr::konst("RED"));
    slide.play(vec![Anim::animate(&never, vec![recolour])]);
    let said = unseen(&deck);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].0.starts_with("“triangle”"), "{said:?}");

    // What starts filled shows its fill colour as it is.
    let mut deck = self::deck();
    let mut slide = deck.add_slide("s");
    let dot = slide.object("dot", filled("Dot"));
    let text = slide.object("text", filled("Text").arg("text", "a"));
    slide.play(vec![fade_in(&dot), fade_in(&text)]);
    assert_eq!(codes(&deck), [] as [&str; 0]);
}

#[test]
fn a_look_given_to_what_is_made_of_parts_is_warned_of() {
    let orange = || Expr::konst("ORANGE");
    let alone = |name: &str, made: Object| {
        let mut deck = deck();
        let mut slide = deck.add_slide("s");
        let object = slide.object(name, made);
        slide.play(vec![fade_in(&object)]);
        deck
    };

    // Manim hands the colour of axes to the group they are, and makes their lines apart
    // from it. A setting of the axes' own is another matter.
    let axes = Object::new("Axes").arg("tips", false);
    assert_eq!(
        unseen(&alone("axes", axes.clone().arg("color", orange()))),
        [(
            "“axes” is given “color”, which shows nothing: Manim makes its parts \
             separately, and they get none of it."
                .to_owned(),
            Some("Use “set color” instead: that reaches every part.".to_owned())
        )]
    );

    // So it is with the outline, on a grid as on axes. The calls are named in the order
    // to make them in: the whole first, so that what is said of the outline stays.
    let grid = Object::new("NumberPlane")
        .arg("stroke_width", 6.0)
        .arg("color", orange())
        .arg("stroke_opacity", 0.5);
    assert_eq!(
        unseen(&alone("grid", grid)),
        [(
            "“grid” is given “stroke width”, “color” and “stroke opacity”, which show \
             nothing: Manim makes its parts separately, and they get none of these."
                .to_owned(),
            Some("Use “set color” and “set stroke” instead: those reach every part.".to_owned())
        )]
    );

    // A fill is said once, as one of them, and with an opacity as without. There is no
    // call to offer for it: the parts are lines.
    let filled = axes
        .clone()
        .arg("fill_color", orange())
        .arg("fill_opacity", 1.0);
    assert_eq!(
        unseen(&alone("axes", filled)),
        [(
            "“axes” is given “fill color” and “fill opacity”, which show nothing: Manim \
             makes its parts separately, and they get none of these."
                .to_owned(),
            None
        )]
    );

    // Done to the finished axes, it reaches every part, and there is nothing to say.
    let width = Call::new("set_stroke").arg("width", 6.0);
    let after = axes
        .then(Call::new("set_color").arg("color", orange()))
        .then(width);
    assert_eq!(codes(&alone("axes", after)), [] as [&str; 0]);

    // A number line has parts as well, but is itself a line: they take its colour.
    let line = Object::new("NumberLine").arg("color", orange());
    assert_eq!(codes(&alone("number line", line)), [] as [&str; 0]);
}

#[test]
fn a_warning_that_waited_keeps_its_place_among_the_others() {
    let mut deck = deck();
    let mut slide = deck.add_slide("s");
    let s = slide.object("square", Object::new("Square"));
    slide.add(&[&s]);
    let turn = Call::new("rotate").arg("angle", 1.0);
    slide.play(vec![Anim::animate(&s, vec![turn.clone()])]);
    let c = slide.object("circle", circle().arg("fill_color", Expr::konst("ORANGE")));
    slide.play(vec![fade_in(&c)]);
    slide.play(vec![Anim::animate(&s, vec![turn])]);
    let codes = codes(&deck);
    let first = codes.iter().position(|code| *code == "unseen");
    assert_eq!(first, Some(codes.len() / 2), "{codes:?}");
    assert_eq!(codes.len(), 3, "{codes:?}");
}
