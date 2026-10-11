//! Runs real Manim, so these are ignored unless asked for:
//! `cargo test -p suit-render -- --ignored`

use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};

use suit_check::check;
use suit_core::{Anim, Command, Deck, DeckConfig, Object, Raw, RawIface, Time};
use suit_render::{Failure, Prober, Sidecar, Watch};
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
