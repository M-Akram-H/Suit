//! Runs real Manim, so these are ignored unless asked for:
//! `cargo test -p suit-render -- --ignored`

use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};

use suit_check::{Diagnostic, Severity, check};
use suit_core::{Anim, Call, Command, Deck, DeckConfig, Expr, Object, Raw, RawIface, Time};
use suit_render::{Drawn, Failure, Prober, Sidecar, Snapshot, Watch};
use suit_sig::Signature;

fn deck(text: &str) -> Deck {
    let mut deck = Deck::new(DeckConfig::default());
    let mut slide = deck.add_slide("One");
    let circle = slide.object("circle", Object::new("Circle"));
    let label = slide.object("label", Object::new("Text").arg("text", text));
    slide.play(vec![Anim::prim("Create").arg("mobject", &circle)]);
    slide.play(vec![Anim::prim("Write").arg("vmobject", &label)]);
    deck
}

fn prober() -> Prober {
    Prober::new(Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR"))))
}

#[test]
#[ignore = "needs Manim"]
fn a_kept_process_answers_repeated_probes() {
    let sig = Signature::builtin();
    let mut prober = prober();
    let mut times = Vec::new();
    let mut processes = Vec::new();
    for text in ["one", "two", "three"] {
        let deck = deck(text);
        let checked = check(&deck, &sig);
        let started = Instant::now();
        let snapshots = prober
            .probe(&deck, &sig, &checked, Watch::patient())
            .expect("Manim runs");
        times.push(started.elapsed());
        processes.push(prober.process().expect("it is kept"));
        assert_eq!(snapshots.error, None);
        // The slide's start, then one snapshot per step.
        assert_eq!(snapshots.steps.len(), 5);
        let last = snapshots.steps.last().unwrap();
        assert_eq!(last.boxes.values().filter(|bounds| bounds.on).count(), 2);
        // The circle, and one outline per letter.
        assert_eq!(last.drawn.len(), 1 + text.len());
    }
    // Alone, the first takes a second or more and the others a tenth of that; next to
    // other tests they also wait their turn, so the times are only printed.
    println!("probe times: {times:?}");
    assert!(
        processes.iter().all(|process| *process == processes[0]),
        "later probes reuse the imported Manim"
    );
}

#[test]
#[ignore = "needs Manim"]
fn a_failing_step_is_reported_with_what_came_before() {
    let sig = Signature::builtin();
    let mut deck = deck("fine");
    // An image that does not exist makes Manim raise when the step runs.
    deck.add_slide("Two").object(
        "photo",
        Object::new("ImageMobject").arg("filename_or_array", "no-such-file.png"),
    );
    let checked = check(&deck, &sig);
    let snapshots = prober()
        .probe(&deck, &sig, &checked, Watch::patient())
        .expect("the probe itself runs");
    assert!(snapshots.error.is_some());
    assert_eq!(
        snapshots.steps.len(),
        6,
        "the first slide, and the start of the second"
    );
}

#[test]
#[ignore = "needs Manim"]
fn a_slide_is_recorded_frame_for_frame() {
    let sig = Signature::builtin();
    let mut deck = deck("abc");
    let mut slide = deck.add_slide("Two");
    let dot = slide.object("dot", Object::new("Dot"));
    slide.add(&[&dot]);
    // 23/60 s is the duration that renders one frame long when something moves (spec §5.7).
    slide.play(vec![
        Anim::prim("Indicate")
            .arg("mobject", &dot)
            .run_time(23.0 / 60.0),
    ]);
    slide.wait(0.5);
    let checked = check(&deck, &sig);
    assert_eq!(checked.diagnostics, []);

    let mut prober = prober();
    for (slide, report) in checked.slides.iter().enumerate() {
        let clip = prober
            .probe_frames(&deck, &sig, &checked, slide, Watch::patient())
            .expect("Manim runs");
        assert_eq!(clip.error, None);
        // Manim plays exactly as many frames as the checker says it will.
        assert_eq!(
            clip.frames.len() as u32,
            report.frames + report.gap_frames,
            "slide {slide}"
        );
    }

    let clip = prober
        .probe_frames(&deck, &sig, &checked, 1, Watch::patient())
        .unwrap();
    let ends: Vec<u32> = clip.marks.iter().map(|(_, end)| *end).collect();
    assert_eq!(
        ends,
        [0, 0, 24, 54],
        "made, shown, indicated for 24 frames, held for 30"
    );
    // What the first slide left behind is there from the first frame: a circle and three letters.
    assert_eq!(clip.frame(0).count(), 5);
    // The hold repeats one frame, and takes no room beyond that.
    assert!(
        clip.frames[24..54]
            .iter()
            .all(|frame| *frame == clip.frames[24])
    );
    // While it is indicated the dot is drawn differently from when it rests.
    assert_ne!(clip.frames[12], clip.frames[24]);
}

#[test]
#[ignore = "needs Manim"]
fn two_manims_take_turns_in_the_shared_directory() {
    // Manim rewrites a cached text file every time it uses it, so two of them making the
    // same text at the same moment would read each other's half-written files.
    let sig = Signature::builtin();
    let mut deck = Deck::new(DeckConfig::default());
    let mut slide = deck.add_slide("One");
    for line in 0..8 {
        let text = format!("line {line}, the same in both");
        let label = slide.object(
            &format!("label{line}"),
            Object::new("Text").arg("text", text.as_str()),
        );
        slide.add(&[&label]);
    }
    slide.wait(1.0);
    let checked = check(&deck, &sig);
    assert_eq!(checked.diagnostics, []);

    let problems = || {
        let mut prober = prober();
        let runs = (0..20).map(|_| prober.probe(&deck, &sig, &checked, Watch::patient()));
        runs.filter_map(|result| match result {
            Ok(snapshots) => snapshots.error,
            Err(failure) => Some(failure.to_string()),
        })
        .collect::<Vec<String>>()
    };
    let (here, there) = thread::scope(|scope| {
        let there = scope.spawn(problems);
        (problems(), there.join().unwrap())
    });
    assert_eq!((here, there), (Vec::new(), Vec::new()));
}

/// A slide that takes Manim a good while to play through: sixty seconds of something moving.
fn a_long_slide() -> Deck {
    let mut deck = deck("slow");
    let mut slide = deck.add_slide("Long");
    let dot = slide.object("dot", Object::new("Dot"));
    slide.add(&[&dot]);
    slide.play(vec![
        Anim::prim("Indicate").arg("mobject", &dot).run_time(60.0),
    ]);
    deck
}

#[test]
#[ignore = "needs Manim"]
fn a_run_nobody_wants_is_stopped_and_the_process_kept() {
    let sig = Signature::builtin();
    let deck = a_long_slide();
    let checked = check(&deck, &sig);
    let mut prober = prober();
    prober
        .probe(&deck, &sig, &checked, Watch::patient())
        .expect("Manim runs");
    let process = prober.process();

    // Wanted for a fifth of a second only.
    let started = Instant::now();
    let briefly = || started.elapsed() < Duration::from_millis(200);
    let watch = Watch {
        wanted: &briefly,
        ..Watch::patient()
    };
    let outcome = prober.probe_frames(&deck, &sig, &checked, 1, watch);
    assert_eq!(outcome.err(), Some(Failure::Abandoned));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "it stops between two frames, not at the end: {:?}",
        started.elapsed()
    );
    assert_eq!(prober.process(), process, "stopped by asking, not by force");

    // The same goes for running out of time.
    let limit = Duration::from_millis(200);
    let watch = Watch {
        limit,
        ..Watch::patient()
    };
    let outcome = prober.probe_frames(&deck, &sig, &checked, 1, watch);
    assert_eq!(outcome.err(), Some(Failure::TooSlow(limit)));
    assert_eq!(prober.process(), process);
    // And it is none the worse for it.
    let snapshots = prober.probe(&deck, &sig, &checked, Watch::patient());
    assert_eq!(snapshots.expect("Manim runs").error, None);
}

