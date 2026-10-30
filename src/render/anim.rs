//! Frame-callback-driven animation, using **niri's vocabulary**.
//!
//! The value types (`Curve`, `Spring`, `Animation`, the curves' maths) live in
//! `niripaper-proto::animation` — config files name them and the schema reports
//! them, so the desktop app compiles against the same spellings. What stays
//! here is the part only a renderer needs: the [`Animator`] state machine.
//!
//! Two rules come from §4.2.6 and are easy to get wrong, both very visible:
//!
//! * an identical target must be ignored — restarting the easing on every event
//!   makes the wallpaper stop dead in the middle of a move;
//! * a new target mid-flight continues from *where the value currently is*.
//!
//! The clock is passed in as `Instant`, so this stays testable without sleeping.

use std::time::{Duration, Instant};

use niripaper_proto::animation::Animation;

/// Values the animator can interpolate.
pub trait Lerp: Copy {
    fn lerp(self, to: Self, t: f64) -> Self;
    /// Magnitude of the difference — a spring settles once this is within its
    /// `epsilon`.
    fn distance(self, to: Self) -> f64;
}

impl Lerp for f64 {
    fn lerp(self, to: Self, t: f64) -> Self {
        self + (to - self) * t
    }

    fn distance(self, to: Self) -> f64 {
        (to - self).abs()
    }
}

impl Lerp for crate::motion::Progress {
    fn lerp(self, to: Self, t: f64) -> Self {
        crate::motion::Progress::lerp(self, to, t)
    }

    fn distance(self, to: Self) -> f64 {
        (to.horizontal - self.horizontal)
            .abs()
            .max((to.vertical - self.vertical).abs())
    }
}

/// One of niri's animation types, with its normalized-progress evaluation —
/// the piece of [`niripaper_proto::animation::Animation`] the state machine
/// needs and the value types alone do not carry.
trait Advance {
    fn advance(&self, elapsed: Duration, distance: f64) -> (f64, bool);
}

