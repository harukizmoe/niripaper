//! Frame-callback-driven easing (`HANDOFF.md` §4.2.6).
//!
//! `OutCubic` over 600 ms, with two rules that are easy to get wrong and very
//! visible when you do:
//!
//! * an identical target must be ignored — a QML `Behavior` only animates when
//!   the source value changes, and restarting the easing on every event makes
//!   the wallpaper stop dead in the middle of a move;
//! * a new target mid-flight eases from *where the wallpaper currently is*, not
//!   from where the previous target started.
//!
//! The clock is passed in as `Instant`, so this stays testable without sleeping.

use std::time::{Duration, Instant};

use crate::motion::Progress;

/// §4.2.6: `durations.large` = 600 ms.
pub const DEFAULT_DURATION: Duration = Duration::from_millis(600);

/// `Easing.OutCubic`.
pub fn out_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Animator {
    from: Progress,
    to: Progress,
    started: Option<Instant>,
    duration: Duration,
}

impl Animator {
    /// At rest at `at`.
    pub fn new(at: Progress) -> Self {
        Self {
            from: at,
            to: at,
            started: None,
            duration: DEFAULT_DURATION,
        }
    }

    pub fn with_duration(at: Progress, duration: Duration) -> Self {
        Self {
            duration,
            ..Self::new(at)
        }
    }

    /// Where the wallpaper is heading.
    pub fn target(&self) -> Progress {
        self.to
    }

    pub fn is_moving(&self) -> bool {
        self.started.is_some()
    }

    /// Aim at `target`. Returns `false` when the target is unchanged, in which
    /// case nothing at all happens (§4.2.6).
    pub fn retarget(&mut self, target: Progress, now: Instant) -> bool {
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

    /// The current position, and whether it is still moving.
    ///
    /// Once the easing finishes the animator comes to rest: no further frames
    /// are needed, which is what keeps an idle wallpaper at zero cost.
    pub fn sample(&mut self, now: Instant) -> (Progress, bool) {
        let Some(started) = self.started else {
            return (self.to, false);
        };
        let elapsed = now.saturating_duration_since(started);
        if elapsed >= self.duration || self.duration.is_zero() {
            self.started = None;
            self.from = self.to;
            return (self.to, false);
        }
        let t = elapsed.as_secs_f64() / self.duration.as_secs_f64();
        (self.from.lerp(self.to, out_cubic(t)), true)
    }

    /// The position right now, without changing the state.
    pub fn position(&self, now: Instant) -> Progress {
        let Some(started) = self.started else {
            return self.to;
        };
        let elapsed = now.saturating_duration_since(started);
        if elapsed >= self.duration {
            return self.to;
        }
        let t = elapsed.as_secs_f64() / self.duration.as_secs_f64();
        self.from.lerp(self.to, out_cubic(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn out_cubic_has_the_expected_shape() {
        assert_eq!(out_cubic(0.0), 0.0);
        assert_eq!(out_cubic(1.0), 1.0);
        assert!((out_cubic(0.5) - 0.875).abs() < 1e-12);
        // Decelerating: the first half covers most of the distance.
        assert!(out_cubic(0.25) > 0.5);
        // Clamped outside 0..=1.
        assert_eq!(out_cubic(-1.0), 0.0);
        assert_eq!(out_cubic(2.0), 1.0);
    }

    #[test]
    fn starts_at_rest() {
        let clock = Clock::new();
        let animator = Animator::new(Progress::new(0.3, 0.7));
        assert!(!animator.is_moving());
        assert_eq!(animator.target(), Progress::new(0.3, 0.7));
        assert_eq!(animator.position(clock.ms(0)), Progress::new(0.3, 0.7));
    }

    #[test]
    fn identical_target_is_ignored() {
        let clock = Clock::new();
        // §4.2.6: 收到与当前目标相同的目标必须忽略
        let mut animator = Animator::new(Progress::CENTER);
        assert!(animator.retarget(Progress::new(0.2, 0.5), clock.ms(0)));
        assert!(!animator.retarget(Progress::new(0.2, 0.5), clock.ms(100)));
        // Still the same target, still moving from the original start.
        assert_eq!(animator.target(), Progress::new(0.2, 0.5));
        assert!(animator.is_moving());
    }

    #[test]
    fn eases_monotonically_over_600_ms() {
        let clock = Clock::new();
        let mut animator = Animator::new(Progress::new(0.0, 0.0));
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
            assert!(progress.horizontal <= 1.0);
            previous = progress.horizontal;
        }

        let (end, moving) = animator.sample(clock.ms(600));
        assert_eq!(end, Progress::new(1.0, 1.0));
        assert!(!moving, "the animator must come to rest");
        assert!(!animator.is_moving());
    }

    #[test]
    fn retarget_mid_flight_continues_from_the_current_position() {
        let clock = Clock::new();
        let mut animator = Animator::new(Progress::new(0.0, 0.5));
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

        // And it still arrives, 600 ms after the retarget.
        let (end, moving) = animator.sample(clock.ms(900));
        assert_eq!(end, Progress::new(0.2, 0.5));
        assert!(!moving);
    }

    #[test]
    fn sampling_after_rest_needs_no_frames() {
        let clock = Clock::new();
        let mut animator = Animator::new(Progress::CENTER);
        animator.retarget(Progress::new(1.0, 1.0), clock.ms(0));
        let (_, moving) = animator.sample(clock.ms(600));
        assert!(!moving);
        // Idempotent from then on — this is the idle path.
        let (again, moving) = animator.sample(clock.ms(10_000));
        assert_eq!(again, Progress::new(1.0, 1.0));
        assert!(!moving);
    }
}