#[test]
#[ignore = "needs Manim"]
fn a_manim_that_hangs_is_replaced() {
    let sig = Signature::builtin();
    let mut deck = deck("stuck");
    // Python that never gets to the next step, so asking it to stop is not heard. It
    // leaves a mark first: with other tests running it may wait a while for its turn, and
    // only once it has started is there anything stuck to give up on.
    let mark = Path::new(env!("CARGO_TARGET_TMPDIR")).join("stuck.mark");
    let _ = std::fs::remove_file(&mark);
    let python = format!(
        "open({:?}, \"w\").write(\"here\")\nimport time\ntime.sleep(600)",
        mark.display().to_string()
    );
    let iface = RawIface {
        duration: Time::from_secs(1.0),
        ..RawIface::default()
    };
    deck.add_slide("Stuck")
        .push(Command::Raw(Raw { python, iface }));
    let checked = check(&deck, &sig);
    let mut prober = prober();
    let healthy = self::deck("fine");
    let healthy_checked = check(&healthy, &sig);
    prober
        .probe(&healthy, &sig, &healthy_checked, Watch::patient())
        .expect("Manim runs");
    let process = prober.process();

    // Wanted until it has been stuck for a third of a second.
    let stuck_since = std::cell::Cell::new(None::<Instant>);
    let wanted = || {
        if stuck_since.get().is_none() && mark.exists() {
            stuck_since.set(Some(Instant::now()));
        }
        stuck_since
            .get()
            .is_none_or(|since| since.elapsed() < Duration::from_millis(300))
    };
    let watch = Watch {
        wanted: &wanted,
        ..Watch::patient()
    };
    let outcome = prober.probe(&deck, &sig, &checked, watch);
    assert_eq!(outcome.err(), Some(Failure::Abandoned));
    let stuck_for = stuck_since.get().expect("it got to run").elapsed();
    assert!(stuck_for < Duration::from_secs(8), "{stuck_for:?}");
    assert_eq!(prober.process(), None, "it had to be stopped by force");

    let snapshots = prober.probe(&healthy, &sig, &healthy_checked, Watch::patient());
    assert_eq!(snapshots.expect("a new Manim runs").error, None);
    assert_ne!(prober.process(), process);
}

