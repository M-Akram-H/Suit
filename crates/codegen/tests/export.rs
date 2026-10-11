//! The exporter against a stored snapshot. After an intended change, regenerate it with
//! `UPDATE_SNAPSHOTS=1 cargo test -p suit-codegen` and review the diff.

use std::{env, fs, path::PathBuf};

use suit_check::check;
use suit_codegen::{Options, export};
use suit_core::{
    Anim, Call, Deck, DeckConfig, Direction, Expr, Object, Place, Slide, SlideConfig, SlideContent,
    SlideId,
};
use suit_sig::Signature;

fn shift(by: &str) -> Call {
    Call::new("shift").arg("vectors", Expr::konst(by))
}

fn deck() -> Deck {
    let mut deck = Deck::new(DeckConfig {
        width: 1280,
        height: 720,
        fps: 30,
        ..DeckConfig::default()
    });

    let mut slide = deck.add_slide("One");
    let circle = slide.object("circle", Object::new("Circle").arg("radius", 1.5));
    let label = slide.object("the label", Object::new("Text").arg("text", "r = \"1.5\""));
    let above = |target: &Place| {
        Call::new("next_to")
            .arg("mobject_or_point", target)
            .arg("direction", Expr::konst("UP"))
    };
    slide.always(&label, above(&circle));
    slide.play(vec![
        Anim::prim("Create").arg("mobject", &circle),
        Anim::prim("FadeIn")
            .arg("mobject", &label)
            .arg("shift", Expr::konst("DOWN"))
            .rate("ease_out_cubic"),
    ]);
    slide.wait(0.5);

    let mut slide = deck.add_slide("Two");
    slide.config(|config| {
        config.auto_next = true;
        config.notes = "Say \"hello\".\nThen move on.".to_owned();
    });
    // `start` is left out, so `end` has to be passed by name.
    let line = slide.object(
        "line",
        Object::new("Line").arg("end", Expr::vec3(1.0, 1.0, 0.0)),
    );
    slide.add(&[&line]);
    let pair = slide.object("pair", Object::group(&[&circle, &line]));
    slide.object("twin", Object::copy(&pair));
    slide.play(vec![
        Anim::seq(vec![
            Anim::animate(&pair, vec![shift("UP")]),
            Anim::prim("Rotate").arg("mobject", &pair),
        ])
        .run_time(2.0),
    ]);
    slide.every(&line, |dt| {
        let step = Expr::call("*", vec![Expr::Var(dt), Expr::konst("RIGHT")]);
        Call::new("shift").arg("vectors", step)
    });
    slide.play(vec![
        Anim::par(vec![
            Anim::prim("Indicate").arg("mobject", &circle),
            Anim::prim("Circumscribe")
                .arg("mobject", &label)
                .arg("color", Expr::konst("RED")),
        ])
        .lag(0.5),
    ]);

    let id = SlideId(deck.fresh_id());
    deck.slides.push(Slide {
        id,
        title: "Clip".to_owned(),
        config: SlideConfig {
            direction: Direction::Vertical,
            ..SlideConfig::default()
        },
        content: SlideContent::Media {
            path: "media/clip.mp4".to_owned(),
        },
    });
    deck
}

#[test]
fn export_matches_the_snapshot() {
    let sig = Signature::builtin();
    let deck = deck();
    let checked = check(&deck, &sig);
    assert_eq!(checked.diagnostics, []);
    let python = export(&deck, &sig, &checked, &Options::default());

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/small.py");
    if env::var_os("UPDATE_SNAPSHOTS").is_some() {
        fs::write(&path, &python).unwrap();
    }
    let expected = fs::read_to_string(&path).unwrap_or_default();
    assert!(
        python == expected,
        "export changed; expected:\n{expected}\ngot:\n{python}"
    );
}

#[test]
fn a_preview_height_keeps_the_aspect_ratio_and_the_timing() {
    let sig = Signature::builtin();
    let deck = deck();
    let checked = check(&deck, &sig);
    let options = Options {
        pixel_height: Some(270),
        ..Options::default()
    };
    let python = export(&deck, &sig, &checked, &options);
    assert!(
        python.contains(
            "config.pixel_width = 480\nconfig.pixel_height = 270\nconfig.frame_rate = 30\n"
        )
    );
}

