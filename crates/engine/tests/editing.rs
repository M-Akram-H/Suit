//! The engine without a preview: edits, selection and the undo history.

use suit_core::{Anim, Call, Command, Expr, Object, Place, SlideContent, Time};
use suit_engine::Session;

fn codes(session: &Session) -> Vec<&'static str> {
    session.checked.diagnostics.iter().map(|d| d.code).collect()
}

fn headlines(session: &Session) -> Vec<String> {
    let pretty = suit_codegen::pretty::Pretty {
        deck: &session.deck,
        sig: &session.sig,
    };
    session
        .steps()
        .iter()
        .map(|node| pretty.headline(&node.item))
        .collect()
}

#[test]
fn a_blank_deck_has_one_empty_slide() {
    let session = Session::blank();
    assert_eq!(session.deck.slides.len(), 1);
    assert_eq!(codes(&session), ["empty-slide"]);
    assert!(!session.is_dirty());
    assert_eq!(session.can_undo(), None);
}

#[test]
fn adding_an_object_makes_it_and_brings_it_in() {
    let mut session = Session::blank();
    let circle = session.add_object("Circle").unwrap();
    assert_eq!(
        headlines(&session),
        ["New circle (Circle)", "Create circle"]
    );
    assert_eq!(
        session.step,
        Some(1),
        "the entrance is selected, so the canvas shows the object"
    );
    assert_eq!(codes(&session), [] as [&str; 0]);
    assert!(session.is_dirty());

    // Text is written rather than drawn, an image can only fade in, and names stay unique.
    session.add_object("Text").unwrap();
    session.add_object("Circle").unwrap();
    assert_eq!(
        headlines(&session)[2..],
        [
            "New text (Text)",
            "Write text",
            "New circle 2 (Circle)",
            "Create circle 2"
        ]
    );
    assert_eq!(codes(&session), [] as [&str; 0]);
    assert_eq!(session.creation(circle), Some((0, 0)));

    // Each addition is one undo step, however many steps it made.
    session.undo();
    session.undo();
    assert_eq!(headlines(&session).len(), 2);
    session.redo();
    assert_eq!(headlines(&session).len(), 4);
    assert_eq!(session.can_undo(), Some("Add object"));
}

#[test]
fn objects_are_inserted_after_the_selected_step() {
    let mut session = Session::blank();
    session.add_object("Circle");
    session.select(0, None);
    session.add_object("Square");
    assert_eq!(
        headlines(&session),
        [
            "New square (Square)",
            "Create square",
            "New circle (Circle)",
            "Create circle"
        ]
    );
}

#[test]
fn steps_can_be_added_moved_and_deleted() {
    let mut session = Session::blank();
    let circle = Place::var(session.add_object("Circle").unwrap());
    session.insert_step(Command::Wait {
        duration: Time::from_secs(0.5),
    });
    let indicate = Anim::prim("Indicate").arg("mobject", &circle);
    session.insert_step(Command::Play {
        anims: vec![suit_core::Node::unset(indicate)],
    });
    assert_eq!(headlines(&session)[2..], ["Pause 0.5 s", "Indicate circle"]);
    assert_eq!(session.step, Some(3));

    session.move_step(false);
    assert_eq!(headlines(&session)[2..], ["Indicate circle", "Pause 0.5 s"]);
    assert_eq!(session.step, Some(2), "the selection follows the step");
    session.move_step(false);
    session.move_step(false);
    session.move_step(false);
    assert_eq!(session.step, Some(0), "and stops at the top");
    // Indicating the circle before it exists is a mistake the checker reports.
    assert_eq!(codes(&session), ["unknown"]);

    session.delete_step();
    assert_eq!(session.step, None);
    assert_eq!(
        headlines(&session),
        ["New circle (Circle)", "Create circle", "Pause 0.5 s"]
    );
    assert_eq!(codes(&session), [] as [&str; 0]);
}

#[test]
fn rapid_changes_to_one_step_are_one_undo_step() {
    let mut session = Session::blank();
    session.add_object("Circle");
    let id = session.steps()[0].id;
    let var = session.creation_var(0);
    for radius in [1.2, 1.4, 1.6] {
        let object = Object::new("Circle").arg("radius", radius);
        let command = Command::LetObject {
            var,
            value: suit_core::Node::unset(object),
        };
        session.replace_step(id, command);
    }
    assert_eq!(
        session.steps()[0].id,
        id,
        "a changed step keeps its identity"
    );
    let pretty = suit_codegen::pretty::Pretty {
        deck: &session.deck,
        sig: &session.sig,
    };
    assert_eq!(
        pretty.command(&session.steps()[0].item),
        "circle = Circle(radius = 1.6)"
    );

    session.undo();
    let pretty = suit_codegen::pretty::Pretty {
        deck: &session.deck,
        sig: &session.sig,
    };
    assert_eq!(
        pretty.command(&session.steps()[0].item),
        "circle = Circle()"
    );
    assert_eq!(session.can_undo(), Some("Add object"));
}