/// A deck of `count` slides with a line of text each; `changed` gets another line. The
/// first slide also leaves a mark in `log` every time Manim runs it.
fn slides(count: usize, changed: Option<usize>, log: &Path) -> Deck {
    let mut deck = Deck::new(DeckConfig::default());
    for index in 0..count {
        let mut slide = deck.add_slide("A slide");
        if index == 0 {
            let python = format!(
                "open({:?}, \"a\").write(\"ran\\n\")",
                log.display().to_string()
            );
            slide.push(Command::Raw(Raw {
                python,
                iface: RawIface::default(),
            }));
        }
        let text = match changed == Some(index) {
            true => format!("slide {index}, changed"),
            false => format!("slide {index}"),
        };
        let line =
            Object::new("Text")
                .arg("text", text.as_str())
                .then(suit_core::Call::new("shift").arg(
                    "vectors",
                    suit_core::Expr::vec3(0.0, index as f64 - 2.0, 0.0),
                ));
        let line = slide.object(&format!("line {index}"), line);
        slide.play(vec![
            Anim::prim("Write").arg("vmobject", &line).run_time(0.5),
        ]);
    }
    deck
}

/// How often Manim has run the first slide.
fn runs(log: &Path) -> usize {
    std::fs::read_to_string(log).map_or(0, |log| log.lines().count())
}

#[test]
#[ignore = "needs Manim"]
fn a_changed_slide_is_run_from_what_the_slides_before_it_left() {
    let sig = Signature::builtin();
    let log = Path::new(env!("CARGO_TARGET_TMPDIR")).join("first-slide-runs.log");
    let _ = std::fs::remove_file(&log);
    let mut prober = prober();
    let shown = |snapshots: &suit_render::Snapshots, from: usize| {
        let steps = snapshots.steps.iter().filter(|step| step.slide >= from);
        format!("{:?}", steps.collect::<Vec<_>>())
    };

    let before = slides(5, None, &log);
    let checked = check(&before, &sig);
    let whole = prober
        .probe(&before, &sig, &checked, Watch::patient())
        .expect("Manim runs");
    assert_eq!((whole.error.as_ref(), whole.first), (None, 0));
    assert_eq!(runs(&log), 1);

    // The fourth slide changes: the session has the first three, and asks from there.
    let after = slides(5, Some(3), &log);
    let checked_after = check(&after, &sig);
    let part = prober
        .probe_slides(&after, &sig, &checked_after, 3, 4, Watch::patient())
        .expect("Manim runs");
    assert_eq!((part.error.as_ref(), part.first), (None, 3));
    assert!(part.steps.iter().all(|step| step.slide >= 3));
    assert_eq!(runs(&log), 1, "the first slide was not run again");
    // Exactly what a Manim that has never seen the deck makes of those slides.
    let mut fresh = self::prober();
    let anew = fresh
        .probe(&after, &sig, &checked_after, Watch::patient())
        .expect("Manim runs");
    assert_eq!(runs(&log), 2);
    assert_eq!(shown(&part, 3), shown(&anew, 3));
    assert_ne!(shown(&part, 3), shown(&whole, 3), "the change shows");

    // A recording starts from what was left behind, too.
    let clip = prober
        .probe_frames(&after, &sig, &checked_after, 4, Watch::patient())
        .expect("Manim runs");
    let report = &checked_after.slides[4];
    assert_eq!(clip.error, None);
    assert_eq!(clip.frames.len() as u32, report.frames + report.gap_frames);
    assert_eq!(runs(&log), 2);

    // And back again: the deck as it was, from the slide where the two part.
    let back = prober
        .probe_slides(&before, &sig, &checked, 3, 4, Watch::patient())
        .expect("Manim runs");
    assert_eq!(shown(&back, 3), shown(&whole, 3));
    assert_eq!(runs(&log), 2);

    // A change to the first slide leaves nothing to start from but the empty scene.
    let other = slides(5, Some(0), &log);
    let checked_other = check(&other, &sig);
    let all = prober
        .probe(&other, &sig, &checked_other, Watch::patient())
        .expect("Manim runs");
    assert_eq!(all.error, None);
    assert_eq!(runs(&log), 3);
}

