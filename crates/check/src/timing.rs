//! Static duration (spec §5.7), in the same binary64 arithmetic Manim uses, so frame counts
//! agree with a render exactly rather than nearly.

fn frame_step(fps: u32) -> f64 {
    1.0 / f64::from(fps)
}

/// Manim never renders less than one frame (`Scene.validate_run_time`).
fn at_least_one_frame(secs: f64, fps: u32) -> f64 {
    secs.max(frame_step(fps))
}

/// Frames emitted by a `play`, or by a `wait` while updaters run: the length of
/// `numpy.arange(0, run_time, 1 / fps)`.
pub fn play_frames(run_time: f64, fps: u32) -> u32 {
    (at_least_one_frame(run_time, fps) / frame_step(fps)).ceil() as u32
}

/// Frames emitted by a `wait` on a frozen scene: `int(duration / dt)`.
pub fn still_frames(duration: f64, fps: u32) -> u32 {
    (at_least_one_frame(duration, fps) / frame_step(fps)) as u32
}

/// Run time of `par` with stagger `lag`: Manim's `AnimationGroup.max_end_time`. Each child
/// starts `lag` × the previous child's run time after the previous child started.
pub fn group_run_time(run_times: &[f64], lag: f64) -> f64 {
    let mut start = 0.0;
    let mut end: f64 = 0.0;
    for &run_time in run_times {
        end = end.max(start + run_time);
        start += run_time * lag;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_second_durations_are_exact() {
        assert_eq!(play_frames(1.0, 60), 60);
        assert_eq!(play_frames(2.5, 60), 150);
        assert_eq!(still_frames(1.0, 60), 60);
        assert_eq!(play_frames(1.0, 30), 30);
    }

    /// Spec §5.7: about 8 % of whole-frame durations render one frame long.
    #[test]
    fn rounding_matches_numpy_not_arithmetic() {
        assert_eq!(play_frames(23.0 / 60.0, 60), 24);
        assert_eq!(still_frames(23.0 / 60.0, 60), 23);
        let long = (1..=600)
            .filter(|&n| play_frames(f64::from(n) / 60.0, 60) != n)
            .count();
        assert_eq!(long, 49);
    }

    #[test]
    fn nothing_is_shorter_than_a_frame() {
        assert_eq!(play_frames(0.001, 60), 1);
        assert_eq!(still_frames(0.001, 60), 1);
    }

    #[test]
    fn staggered_groups() {
        assert_eq!(group_run_time(&[1.0, 1.0, 1.0], 0.0), 1.0);
        assert_eq!(group_run_time(&[1.0, 2.0, 0.5], 1.0), 3.5);
        assert_eq!(group_run_time(&[1.0, 1.0, 1.0], 0.25), 1.5);
        // A long first child can outlast everything staggered after it.
        assert_eq!(group_run_time(&[10.0, 1.0], 0.1), 10.0);
        assert_eq!(group_run_time(&[], 0.5), 0.0);
    }
}