#[test]
fn pictures_named_from_the_deck_are_found_from_the_file() {
    let sig = Signature::builtin();
    let picture = |file: &str| {
        let mut deck = Deck::new(DeckConfig::default());
        let mut slide = deck.add_slide("One");
        let photo = slide.object(
            "photo",
            Object::new("ImageMobject").arg("filename_or_array", file),
        );
        slide.play(vec![Anim::prim("FadeIn").arg("mobject", &photo)]);
        let checked = check(&deck, &sig);
        assert_eq!(checked.diagnostics, []);
        export(&deck, &sig, &checked, &Options::default())
    };
    let beside = picture("pictures/photo.png");
    assert!(beside.contains("from pathlib import Path\n"));
    assert!(beside.contains("config.assets_dir = str(Path(__file__).resolve().parent)\n"));
    assert!(beside.contains("ImageMobject(\"pictures/photo.png\")"));
    // A picture named in full needs no such help.
    assert!(!picture("/somewhere/photo.png").contains("assets_dir"));
}

#[test]
fn a_graph_brings_its_definition_along() {
    let sig = Signature::builtin();
    let mut deck = Deck::new(DeckConfig::default());
    let mut slide = deck.add_slide("One");
    let x = slide.var("x");
    let rule = suit_core::Arg::fun(x, Expr::call("/", vec![Expr::Real(1.0), Expr::Var(x)]));
    let graph = slide.object(
        "graph",
        Object::new("Plot")
            .arg("function", rule)
            .arg("on", Expr::vec3(-1.0, -0.1, 0.0))
            .arg("on", Expr::vec3(0.1, 1.0, 0.0))
            .arg("asymptotes", true),
    );
    slide.play(vec![Anim::prim("Create").arg("mobject", &graph)]);
    let checked = check(&deck, &sig);
    assert_eq!(checked.diagnostics, []);
    let python = export(&deck, &sig, &checked, &Options::default());
    // What Manim does not have is defined in the file itself, once, before it is used.
    assert_eq!(python.matches("class Plot(VMobject):").count(), 1);
    assert!(python.find("class Plot(VMobject):") < python.find("class Deck(Slide):"));
    let made = "Plot(lambda x_2: (1.0 / x_2), on=[np.array([-1.0, -0.1, 0.0]), \
                np.array([0.1, 1.0, 0.0])], asymptotes=True)";
    assert!(python.contains(made), "{python}");

    // A deck without a graph does not carry it.
    let mut plain = Deck::new(DeckConfig::default());
    let mut slide = plain.add_slide("One");
    let circle = slide.object("circle", Object::new("Circle"));
    slide.play(vec![Anim::prim("Create").arg("mobject", &circle)]);
    let checked = check(&plain, &sig);
    assert!(!export(&plain, &sig, &checked, &Options::default()).contains("class Plot"));
}

#[test]
fn a_tangent_brings_its_definition_and_its_animation_along() {
    let sig = Signature::builtin();
    let mut deck = Deck::new(DeckConfig::default());
    let mut slide = deck.add_slide("One");
    let x = slide.var("x");
    let rule = suit_core::Arg::fun(x, Expr::call("sin", vec![Expr::Var(x)]));
    let graph = slide.object("graph", Object::new("Plot").arg("function", rule));
    let tangent = slide.object(
        "tangent",
        Object::new("Tangent")
            .arg("graph", &graph)
            .arg("at", 0.5)
            .arg("dot", true),
    );
    slide.play(vec![Anim::prim("Create").arg("mobject", &graph)]);
    slide.play(vec![Anim::prim("Create").arg("mobject", &tangent)]);
    slide.play(vec![
        Anim::prim("Glide")
            .arg("mobject", &tangent)
            .arg("to", 3.0)
            .run_time(2.0),
    ]);
    slide.apply(&tangent, Call::new("touch").arg("x", 1.0));
    let checked = check(&deck, &sig);
    assert_eq!(checked.diagnostics, []);
    let python = export(&deck, &sig, &checked, &Options::default());
    for class in [
        "class Plot(VMobject):",
        "class Tangent(Line):",
        "class Glide(Animation):",
    ] {
        assert_eq!(python.matches(class).count(), 1, "{class}");
        assert!(
            python.find(class) < python.find("class Deck(Slide):"),
            "{class}"
        );
    }
    assert!(
        python.contains("tangent_6 = Tangent(graph_3, at=0.5, dot=True)"),
        "{python}"
    );
    assert!(python.contains("self.play(Glide(tangent_6, 3.0, run_time=2.0))"));
    assert!(python.contains("tangent_6.touch(1.0)"));

    // A tangent to a circle is no tangent of this kind.
    let mut wrong = Deck::new(DeckConfig::default());
    let mut slide = wrong.add_slide("One");
    let circle = slide.object("circle", Object::new("Circle"));
    let line = slide.object("line", Object::new("Tangent").arg("graph", &circle));
    slide.play(vec![Anim::prim("Create").arg("mobject", &line)]);
    assert_eq!(check(&wrong, &sig).diagnostics.len(), 1);
}