/// Does not need Manim: it needs a Python without it, which the system's usually is.
#[test]
fn a_python_without_manim_is_named_and_what_to_do_about_it() {
    let python = Path::new("/usr/bin/python3");
    let has = |module: &str| {
        let found = std::process::Command::new(python)
            .args(["-c", &format!("import {module}")])
            .stderr(std::process::Stdio::null())
            .status();
        found.is_ok_and(|status| status.success())
    };
    if !has("sys") || has("manim") {
        return;
    }
    let sig = Signature::builtin();
    let deck = deck("no Manim");
    let checked = check(&deck, &sig);
    let sidecar = Sidecar::locate(Path::new(".")).with_python(python.to_path_buf());
    let failure = Prober::new(sidecar)
        .probe(&deck, &sig, &checked, Watch::patient())
        .expect_err("there is no Manim to run");
    let Failure::Broken(reason) = failure else {
        panic!("{failure:?}")
    };
    assert!(
        reason.starts_with("/usr/bin/python3 has no Manim."),
        "{reason}"
    );
    assert!(reason.contains("run.sh"));
}

#[test]
#[ignore = "needs Manim"]
fn a_graph_of_a_rule_with_no_value_somewhere_is_remarked_on_not_choked_on() {
    let sig = Signature::builtin();
    // The logarithm of x from -7 to 7: no value left of zero. Manim draws none of it
    // and says nothing; the probe says so, and shows everything else.
    let graph_over = |from: f64| {
        let mut deck = deck("beside a graph");
        let mut slide = deck.add_slide("Graph");
        let x = slide.var("x");
        let rule = suit_core::Arg::fun(
            x,
            suit_core::Expr::call("log", vec![suit_core::Expr::Var(x)]),
        );
        let graph = slide.object(
            "graph",
            Object::new("FunctionGraph")
                .arg("function", rule)
                .arg("x_range", suit_core::Expr::vec3(from, 7.0, 0.05)),
        );
        slide.play(vec![Anim::prim("Create").arg("mobject", &graph)]);
        slide.wait(0.5);
        (deck, graph.root)
    };
    let mut prober = prober();

    let (deck, graph) = graph_over(-7.0);
    let checked = check(&deck, &sig);
    assert_eq!(checked.diagnostics, []);
    let shown = prober
        .probe(&deck, &sig, &checked, Watch::patient())
        .expect("the answer can be read");
    assert_eq!(shown.error, None);
    assert_eq!(shown.remarks.len(), 1);
    let remark = &shown.remarks[0];
    assert_eq!(
        (remark.slide, remark.var, remark.what.as_str()),
        (1, graph, "undefined")
    );
    let last = shown.steps.last().unwrap();
    assert!(
        !last.boxes.contains_key(&graph),
        "it has no place to speak of"
    );
    // The circle and the letters from the slide before are still there, and that is all.
    assert_eq!(
        last.drawn.len(),
        1 + "beside a graph".replace(' ', "").len()
    );
    // Playing the slide is the same: nothing of the graph, no trouble with the rest.
    let clip = prober
        .probe_frames(&deck, &sig, &checked, 1, Watch::patient())
        .expect("the answer can be read");
    let report = &checked.slides[1];
    assert_eq!(clip.error, None);
    assert_eq!(clip.frames.len() as u32, report.frames + report.gap_frames);

    // From where the logarithm has a value, there is a graph and nothing to remark on.
    let (deck, graph) = graph_over(0.1);
    let checked = check(&deck, &sig);
    let shown = prober
        .probe(&deck, &sig, &checked, Watch::patient())
        .expect("Manim runs");
    assert_eq!((shown.error.as_ref(), shown.remarks.len()), (None, 0));
    let last = shown.steps.last().unwrap();
    assert!(last.boxes[&graph].on);
    let [left, _, right, _] = last.boxes[&graph].rect;
    assert!(
        (left - 0.1).abs() < 0.01 && (right - 7.0).abs() < 0.06,
        "{left} to {right}"
    );
}

