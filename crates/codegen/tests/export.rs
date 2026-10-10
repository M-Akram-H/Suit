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