#[test]
fn dragging_at_creation_replaces_the_placement() {
    let mut session = Session::blank();
    let title = session.add_object("Text").unwrap();
    let circle = session.add_object("Circle").unwrap();
    let (_, step) = session.creation(circle).unwrap();
    let id = session.steps()[step].id;
    let below = Call::new("next_to")
        .arg("mobject_or_point", &Place::var(title))
        .arg("direction", Expr::konst("DOWN"));
    let object = Object::new("Circle")
        .then(Call::new("scale").arg("scale_factor", 0.5))
        .then(below);
    session.replace_step(
        id,
        Command::LetObject {
            var: circle,
            value: suit_core::Node::unset(object),
        },
    );

    assert!(session.place_at_creation(circle, [2.004, -1.5]));
    let pretty = suit_codegen::pretty::Pretty {
        deck: &session.deck,
        sig: &session.sig,
    };
    // The scaling stays; where it was put is replaced, rounded to hundredths.
    assert_eq!(
        pretty.command(&session.steps()[step].item),
        "circle = Circle().scale(0.5).move_to(⟨2, -1.5, 0⟩)"
    );
    assert!(session.place_at_creation(circle, [0.0, 3.0]));
    let pretty = suit_codegen::pretty::Pretty {
        deck: &session.deck,
        sig: &session.sig,
    };
    assert_eq!(
        pretty.command(&session.steps()[step].item),
        "circle = Circle().scale(0.5).move_to(⟨0, 3, 0⟩)"
    );
    assert_eq!(codes(&session), [] as [&str; 0]);
}

#[test]
fn a_later_move_is_a_step_of_its_own() {
    let mut session = Session::blank();
    let circle = session.add_object("Circle").unwrap();
    session.insert_move(circle, [1.0, 1.0], true);
    session.insert_move(circle, [0.0, 0.0], false);
    assert_eq!(
        headlines(&session)[2..],
        ["Change circle: move_to", "Change circle: move_to"]
    );
    let frames = |step: usize| session.checked.notes[&session.steps()[step].id].frames;
    assert_eq!(
        (frames(2), frames(3)),
        (60, 0),
        "gliding takes a second, jumping no time"
    );
    assert_eq!(codes(&session), [] as [&str; 0]);
}

#[test]
fn slides_can_be_added_moved_and_deleted() {
    let mut session = Session::blank();
    session.add_object("Circle");
    session.add_slide();
    assert_eq!((session.slide, session.step), (1, None));
    session.edit_slide(|slide| slide.title = "Second".to_owned());
    session.edit_slide(|slide| slide.config.looping = true);
    let titles = |session: &Session| -> Vec<String> {
        session
            .deck
            .slides
            .iter()
            .map(|slide| slide.title.clone())
            .collect()
    };
    assert_eq!(titles(&session), ["Slide 1", "Second"]);

    session.move_slide(false);
    assert_eq!(titles(&session), ["Second", "Slide 1"]);
    assert_eq!(session.slide, 0);
    session.move_slide(false);
    assert_eq!(session.slide, 0, "the first slide cannot move earlier");

    session.delete_slide();
    assert_eq!(titles(&session), ["Slide 1"]);
    session.delete_slide();
    assert_eq!(titles(&session), ["Slide 1"], "the last slide stays");

    // Title and loop setting were changed in one go, so one undo takes both back.
    session.undo();
    session.undo();
    session.undo();
    assert_eq!(titles(&session), ["Slide 1", "Slide 2"]);
    assert!(
        matches!(&session.deck.slides[1].content, SlideContent::Blueprint(b) if b.body.is_empty())
    );
}

#[test]
fn objects_at_the_selection() {
    let mut session = Session::blank();
    let circle = session.add_object("Circle").unwrap();
    let on = |session: &Session| -> Vec<bool> {
        session
            .objects()
            .iter()
            .map(|o| o.state == suit_check::State::Is(suit_core::Presence::On))
            .collect()
    };
    assert_eq!(on(&session), [true]);
    session.select(0, Some(0));
    assert_eq!(on(&session), [false], "made, but not yet on the slide");
    assert_eq!(session.objects()[0].var, circle);
    session.select(0, None);
    assert_eq!(
        on(&session),
        [] as [bool; 0],
        "nothing exists when the slide starts"
    );
}

#[test]
fn saving_and_opening_round_trips() {
    let mut session = Session::blank();
    session.add_object("Star");
    assert_eq!(session.save(), Err("This deck has no file yet.".to_owned()));
    let path = std::env::temp_dir().join(format!("suit-test-{}.suit.json", std::process::id()));
    session.save_as(&path).unwrap();
    assert!(!session.is_dirty());
    session.rename(session.creation_var(0), "sun");
    assert!(session.is_dirty());
    session.save().unwrap();

    let opened = Session::open(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(opened.deck, session.deck);
    assert_eq!(headlines(&opened), ["New sun (Star)", "Create sun"]);
    assert_eq!(opened.title(), path.file_name().unwrap().to_string_lossy());
}

#[test]
fn the_sample_deck_is_sound() {
    let session = Session::sample();
    assert_eq!(session.deck.slides.len(), 6);
    assert_eq!(codes(&session), [] as [&str; 0]);
}

/// The variable made by step `step` of the first slide.
trait CreationVar {
    fn creation_var(&self, step: usize) -> suit_core::VarId;
}

impl CreationVar for Session {
    fn creation_var(&self, step: usize) -> suit_core::VarId {
        match &self.steps()[step].item {
            Command::LetObject { var, .. } => *var,
            other => panic!("step {step} makes no object: {other:?}"),
        }
    }
}