#[test]
#[ignore = "needs Manim"]
fn a_graph_is_drawn_where_its_rule_has_a_value() {
    let sig = Signature::builtin();
    use suit_core::{Arg, Expr};
    // A deck with one graph; `rule` builds it from its parameter.
    let graph_of = |rule: &dyn Fn(Expr) -> Expr, on: &[[f64; 2]], asymptotes: bool| {
        let mut deck = Deck::new(DeckConfig::default());
        let mut slide = deck.add_slide("Graph");
        let x = slide.var("x");
        let mut plot = Object::new("Plot").arg("function", Arg::fun(x, rule(Expr::Var(x))));
        for [from, to] in on {
            plot = plot.arg("on", Expr::vec3(*from, *to, 0.0));
        }
        if asymptotes {
            plot = plot.arg("asymptotes", true);
        }
        let graph = slide.object("graph", plot);
        slide.play(vec![Anim::prim("Create").arg("mobject", &graph)]);
        (deck, graph.root)
    };
    let mut prober = prober();
    let mut look = |deck: &Deck| {
        let checked = check(deck, &sig);
        assert_eq!(checked.diagnostics, []);
        let shown = prober
            .probe(deck, &sig, &checked, Watch::patient())
            .expect("Manim runs");
        assert_eq!(shown.error, None);
        // Playing it is no different, and as long as the timeline says.
        let clip = prober
            .probe_frames(deck, &sig, &checked, 0, Watch::patient())
            .expect("Manim runs");
        assert_eq!(clip.error, None);
        let report = &checked.slides[0];
        assert_eq!(clip.frames.len() as u32, report.frames + report.gap_frames);
        shown
    };
    let near = |found: &[f64], wanted: &[f64]| {
        found.len() == wanted.len() && found.iter().zip(wanted).all(|(a, b)| (a - b).abs() < 1e-3)
    };
    let inverse = |x: Expr| Expr::call("/", vec![Expr::Real(1.0), x]);
    let logarithm = |x: Expr| Expr::call("log", vec![x]);

    // 1/x, left to itself: two branches that stop short of the pole, and nothing wild.
    let (deck, graph) = graph_of(&inverse, &[], false);
    let shown = look(&deck);
    let found = &shown.remarks[0];
    assert_eq!((found.var, found.what.as_str()), (graph, "plot"));
    assert!(!found.missing && !found.chosen);
    assert_eq!(found.pieces.len(), 2);
    assert!(found.pieces[0][1] < -0.05 && found.pieces[1][0] > 0.05);
    assert!(near(&found.vertical, &[0.0]) && near(&found.horizontal, &[0.0]));
    let last = shown.steps.last().unwrap();
    assert_eq!(last.drawn.len(), 1, "the curve, and no lines");
    let far = last.drawn[0].paths.iter().flat_map(|path| &path.points);
    assert!(far.clone().all(|[x, y]| x.is_finite() && y.abs() < 20.0));
    assert_eq!(last.drawn[0].paths.len(), 2, "one stroke for each branch");

    // With its asymptotes shown: the two dashed lines are drawn as well.
    let (deck, _) = graph_of(&inverse, &[], true);
    let shown = look(&deck);
    assert!(shown.steps.last().unwrap().drawn.len() > 10, "dashes");

    // The logarithm, left to itself: from nought on, with nothing to put right.
    let (deck, _) = graph_of(&logarithm, &[], false);
    let shown = look(&deck);
    let found = &shown.remarks[0];
    assert!(found.missing && !found.chosen);
    assert_eq!(found.pieces.len(), 1);
    assert!(found.pieces[0][0].abs() < 1e-3 && near(&found.vertical, &[0.0]));

    // Told where: only there, and told of what the rule cannot do there.
    let (deck, _) = graph_of(&logarithm, &[[-7.0, 7.0]], false);
    let found = &look(&deck).remarks[0];
    assert!(found.missing && found.chosen);
    assert_eq!(found.asked, [[-7.0, 7.0]]);
    let (deck, _) = graph_of(&inverse, &[[-1.0, -0.1], [0.1, 1.0]], false);
    let found = &look(&deck).remarks[0];
    assert!(!found.missing && found.chosen);
    assert_eq!(found.pieces, [[-1.0, -0.1], [0.1, 1.0]]);
}

