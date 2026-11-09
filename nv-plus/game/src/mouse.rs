//! An NV+ option for looking around, not behaviour of the original game (no exe address).
//!
//! Smoothing spreads a frame's hand motion evenly over a short window, so the turn is the same at
//! any frame rate. Acceleration turns faster hand movement further, rising smoothly to a limit.
//! Menus and loading screens keep the game's own cursor.

use nv_plus_options::mouse::Feel;
use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

/// A frame's motion, spread over the smoothing window from `start` (seconds).
#[derive(Clone, Copy)]
struct Sample {
    start: f64,
    x: f64,
    y: f64,
    delivered: f64,
}

/// Looking around, frame by frame: motion not yet delivered, the clock and the counts held back.
#[derive(Default)]
pub struct LookMouse {
    pending: VecDeque<Sample>,
    clock: f64,
    remainder: [f64; 2],
}

impl LookMouse {
    /// This frame's counts after smoothing and acceleration. `dt` is the time since the last
    /// frame, in seconds.
    pub fn step(&mut self, feel: &Feel, dx: i32, dy: i32, dt: f64) -> (i32, i32) {
        if feel.is_off() {
            self.reset();
            return (dx, dy);
        }
        let dt = dt.max(0.0);
        let window = feel.smoothing_ms / 1000.0;
        let (mut x, mut y) = if window > 0.0 {
            self.smooth(window, dx, dy, dt)
        } else {
            (f64::from(dx), f64::from(dy))
        };
        if let Some(natural) = feel.accel {
            let speed = (x * x + y * y).sqrt() / (dt.max(0.0001) * 1000.0);
            let mult = natural.limit - (natural.limit - 1.0) * (-natural.ramp * speed).exp();
            x *= mult;
            y *= mult;
        }
        (self.emit(x, 0), self.emit(y, 1))
    }

    /// The motion spread over the window: each frame's counts come back evenly across `window`
    /// seconds, so the total is the same at any frame rate.
    fn smooth(&mut self, window: f64, dx: i32, dy: i32, dt: f64) -> (f64, f64) {
        let t0 = self.clock;
        let t1 = t0 + dt;
        if dx != 0 || dy != 0 {
            self.pending.push_back(Sample {
                start: t0,
                x: f64::from(dx),
                y: f64::from(dy),
                delivered: 0.0,
            });
        }
        let mut sum = [0.0f64; 2];
        self.pending.retain_mut(|sample| {
            let end = sample.start + window;
            // A sample's last frame gives what is left, so the shares, which round, add up to it.
            let part = if end <= t1 {
                1.0 - sample.delivered
            } else {
                (t1 - sample.start.max(t0)).max(0.0) / window
            };
            sample.delivered += part;
            sum[0] += sample.x * part;
            sum[1] += sample.y * part;
            end > t1
        });
        self.clock = t1;
        (sum[0], sum[1])
    }

    /// The whole counts of one axis, the fraction kept for the next frame.
    fn emit(&mut self, value: f64, axis: usize) -> i32 {
        let total = value + self.remainder[axis];
        // Shares of a count that add up to a whole one can land a hair under it.
        let near = total.round();
        let whole = if (total - near).abs() < 1e-9 {
            near
        } else {
            total.trunc()
        };
        self.remainder[axis] = total - whole;
        whole as i32
    }

    /// Drops the pending motion and the counts held back.
    pub fn reset(&mut self) {
        self.pending.clear();
        self.remainder = [0.0; 2];
        self.clock = 0.0;
    }
}

/// Whether looking around runs: in play, with either part on.
fn running(in_play: bool, feel: &Feel) -> bool {
    in_play && !feel.is_off()
}

/// One frame, with the time step given, so it can be tested without a clock or the statics.
fn frame(
    mouse: &mut LookMouse,
    in_play: bool,
    feel: &Feel,
    dx: i32,
    dy: i32,
    dt: f64,
) -> (i32, i32) {
    if !running(in_play, feel) {
        mouse.reset();
        return (dx, dy);
    }
    mouse.step(feel, dx, dy, dt)
}

/// The feel bound at start-up.
pub fn set(feel: Feel) {
    *feel_slot() = feel;
}