impl Advance for Animation {
    fn advance(&self, elapsed: Duration, distance: f64) -> (f64, bool) {
        match self {
            Self::Off => (1.0, false),
            Self::Easing { duration, curve } => {
                let total = duration.as_secs_f64();
                if total <= 0.0 {
                    return (1.0, false);
                }
                let t = elapsed.as_secs_f64() / total;
                if t >= 1.0 {
                    (1.0, false)
                } else {
                    (curve.eval(t), true)
                }
            }
            Self::Spring(spring) => {
                let seconds = elapsed.as_secs_f64();
                (spring.eval(seconds), !spring.settled(seconds, distance))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Animator<T> {
    from: T,
    to: T,
    started: Option<Instant>,
    animation: Animation,
    /// niri's `slowdown`: divides the elapsed time, so every animation gets
    /// slower (or faster, below 1.0) without changing its shape.
    slowdown: f64,
}

impl<T: Lerp + PartialEq> Animator<T> {
    /// At rest at `at`.
    pub fn new(at: T, animation: Animation) -> Self {
        Self {
            from: at,
            to: at,
            started: None,
            animation,
            slowdown: 1.0,
        }
    }

    pub fn with_slowdown(mut self, slowdown: f64) -> Self {
        self.slowdown = if slowdown > 0.0 { slowdown } else { 1.0 };
        self
    }

    pub fn animation(&self) -> Animation {
        self.animation
    }

    /// Where the value is heading.
    pub fn target(&self) -> T {
        self.to
    }

    pub fn is_moving(&self) -> bool {
        self.started.is_some()
    }

    /// Aim at `target`. Returns `false` when the target is unchanged, in which
    /// case nothing at all happens (§4.2.6).
    pub fn retarget(&mut self, target: T, now: Instant) -> bool {
        if target == self.to {
            return false;
        }
        // Take the current position *before* switching targets: this is what
        // makes a mid-flight retarget continue instead of jumping.
        self.from = self.position(now);
        self.to = target;
        self.started = Some(now);
        true
    }

    /// The current value, and whether it is still moving.
    ///
    /// Once the animation finishes the animator comes to rest: no further
    /// frames are needed, which is what keeps an idle wallpaper at zero cost.
    pub fn sample(&mut self, now: Instant) -> (T, bool) {
        let Some(started) = self.started else {
            return (self.to, false);
        };
        let elapsed = self.elapsed(started, now);
        let distance = self.from.distance(self.to);
        let (progress, moving) = self.animation.advance(elapsed, distance);
        if !moving {
            self.started = None;
            self.from = self.to;
            return (self.to, false);
        }
        (self.from.lerp(self.to, progress), true)
    }

    /// The value right now, without changing the state.
    pub fn position(&self, now: Instant) -> T {
        let Some(started) = self.started else {
            return self.to;
        };
        let elapsed = self.elapsed(started, now);
        let distance = self.from.distance(self.to);
        let (progress, moving) = self.animation.advance(elapsed, distance);
        if !moving {
            return self.to;
        }
        self.from.lerp(self.to, progress)
    }

    fn elapsed(&self, started: Instant, now: Instant) -> Duration {
        now.saturating_duration_since(started)
            .div_f64(self.slowdown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::Progress;
    use niripaper_proto::animation::{out_cubic, Curve, Spring};

    /// A fixed origin, so `ms(n)` is always `n` ms after `ms(0)`.
    struct Clock(Instant);

    impl Clock {
        fn new() -> Self {
            Self(Instant::now())
        }

        fn ms(&self, millis: u64) -> Instant {
            self.0 + Duration::from_millis(millis)
        }
    }

    fn cubic(duration_ms: u64) -> Animation {
        Animation::easing(Curve::EaseOutCubic, Duration::from_millis(duration_ms))
    }

    // --- curves ------------------------------------------------------------

    #[test]
    fn curves_start_at_zero_and_end_at_one() {
        for curve in [
            Curve::Linear,
            Curve::EaseOutQuad,
            Curve::EaseOutCubic,
            Curve::EaseOutExpo,
            Curve::CubicBezier([0.05, 0.7, 0.1, 1.0]),
        ] {
            assert!(curve.eval(0.0).abs() < 1e-9, "{}", curve.name());
            assert!((curve.eval(1.0) - 1.0).abs() < 1e-9, "{}", curve.name());
            // Clamped outside 0..=1.
            assert!(curve.eval(-1.0).abs() < 1e-9, "{}", curve.name());
            assert!((curve.eval(2.0) - 1.0).abs() < 1e-9, "{}", curve.name());
        }
    }

    #[test]
    fn out_curves_decelerate() {
        // "ease-out" means most of the distance is covered early.
        for curve in [Curve::EaseOutQuad, Curve::EaseOutCubic, Curve::EaseOutExpo] {
            // All three are ahead of linear at the quarter point (quad is the
            // gentlest: 1 - 0.75² = 0.4375) and well ahead at the halfway point.
            assert!(curve.eval(0.25) > 0.4, "{}", curve.name());
            assert!(curve.eval(0.5) > 0.7, "{}", curve.name());
            assert!(curve.eval(0.5) < 1.0, "{}", curve.name());
        }
        assert!((Curve::EaseOutCubic.eval(0.5) - 0.875).abs() < 1e-12);
        assert_eq!(out_cubic(0.5), Curve::EaseOutCubic.eval(0.5));
    }

    #[test]
    fn cubic_bezier_matches_css() {
        // cubic-bezier(0, 0, 1, 1) is linear by definition.
        let linear = Curve::CubicBezier([0.0, 0.0, 1.0, 1.0]);
        for step in 0..=10 {
            let t = step as f64 / 10.0;
            assert!((linear.eval(t) - t).abs() < 1e-6, "t = {t}");
        }
        // The CSS example from niri's docs must be monotonic and decelerating.
        let curve = Curve::CubicBezier([0.05, 0.7, 0.1, 1.0]);
        let mut previous = 0.0;
        for step in 0..=100 {
            let value = curve.eval(step as f64 / 100.0);
            assert!(value >= previous - 1e-9, "not monotonic at {step}");
            previous = value;
        }
        assert!(curve.eval(0.25) > 0.5);
    }

    #[test]
    fn curve_names_round_trip_and_reject_typos() {
        for name in Curve::NAMES {
            let params = (name == "cubic-bezier").then_some([0.1, 0.2, 0.3, 0.4]);
            let curve = Curve::parse(name, params).expect("parses");
            assert_eq!(curve.name(), name);
        }
        let err = Curve::parse("ease-out-cubiccc", None).unwrap_err();
        assert!(err.contains("ease-out-cubic"), "{err}");
        let err = Curve::parse("cubic-bezier", None).unwrap_err();
        assert!(err.contains("four control points"), "{err}");
    }

    // --- springs -----------------------------------------------------------

    #[test]
    fn critically_damped_spring_does_not_overshoot() {
        let spring = Spring {
            damping_ratio: 1.0,
            stiffness: 400.0,
            epsilon: 1e-6,
        };
        let mut previous = 0.0;
        for step in 0..=200 {
            let value = spring.eval(step as f64 * 0.01);
            assert!(value >= previous - 1e-12, "not monotonic at {step}");
            assert!(value <= 1.0 + 1e-12, "overshoot at {step}: {value}");
            previous = value;
        }
        assert!((spring.eval(10.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn underdamped_spring_oscillates() {
        let spring = Spring {
            damping_ratio: 0.5,
            stiffness: 400.0,
            epsilon: 1e-6,
        };
        let overshoot = (0..2000)
            .map(|step| spring.eval(step as f64 * 0.001))
            .fold(f64::MIN, f64::max);
        assert!(
            overshoot > 1.0,
            "underdamped spring should overshoot, got {overshoot}"
        );
    }

    #[test]
    fn stiffer_springs_settle_sooner() {
        let soft = Spring {
            damping_ratio: 1.0,
            stiffness: 100.0,
            epsilon: 1e-3,
        };
        let stiff = Spring {
            damping_ratio: 1.0,
            stiffness: 1000.0,
            epsilon: 1e-3,
        };
        let settle = |spring: Spring| {
            (0..10_000)
                .map(|step| step as f64 * 0.001)
                .find(|t| spring.settled(*t, 1.0))
                .expect("settles")
        };
        assert!(
            settle(stiff) < settle(soft),
            "{} vs {}",
            settle(stiff),
            settle(soft)
        );
    }

    #[test]
    fn an_underdamped_spring_keeps_going_through_its_overshoot() {
        // Regression: judging "done" by the instantaneous error stopped the
        // animation at the first crossing, so the overshoot never rendered.
        let spring = Spring {
            damping_ratio: 0.5,
            stiffness: 400.0,
            epsilon: 0.001,
        };
        let crossing = std::f64::consts::PI / (400f64.sqrt() * 0.75f64.sqrt());
        assert!(
            !spring.settled(crossing, 0.04),
            "stopped at the first crossing"
        );
        assert!(spring.settled(2.0, 0.04));
        // The envelope is monotonic even though the value oscillates.
        let mut previous = f64::MAX;
        for step in 0..=500 {
            let envelope = spring.envelope(step as f64 * 0.01);
            assert!(envelope <= previous + 1e-12, "envelope grew at step {step}");
            previous = envelope;
        }
    }

    #[test]
    fn epsilon_controls_when_a_spring_is_done() {
        let loose = Spring {
            damping_ratio: 1.0,
            stiffness: 400.0,
            epsilon: 0.01,
        };
        let tight = Spring {
            damping_ratio: 1.0,
            stiffness: 400.0,
            epsilon: 0.0001,
        };
        // ζ=1, ω=√400=20: the envelope is e^{-20t}(1 + 20t), which is 5.0e-4
        // at 0.5 s and 1.1e-3 at 0.4 s.
        assert!(loose.settled(0.4, 1.0)); // envelope 3.0e-3 ≤ 1e-2
        assert!(!tight.settled(0.4, 1.0)); // 3.0e-3 > 1e-4
        assert!(tight.settled(0.6, 1.0)); // envelope 7.9e-5 ≤ 1e-4
                                          // Distance matters: the same spring over a tiny distance is done sooner.
        assert!(tight.settled(0.2, 0.001));
    }

    // --- animator semantics ------------------------------------------------

    #[test]
    fn starts_at_rest() {
        let animator = Animator::new(Progress::new(0.3, 0.7), cubic(600));
        assert!(!animator.is_moving());
        assert_eq!(animator.target(), Progress::new(0.3, 0.7));
        assert_eq!(
            animator.position(Clock::new().ms(0)),
            Progress::new(0.3, 0.7)
        );
    }

    #[test]
    fn identical_target_is_ignored() {
        // §4.2.6: 收到与当前目标相同的目标必须忽略
        let clock = Clock::new();
        let mut animator = Animator::new(Progress::CENTER, cubic(600));
        assert!(animator.retarget(Progress::new(0.2, 0.5), clock.ms(0)));
        assert!(!animator.retarget(Progress::new(0.2, 0.5), clock.ms(100)));
        assert_eq!(animator.target(), Progress::new(0.2, 0.5));
        assert!(animator.is_moving());
    }

    #[test]
    fn eases_monotonically_over_the_duration() {
        let clock = Clock::new();
        let mut animator = Animator::new(Progress::new(0.0, 0.0), cubic(600));
        assert!(animator.retarget(Progress::new(1.0, 1.0), clock.ms(0)));

        let (start, moving) = animator.sample(clock.ms(0));
        assert_eq!(start, Progress::new(0.0, 0.0));
        assert!(moving);

        // Halfway through, OutCubic is already 87.5% of the way there.
        let (middle, moving) = animator.sample(clock.ms(300));
        assert!(moving);
        assert!((middle.horizontal - 0.875).abs() < 1e-9, "{middle:?}");

        let mut previous = start.horizontal;
        for millis in (0..=600).step_by(10) {
            let (progress, _) = animator.sample(clock.ms(millis));
            assert!(
                progress.horizontal >= previous,
                "not monotonic at {millis} ms"
            );
            previous = progress.horizontal;
        }

        let (end, moving) = animator.sample(clock.ms(600));
        assert_eq!(end, Progress::new(1.0, 1.0));
        assert!(!moving, "the animator must come to rest");
    }

    #[test]
    fn retarget_mid_flight_continues_from_the_current_position() {
        let clock = Clock::new();
        let mut animator = Animator::new(Progress::new(0.0, 0.5), cubic(600));
        animator.retarget(Progress::new(1.0, 0.5), clock.ms(0));

        let (current, _) = animator.sample(clock.ms(300));
        assert!(animator.retarget(Progress::new(0.2, 0.5), clock.ms(300)));

        // No jump: the new easing starts exactly where the old one was.
        let (just_after, moving) = animator.sample(clock.ms(300));
        assert!(moving);
        assert!(
            (just_after.horizontal - current.horizontal).abs() < 1e-12,
            "{just_after:?} vs {current:?}"
        );

        let (end, moving) = animator.sample(clock.ms(900));
        assert_eq!(end, Progress::new(0.2, 0.5));
        assert!(!moving);
    }

    #[test]
    fn drives_plain_scalars_too() {
        // The overview zoom is an `Animator<f64>`; the semantics must match.
        let clock = Clock::new();
        let mut zoom = Animator::new(1.0f64, cubic(350));
        assert!(!zoom.is_moving());
        assert!(zoom.retarget(0.96, clock.ms(0)));
        assert!(
            !zoom.retarget(0.96, clock.ms(50)),
            "identical target ignored"
        );
        let (middle, moving) = zoom.sample(clock.ms(175));
        assert!(moving);
        assert!((middle - (1.0 - 0.04 * 0.875)).abs() < 1e-9, "{middle}");
        let (end, moving) = zoom.sample(clock.ms(350));
        assert!(!moving);
        assert!((end - 0.96).abs() < 1e-12);
    }

    #[test]
    fn off_jumps_straight_to_the_target() {
        let clock = Clock::new();
        let mut animator = Animator::new(0.0f64, Animation::Off);
        assert!(animator.retarget(1.0, clock.ms(0)));
        let (value, moving) = animator.sample(clock.ms(0));
        assert_eq!(value, 1.0);
        assert!(!moving);
        assert!(!animator.is_moving());
    }

    #[test]
    fn slowdown_stretches_every_animation() {
        let clock = Clock::new();
        let mut normal = Animator::new(0.0f64, cubic(600));
        let mut slowed = Animator::new(0.0f64, cubic(600)).with_slowdown(3.0);
        normal.retarget(1.0, clock.ms(0));
        slowed.retarget(1.0, clock.ms(0));

        // At 600 ms the normal one is done; the slowed one is a third through.
        let (done, moving) = normal.sample(clock.ms(600));
        assert_eq!(done, 1.0);
        assert!(!moving);
        let (partial, moving) = slowed.sample(clock.ms(600));
        assert!(moving);
        assert!(
            (partial - Curve::EaseOutCubic.eval(1.0 / 3.0)).abs() < 1e-9,
            "{partial}"
        );
        // …and it finishes at 3× the duration.
        let (end, moving) = slowed.sample(clock.ms(1800));
        assert_eq!(end, 1.0);
        assert!(!moving);
    }

    #[test]
    fn springs_drive_the_animator_to_rest() {
        let clock = Clock::new();
        let mut animator = Animator::new(0.0f64, Animation::spring(0.5, 400.0, 0.001));
        animator.retarget(1.0, clock.ms(0));
        let mut frames = 0;
        let mut settled_at = None;
        for step in 0..2000 {
            let (value, moving) = animator.sample(clock.ms(step * 16));
            frames += 1;
            if !moving {
                settled_at = Some(step * 16);
                assert!((value - 1.0).abs() < 1e-3, "settled at {value}");
                break;
            }
        }
        let settled_at = settled_at.expect("a spring must settle");
        assert!(frames > 5, "should take several frames");
        assert!(settled_at < 2000, "settled at {settled_at} ms");
        assert!(!animator.is_moving());
    }

    #[test]
    fn sampling_after_rest_needs_no_frames() {
        let clock = Clock::new();
        let mut animator = Animator::new(Progress::CENTER, cubic(600));
        animator.retarget(Progress::new(1.0, 1.0), clock.ms(0));
        let (_, moving) = animator.sample(clock.ms(600));
        assert!(!moving);
        let (again, moving) = animator.sample(clock.ms(10_000));
        assert_eq!(again, Progress::new(1.0, 1.0));
        assert!(!moving);
    }
}