#[test]
#[ignore = "needs Manim"]
fn a_tangent_touches_its_graph_all_the_way_along() {
    use suit_core::{Arg, Call, Expr};
    let sig = Signature::builtin();
    let mut deck = Deck::new(DeckConfig::default());
    let mut slide = deck.add_slide("Tangent");
    let x = slide.var("x");
    let rule = Arg::fun(x, Expr::call("sin", vec![Expr::Var(x)]));
    let graph = slide.object("graph", Object::new("Plot").arg("function", rule));
    let tangent = slide.object(
        "tangent",
        Object::new("Tangent")
            .arg("graph", &graph)
            .arg("at", 0.0)
            .arg("length", 2.0),
    );
    slide.add(&[&graph, &tangent]);
    slide.play(vec![
        Anim::prim("Glide")
            .arg("mobject", &tangent)
            .arg("to", 3.0)
            .run_time(1.0)
            .rate("linear"),
    ]);
    slide.apply(&tangent, Call::new("touch").arg("x", 1.0));
    // A tangent to nothing: 1/x at 0.
    let y = slide.var("x");
    let inverse = Arg::fun(y, Expr::call("/", vec![Expr::Real(1.0), Expr::Var(y)]));
    let other = slide.object("other", Object::new("Plot").arg("function", inverse));
    let stray = slide.object(
        "stray",
        Object::new("Tangent").arg("graph", &other).arg("at", 0.0),
    );
    let checked = check(&deck, &sig);
    assert_eq!(checked.diagnostics, []);

    // The line of a snapshot or a frame: where its middle is, and how steep it is.
    let line_of = |drawn: &suit_render::Drawn| {
        let points = &drawn.paths[0].points;
        let (start, end) = (points[0], points[points.len() - 1]);
        let middle = [(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0];
        (middle, (end[1] - start[1]) / (end[0] - start[0]))
    };
    let touches = |(middle, slope): ([f32; 2], f32), x: f32| {
        (middle[0] - x).abs() < 2e-3
            && (middle[1] - x.sin()).abs() < 2e-3
            && (slope - x.cos()).abs() < 2e-3
    };

    let mut prober = prober();
    let shown = prober
        .probe(&deck, &sig, &checked, Watch::patient())
        .expect("Manim runs");
    assert_eq!(shown.error, None);
    let of_tangent = |step: &suit_render::Snapshot| {
        let own =
            |drawn: &&suit_render::Drawn| drawn.owners.iter().any(|(var, _)| *var == tangent.root);
        line_of(step.drawn.iter().find(own).expect("the tangent is drawn"))
    };
    // Shown at 0, after the glide at 3, after the touch at 1.
    assert!(
        touches(of_tangent(&shown.steps[3]), 0.0),
        "{:?}",
        of_tangent(&shown.steps[3])
    );
    assert!(
        touches(of_tangent(&shown.steps[4]), 3.0),
        "{:?}",
        of_tangent(&shown.steps[4])
    );
    assert!(
        touches(of_tangent(&shown.steps[5]), 1.0),
        "{:?}",
        of_tangent(&shown.steps[5])
    );
    // The one with nothing to touch is told of, by name.
    let lost: Vec<_> = shown
        .remarks
        .iter()
        .filter(|remark| remark.what == "lost")
        .collect();
    assert_eq!(lost.len(), 1);
    assert_eq!(lost[0].var, stray.root);

    // Played, it touches the graph in every frame of the glide: at a sixtieth of the way
    // more each time, the graph's own point and the graph's own steepness.
    let clip = prober
        .probe_frames(&deck, &sig, &checked, 0, Watch::patient())
        .expect("Manim runs");
    assert_eq!(clip.error, None);
    let report = &checked.slides[0];
    assert_eq!(clip.frames.len() as u32, report.frames + report.gap_frames);
    for frame in [0usize, 1, 17, 30, 59] {
        // The graph is one outline of many curves; the tangent is the one of a single line.
        let line = clip
            .frame(frame)
            .find(|drawn| drawn.paths.len() == 1 && drawn.paths[0].points.len() == 4);
        let x = 3.0 * frame as f32 / 60.0;
        assert!(touches(line_of(line.expect("a line")), x), "frame {frame}");
    }

    // A graph that is moved and stretched after it is drawn takes its tangent with it.
    let mut deck = Deck::new(DeckConfig::default());
    let mut slide = deck.add_slide("Moved");
    let x = slide.var("x");
    let rule = Arg::fun(x, Expr::call("sin", vec![Expr::Var(x)]));
    let moved = Object::new("Plot")
        .arg("function", rule)
        .then(Call::new("shift").arg("vectors", Expr::vec3(1.0, 2.0, 0.0)))
        .then(Call::new("stretch").arg("factor", 2.0).arg("dim", 1_i64));
    let graph = slide.object("graph", moved);
    let tangent = slide.object(
        "tangent",
        Object::new("Tangent").arg("graph", &graph).arg("at", 0.0),
    );
    slide.add(&[&graph, &tangent]);
    slide.wait(0.5);
    let checked = check(&deck, &sig);
    assert_eq!(checked.diagnostics, []);
    let shown = prober
        .probe(&deck, &sig, &checked, Watch::patient())
        .expect("Manim runs");
    assert_eq!(shown.error, None);
    let last = shown.steps.last().unwrap();
    let own =
        |drawn: &&suit_render::Drawn| drawn.owners.iter().any(|(var, _)| *var == tangent.root);
    let (middle, slope) = line_of(last.drawn.iter().find(own).expect("the tangent is drawn"));
    // The rule's point (0, 0) is now at (1, 2), and twice as steep as the sine is there.
    assert!(
        (middle[0] - 1.0).abs() < 2e-3 && (middle[1] - 2.0).abs() < 2e-3,
        "{middle:?}"
    );
    assert!((slope - 2.0).abs() < 2e-3, "{slope}");
}

#[test]
#[ignore = "needs Manim"]
fn an_updater_given_the_frame_time_moves_with_it() {
    let sig = Signature::builtin();
    let mut deck = Deck::new(DeckConfig::default());
    let mut slide = deck.add_slide("Drift");
    let dot = slide.object("dot", Object::new("Dot"));
    slide.add(&[&dot]);
    // A unit to the right every second, worked out from the time each frame takes.
    slide.every(&dot, |dt| {
        let step = Expr::call("*", vec![Expr::Var(dt), Expr::konst("RIGHT")]);
        Call::new("shift").arg("vectors", step)
    });
    slide.wait(1.0);
    let checked = check(&deck, &sig);
    assert_eq!(checked.diagnostics, []);

    let clip = prober()
        .probe_frames(&deck, &sig, &checked, 0, Watch::patient())
        .expect("Manim runs");
    assert_eq!(clip.error, None);
    let x = |index: usize| {
        let xs: Vec<f32> = clip
            .frame(index)
            .flat_map(|drawn| drawn.paths.iter().flat_map(|path| path.points.iter()))
            .map(|point| point[0])
            .collect();
        xs.iter().sum::<f32>() / xs.len() as f32
    };
    let travelled = x(clip.frames.len() - 1) - x(0);
    assert!((0.9..1.3).contains(&travelled), "travelled {travelled}");
}

/// Where two looks at a deck differ by more than the rounding in Manim's own arithmetic.
fn differences(seen: &[Snapshot], exact: &[Snapshot]) -> Vec<String> {
    let close = |a: f32, b: f32| (a - b).abs() <= 1e-4;
    let mut found = Vec::new();
    if seen.len() != exact.len() {
        return vec![format!("{} steps against {}", seen.len(), exact.len())];
    }
    for (index, (a, b)) in seen.iter().zip(exact).enumerate() {
        let at = format!("step {index} (slide {}, node {:?})", b.slide, b.node);
        if (a.slide, a.node) != (b.slide, b.node) || a.drawn.len() != b.drawn.len() {
            found.push(format!(
                "{at}: {} drawn against {}",
                a.drawn.len(),
                b.drawn.len()
            ));
            continue;
        }
        for (p, q) in a.drawn.iter().zip(&b.drawn) {
            let points = |drawn: &Drawn| -> Vec<[f32; 2]> {
                drawn
                    .paths
                    .iter()
                    .flat_map(|path| path.points.clone())
                    .collect()
            };
            let (pp, qp) = (points(p), points(q));
            let same = p.owners == q.owners
                && p.paths.len() == q.paths.len()
                && p.paths
                    .iter()
                    .zip(&q.paths)
                    .all(|(x, y)| x.closed == y.closed)
                && pp.len() == qp.len()
                && pp
                    .iter()
                    .zip(&qp)
                    .all(|(x, y)| close(x[0], y[0]) && close(x[1], y[1]))
                && p.fill.iter().zip(&q.fill).all(|(x, y)| close(*x, *y))
                && p.stroke.iter().zip(&q.stroke).all(|(x, y)| close(*x, *y))
                && close(p.stroke_width, q.stroke_width);
            if !same {
                found.push(format!("{at}: {:?} is drawn otherwise", q.owners));
            }
        }
        if a.boxes.keys().ne(b.boxes.keys()) {
            found.push(format!("{at}: other objects have places"));
            continue;
        }
        for (var, p) in &a.boxes {
            let q = &b.boxes[var];
            if p.on != q.on || !p.rect.iter().zip(&q.rect).all(|(x, y)| close(*x, *y)) {
                found.push(format!("{at}: {var:?} at {:?} against {:?}", p, q));
            }
        }
    }
    found
}

#[cfg(unix)]
#[test]
#[ignore = "needs Manim"]
fn what_the_probe_skips_ends_as_playing_it_does() {
    use std::os::unix::fs::PermissionsExt;

    let sig = Signature::builtin();
    let mut deck = Deck::new(DeckConfig::default());
    let point = |x, y| Expr::vec3(x, y, 0.0);
    let mut slide = deck.add_slide("In");
    let circle = slide.object("circle", Object::new("Circle"));
    let square = slide.object("square", Object::new("Square").arg("side_length", 1.0));
    let title = slide.object("title", Object::new("Text").arg("text", "Same words"));
    // Made just like the one before: the second is a copy of the first.
    let again = slide.object("again", Object::new("Text").arg("text", "Same words"));
    let marked = slide.object(
        "marked",
        Object::new("MarkupText").arg("text", "<b>bold</b>"),
    );
    let dot = slide.object("dot", Object::new("Dot"));
    let tri = slide.object("tri", Object::new("Triangle"));
    let star = slide.object("star", Object::new("Star"));
    let arrow = slide.object(
        "arrow",
        Object::new("Arrow")
            .arg("start", point(-2.0, -2.0))
            .arg("end", point(2.0, -2.0)),
    );
    let letters = slide.object("letters", Object::new("Text").arg("text", "abc"));
    let unseen = slide.object("unseen", Object::new("Square"));
    slide.play(vec![
        Anim::prim("Create").arg("mobject", &circle),
        Anim::prim("DrawBorderThenFill").arg("vmobject", &square),
    ]);
    slide.play(vec![
        Anim::prim("Write").arg("vmobject", &title),
        Anim::prim("FadeIn")
            .arg("mobject", &again)
            .arg("shift", Expr::konst("UP")),
        Anim::prim("FadeIn")
            .arg("mobject", &marked)
            .arg("scale", 2.0),
    ]);
    slide.play(vec![
        Anim::par(vec![
            Anim::prim("GrowFromCenter").arg("mobject", &dot),
            Anim::prim("SpinInFromNothing").arg("mobject", &tri),
        ])
        .lag(0.3),
    ]);
    slide.play(vec![
        Anim::prim("GrowArrow").arg("arrow", &arrow),
        Anim::prim("AddTextLetterByLetter").arg("text", &letters),
    ]);
    // Eased so that they do not end where they would: played.
    slide.play(vec![
        Anim::prim("FadeIn")
            .arg("mobject", &unseen)
            .rate("there_and_back"),
        Anim::prim("GrowFromEdge")
            .arg("mobject", &star)
            .arg("edge", Expr::konst("DOWN"))
            .rate("exponential_decay"),
    ]);

    let mut slide = deck.add_slide("Point");
    let extra = slide.object("extra", Object::new("Circle").arg("radius", 0.5));
    let shape = slide.object("shape", Object::new("RegularPolygon").arg("n", 6i64));
    let streak = slide.object(
        "streak",
        Object::new("Line")
            .arg("start", point(-3.0, 3.0))
            .arg("end", point(3.0, 3.0)),
    );
    slide.play(vec![
        Anim::prim("Indicate").arg("mobject", &circle),
        Anim::prim("Wiggle").arg("mobject", &dot),
        Anim::prim("ApplyWave").arg("mobject", &title),
    ]);
    slide.play(vec![
        // Ends large.
        Anim::prim("Indicate")
            .arg("mobject", &square)
            .rate("smooth"),
        Anim::prim("Circumscribe").arg("mobject", &again),
        Anim::prim("Flash").arg("point", &dot),
        Anim::prim("FocusOn").arg("focus_point", &circle),
    ]);
    slide.play(vec![Anim::prim("ShowPassingFlash").arg("mobject", &streak)]);
    // One played, one jumped, in one step.
    slide.play(vec![
        Anim::prim("Transform")
            .arg("mobject", &tri)
            .arg("target_mobject", &shape),
        Anim::prim("FadeIn").arg("mobject", &extra),
    ]);

    let mut slide = deck.add_slide("Out");
    let later = slide.object("later", Object::new("Square").arg("side_length", 0.5));
    slide.play(vec![
        Anim::prim("FadeOut")
            .arg("mobject", &circle)
            .arg("shift", Expr::konst("DOWN")),
        Anim::prim("Uncreate").arg("mobject", &square),
        Anim::prim("Unwrite").arg("vmobject", &title),
    ]);
    // While something follows another, all is played.
    slide.always(
        &again,
        Call::new("next_to")
            .arg("mobject_or_point", &dot)
            .arg("direction", Expr::konst("UP")),
    );
    slide.play(vec![
        Anim::prim("FadeIn").arg("mobject", &later),
        Anim::animate(
            &dot,
            vec![Call::new("shift").arg("vectors", Expr::konst("LEFT"))],
        ),
    ]);
    slide.detach(&again);
    slide.play(vec![Anim::prim("FadeOut").arg("mobject", &again)]);
    let checked = check(&deck, &sig);
    let errors: Vec<_> = checked
        .diagnostics
        .iter()
        .filter(|found| found.severity == Severity::Error)
        .collect();
    assert_eq!(errors, Vec::<&Diagnostic>::new());

    let sidecar = Sidecar::locate(Path::new(env!("CARGO_MANIFEST_DIR")));
    let folder = std::env::temp_dir().join(format!("suit-exact-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let exact_python = folder.join("python");
    let script = format!(
        "#!/bin/sh\nSUIT_PROBE_EXACT=1 exec '{}' \"$@\"\n",
        sidecar.python().display()
    );
    std::fs::write(&exact_python, script).unwrap();
    std::fs::set_permissions(&exact_python, std::fs::Permissions::from_mode(0o755)).unwrap();

    let seen = Prober::new(sidecar.clone())
        .probe(&deck, &sig, &checked, Watch::patient())
        .expect("Manim runs");
    let exact = Prober::new(sidecar.with_python(exact_python))
        .probe(&deck, &sig, &checked, Watch::patient())
        .expect("Manim runs");
    std::fs::remove_dir_all(&folder).ok();
    assert_eq!(seen.error, None);
    assert_eq!(exact.error, None);
    let found = differences(&seen.steps, &exact.steps);
    assert!(found.is_empty(), "{}", found.join("\n"));
}
