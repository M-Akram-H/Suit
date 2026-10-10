use std::ops::{Add, Sub};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const FLICKS_PER_SECOND: i64 = 705_600_000;

/// Exact time in flicks (1/705_600_000 s). 24, 25, 30, 50, 60 and 120 fps all divide it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub struct Time(pub i64);

impl Time {
    pub const ZERO: Time = Time(0);

    pub fn from_secs(secs: f64) -> Time {
        Time((secs * FLICKS_PER_SECOND as f64).round() as i64)
    }

    /// Seconds as the nearest binary64, which is the value Manim is handed.
    pub fn secs(self) -> f64 {
        self.0 as f64 / FLICKS_PER_SECOND as f64
    }

    pub fn from_frames(frames: i64, fps: u32) -> Time {
        Time(frames * FLICKS_PER_SECOND / i64::from(fps))
    }
}

impl Add for Time {
    type Output = Time;
    fn add(self, rhs: Time) -> Time {
        Time(self.0 + rhs.0)
    }
}

impl Sub for Time {
    type Output = Time;
    fn sub(self, rhs: Time) -> Time {
        Time(self.0 - rhs.0)
    }
}

// Files store seconds: readable, and exact for every value an editor produces because
// reading rounds back to the nearest flick.
impl Serialize for Time {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_f64(self.secs())
    }
}

impl<'de> Deserialize<'de> for Time {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        f64::deserialize(d).map(Time::from_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_round_trip_through_flicks() {
        for secs in [0.5, 1.0 / 3.0, 0.1, 23.0 / 60.0, 2.5] {
            let t = Time::from_secs(secs);
            assert_eq!(Time::from_secs(t.secs()), t);
        }
        assert_eq!(Time::from_frames(30, 60), Time::from_secs(0.5));
    }
}