fn feel_slot() -> MutexGuard<'static, Feel> {
    static FEEL: OnceLock<Mutex<Feel>> = OnceLock::new();
    FEEL.get_or_init(|| Mutex::new(Feel::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn state() -> MutexGuard<'static, (LookMouse, Option<Instant>)> {
    static STATE: OnceLock<Mutex<(LookMouse, Option<Instant>)>> = OnceLock::new();
    STATE
        .get_or_init(|| Mutex::new((LookMouse::default(), None)))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The player's motion for the game's mouse, after the feel set by [`set`]: unchanged in a menu
/// or a loading screen, and unchanged with both parts off.
pub fn look(in_play: bool, dx: i32, dy: i32) -> (i32, i32) {
    let feel = *feel_slot();
    let mut state = state();
    let (mouse, last) = &mut *state;
    let now = Instant::now();
    let dt = if running(in_play, &feel) {
        last.replace(now).map_or(1.0 / 60.0, |previous| {
            now.duration_since(previous).as_secs_f64()
        })
    } else {
        *last = None;
        0.0
    };
    frame(mouse, in_play, &feel, dx, dy, dt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nv_plus_options::mouse::Natural;

    #[test]
    fn off_passes_counts_through_and_keeps_nothing() {
        let mut mouse = LookMouse::default();
        let feel = Feel {
            smoothing_ms: 50.0,
            accel: None,
        };
        mouse.step(&feel, 40, 0, 0.004);
        for (dx, dy, dt) in [(10, -4, 0.016), (0, 0, 0.001), (-25, 7, 0.5), (3, 3, 0.0)] {
            assert_eq!(
                mouse.step(&Feel::default(), dx, dy, dt),
                (dx, dy),
                "dt {dt}"
            );
        }
        assert!(mouse.pending.is_empty());
        assert_eq!(mouse.clock, 0.0);
        assert_eq!(mouse.remainder, [0.0, 0.0]);
    }

    #[test]
    fn smoothing_conserves_motion() {
        let feel = Feel {
            smoothing_ms: 10.0,
            accel: None,
        };
        let mut mouse = LookMouse::default();
        let mut out = Vec::new();
        for (dx, dy) in [(100, 0), (0, 0), (0, 0), (0, 0)] {
            out.push(mouse.step(&feel, dx, dy, 0.004));
        }
        assert_eq!(out, vec![(40, 0), (40, 0), (20, 0), (0, 0)]);
    }

    /// 6 counts a millisecond for 200 ms, fed at 60 and at 144 frames a second: the first frame
    /// holds motion back at both, the same total comes out once the last window has drained, and
    /// the same rate per millisecond once the first window has passed.
    #[test]
    fn smoothing_is_the_same_at_any_frame_rate() {
        let feel = Feel {
            smoothing_ms: 50.0,
            accel: None,
        };
        let run = |fps: f64| {
            let dt = 1.0 / fps;
            let frames = (0.2 * fps).round() as usize;
            let mut mouse = LookMouse::default();
            let mut t = 0.0;
            let mut fed = 0i64;
            let mut total = 0i64;
            let mut steady = Vec::new();
            let mut first = None;
            for frame in 0..frames {
                let step = if frame + 1 == frames { 0.2 - t } else { dt };
                t += step;
                let ideal = (6000.0 * t).round() as i64;
                let (x, _) = mouse.step(&feel, (ideal - fed) as i32, 0, step);
                first.get_or_insert((i64::from(x), ideal));
                fed = ideal;
                total += i64::from(x);
                if t >= 0.05 {
                    steady.push(f64::from(x) / (step * 1000.0));
                }
            }
            let (first_out, first_in) = first.unwrap();
            assert!(first_out < first_in, "{fps}: {first_out} of {first_in}");
            for _ in 0..(fps as usize / 20 + 2) {
                let (x, _) = mouse.step(&feel, 0, 0, dt);
                total += i64::from(x);
            }
            (total, steady)
        };
        let (slow, slow_rate) = run(60.0);
        let (fast, fast_rate) = run(144.0);
        assert!((slow - fast).abs() <= 1, "60: {slow}, 144: {fast}");
        assert_eq!(slow, 1200);
        for rate in slow_rate.into_iter().chain(fast_rate) {
            assert!((rate - 6.0).abs() < 0.5, "rate {rate}");
        }
    }

    #[test]
    fn a_frame_at_least_the_window_through_comes_out_whole() {
        let feel = Feel {
            smoothing_ms: 10.0,
            accel: None,
        };
        let mut mouse = LookMouse::default();
        assert_eq!(mouse.step(&feel, 100, -30, 0.02), (100, -30));
        assert_eq!(mouse.step(&feel, 7, 7, 0.02), (7, 7));
        assert_eq!(mouse.step(&feel, 0, 0, 0.02), (0, 0));
        // A short frame splits the motion across the window instead.
        assert_eq!(mouse.step(&feel, 5, 5, 0.004), (2, 2));
    }

    /// The multiplier a frame's counts get at `speed` counts a millisecond: 2000 counts over the
    /// time that makes that speed, many enough that dropping the fraction doesn't blur it.
    fn at(speed: f64) -> f64 {
        let feel = Feel {
            smoothing_ms: 0.0,
            accel: Some(Natural {
                limit: 2.0,
                ramp: 0.1,
            }),
        };
        let mut mouse = LookMouse::default();
        let (x, _) = mouse.step(&feel, 2000, 0, 2.0 / speed);
        f64::from(x) / 2000.0
    }

    #[test]
    fn acceleration_rises_with_speed_towards_its_limit() {
        let feel = Feel {
            smoothing_ms: 0.0,
            accel: Some(Natural {
                limit: 2.0,
                ramp: 0.1,
            }),
        };
        assert!((at(0.01) - 1.0).abs() < 0.002, "{}", at(0.01));
        // 2 - 1 * e^(-0.1 * 5) = 1.3935 counts out per count at 5 counts a millisecond.
        assert!((at(5.0) - 1.3935).abs() < 0.001, "{}", at(5.0));
        let mut last = 0.0;
        for speed in [0.5, 1.0, 2.0, 5.0, 10.0, 50.0, 200.0] {
            let mult = at(speed);
            assert!(mult > last, "speed {speed}");
            assert!(mult <= 2.0, "speed {speed}: {mult}");
            last = mult;
        }
        let mut mouse = LookMouse::default();
        let (x, y) = mouse.step(&feel, -2000, 2000, 0.4);
        assert!(x == -y && x < -2000, "{x}, {y}");
    }

    /// A flick spread over several frames reaches the game whole once the hand stops, whatever
    /// the frame times.
    #[test]
    fn a_spread_flick_delivers_every_count() {
        let feel = Feel {
            smoothing_ms: 20.0,
            accel: None,
        };
        for dt in [0.01281, 0.0037, 0.0069, 0.0091, 0.0133] {
            for counts in [5, -5, 37, 1] {
                let mut mouse = LookMouse::default();
                let mut total = mouse.step(&feel, counts, -counts, dt);
                for _ in 0..40 {
                    let (x, y) = mouse.step(&feel, 0, 0, dt);
                    total.0 += x;
                    total.1 += y;
                }
                assert_eq!(total, (counts, -counts), "dt {dt}");
            }
        }
    }

    #[test]
    fn the_remainder_carries_counts_between_frames() {
        // ramp -ln(0.79) at 1 count a millisecond makes the multiplier 1.21, so ten counts in
        // come out as twelve whole ones.
        let feel = Feel {
            smoothing_ms: 0.0,
            accel: Some(Natural {
                limit: 2.0,
                ramp: -0.79f64.ln(),
            }),
        };
        let mut mouse = LookMouse::default();
        let total: i32 = (0..10).map(|_| mouse.step(&feel, 1, 0, 0.001).0).sum();
        assert_eq!(total, 12);

        let smooth = Feel {
            smoothing_ms: 50.0,
            accel: None,
        };
        let mut mouse = LookMouse::default();
        let mut out = Vec::new();
        for frame in 0..20 {
            let (x, _) = mouse.step(&smooth, if frame == 0 { 1 } else { 0 }, 0, 0.004);
            out.push(x);
        }
        assert_eq!(out.iter().sum::<i32>(), 1, "{out:?}");
    }

    /// A menu or a loading screen passes the counts on and drops what smoothing still had, so it
    /// never arrives after the menu.
    #[test]
    fn a_menu_keeps_its_counts_and_drops_the_pending_motion() {
        let feel = Feel {
            smoothing_ms: 50.0,
            accel: None,
        };
        let mut mouse = LookMouse::default();
        assert_eq!(frame(&mut mouse, true, &feel, 100, 0, 0.004), (8, 0));
        assert_eq!(frame(&mut mouse, false, &feel, 3, -2, 0.004), (3, -2));
        assert!(mouse.pending.is_empty());
        assert_eq!(frame(&mut mouse, true, &feel, 0, 0, 0.004), (0, 0));
    }
}
