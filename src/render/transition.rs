//! Runs transitions: picks the effect, animates the progress, owns the
//! snapshot.
//!
//! The settings vocabulary ([`Effect`], [`Selection`], [`Direction`],
//! [`Settings`]) lives in `niripaper-proto::transition`; this module is the
//! player — the only consumer of GL, and therefore daemon-side.

use niripaper_proto::animation::Animation;
pub use niripaper_proto::transition::{choose, Direction, Effect, Selection, Settings};
use std::time::Instant;

use crate::render::anim::Animator;
use crate::render::gl;

pub struct Transition {
    settings: Settings,
    slowdown: f64,
    animation: Animation,
    animator: Animator<f64>,
    /// The effect this change is running, chosen by `restart`.
    effect: Effect,
    /// How many changes have been played: `rotate` walks this.
    played: u64,
    /// xorshift state for `random`. Deterministic on purpose — a bug that only
    /// shows up with one effect in a hundred is worth being able to reproduce.
    random: u64,
    /// Created on the first change: a daemon that never switches wallpapers
    /// never pays for it.
    snapshot: Option<gl::Snapshot>,
}

impl Transition {
    /// Starts settled at 1.0 — "fully the new content". A transition only exists
    /// while the wallpaper is changing.
    pub fn new(settings: Settings, slowdown: f64) -> Self {
        // The hold is part of the duration, so the motion itself is what is left
        // of it. `restart` pushes the animator's clock out by the hold, which is
        // what parks it on the first frame until the hold is over.
        let animation = Animation::easing(
            settings.curve,
            settings.duration.saturating_sub(settings.hold),
        );
        Self {
            settings,
            slowdown,
            animation,
            animator: Animator::new(1.0, animation).with_slowdown(slowdown),
            effect: Effect::None,
            played: 0,
            random: 0x2545_f491_4f6c_dd1d,
            snapshot: None,
        }
    }

    /// Nothing to show: no transition was configured, or the effect is `none`.
    pub fn is_off(&self) -> bool {
        self.animation == Animation::Off
            || (self.settings.selection == Selection::Fixed && self.settings.effect == Effect::None)
    }

    pub fn is_moving(&self) -> bool {
        self.animator.is_moving()
    }

    /// The effect this change is playing, for the log.
    pub fn effect(&self) -> Effect {
        self.effect
    }

    /// Snapshot what is on screen right now and start bringing the new content
    /// in. `capture` is handed the snapshot to fill in; the caller binds the
    /// framebuffer, because `Frame::begin` and `mpv_render_context_render` both
    /// manage bindings and this module must not guess which one is current.
    ///
    /// Pass `None` for `capture` to start from an empty screen instead: that is
    /// what "play one at startup" means, and it is why `Snapshot::new` clears.
    pub fn restart(
        &mut self,
        screen: (u32, u32),
        now: Instant,
        capture: Option<&dyn Fn(&gl::Snapshot)>,
    ) {
        if self.is_off() {
            return;
        }
        self.effect = self.choose();
        self.played += 1;
        if self.effect == Effect::None {
            // One of the pool's effects is "no transition": settle straight away
            // rather than run a fade the user did not ask for.
            self.animator = Animator::new(1.0, self.animation);
            return;
        }
        if self.snapshot.is_none() {
            self.snapshot = Some(match capture {
                // Nothing to fade from: the effect plays against an empty screen.
                None => gl::Snapshot::new_black(screen.0, screen.1),
                Some(_) => gl::Snapshot::new(screen.0, screen.1),
            });
        }
        if let (Some(snapshot), Some(capture)) = (&self.snapshot, capture) {
            capture(snapshot);
        }
        self.animator = Animator::new(0.0, self.animation).with_slowdown(self.slowdown);
        // `elapsed` saturates, so a start time in the future parks the animation
        // at t = 0 until it arrives — and `slowdown` stretches the hold the same
        // way it stretches everything else.
        self.animator
            .retarget(1.0, now + self.settings.hold.mul_f64(self.slowdown));
    }

    /// What the shader needs right now, or `None` when there is nothing to
    /// blend. Returning `None` rather than a "fade = 1.0" sentinel is what keeps
    /// an unbound `u_previous` from ever being sampled.
    pub fn sample(&mut self, now: Instant) -> Option<gl::Blend> {
        if self.is_off() || self.effect == Effect::None || self.snapshot.is_none() {
            return None;
        }
        // Once it has settled there is nothing left to blend: the wallpaper is
        // just the wallpaper. Stopping here also means a mask that ends at 0.999
        // rather than 1.0 cannot leave a residue behind.
        if !self.animator.is_moving() {
            return None;
        }
        let (value, _) = self.animator.sample(now);
        // The raw value, so an overshooting curve reaches the effect; the shader
        // clamps where the *colour* mix needs it, and geometric effects get the
        // bounce.
        let progress = if self.settings.allow_overshoot {
            value
        } else {
            value.clamp(0.0, 1.0)
        };
        Some(gl::Blend {
            previous: self.snapshot.as_ref()?.texture(),
            effect: self.effect.index(),
            progress: progress as f32,
            softness: self.settings.softness as f32,
            center: (self.settings.center.0 as f32, self.settings.center.1 as f32),
            direction: self.settings.direction.vector(),
            stripes: self.settings.stripes as f32,
            push: self.settings.push as f32,
            start_radius: self.settings.start_radius as f32,
        })
    }

    /// Which effect this change gets.
    fn choose(&mut self) -> Effect {
        choose(
            self.settings.selection,
            self.settings.effect,
            &self.settings.effects,
            self.played,
            &mut self.random,
        )
    }
}

/// Which effect change number `played` gets.
///
#[cfg(test)]
mod tests {
    use super::*;
    use niripaper_proto::animation::Curve;
    use std::time::Duration;

    fn settings(selection: Selection, effects: Vec<Effect>) -> Settings {
        Settings {
            selection,
            effect: Effect::Fade,
            effects,
            duration: Duration::from_millis(1500),
            curve: Curve::EaseOutCubic,
            allow_overshoot: false,
            softness: 0.3,
            center: (0.5, 0.5),
            direction: Direction::Right,
            stripes: 12,
            push: 0.55,
            start_radius: 0.0,
            hold: Duration::ZERO,
            on_start: true,
        }
    }

    #[test]
    fn effect_names_round_trip_and_index_matches_the_list() {
        for (index, name) in Effect::NAMES.iter().enumerate() {
            let effect = Effect::parse(name).expect("parses");
            assert_eq!(effect.name(), *name);
            assert_eq!(effect.index() as usize, index, "{name} is out of order");
        }
        assert!(Effect::parse("swirl").is_err());
    }

    #[test]
    fn selection_and_direction_names_round_trip() {
        for name in Selection::NAMES {
            assert_eq!(Selection::parse(name).expect("parses").name(), name);
        }
        for name in Direction::NAMES {
            assert_eq!(Direction::parse(name).expect("parses").name(), name);
        }
        assert!(Selection::parse("sometimes").is_err());
        assert!(Direction::parse("sideways").is_err());
    }

    /// One component is always ±1 and the other 0: that is what makes the sweep
    /// run exactly `0..1` across the screen, whichever way it goes.
    #[test]
    fn directions_are_axis_unit_vectors() {
        for name in Direction::NAMES {
            let (x, y) = Direction::parse(name).expect("parses").vector();
            assert_eq!(x.abs() + y.abs(), 1.0, "{name}");
            assert_eq!(x * y, 0.0, "{name}");
        }
    }

    #[test]
    fn rotate_never_repeats_consecutively() {
        let pool = vec![Effect::Portal, Effect::Iris, Effect::Dissolve];
        let mut random = 1;
        let seen: Vec<Effect> = (0..6)
            .map(|played| choose(Selection::Rotate, Effect::Fade, &pool, played, &mut random))
            .collect();
        assert_eq!(
            seen,
            vec![
                Effect::Portal,
                Effect::Iris,
                Effect::Dissolve,
                Effect::Portal,
                Effect::Iris,
                Effect::Dissolve,
            ]
        );
    }

    #[test]
    fn random_stays_inside_the_pool() {
        let pool = vec![Effect::Portal, Effect::Zoom];
        let mut random = 1;
        let mut seen = [false; 2];
        for played in 0..40 {
            let effect = choose(Selection::Random, Effect::Fade, &pool, played, &mut random);
            seen[match effect {
                Effect::Portal => 0,
                Effect::Zoom => 1,
                other => panic!("{other:?} is not in the pool"),
            }] = true;
        }
        assert!(seen[0] && seen[1], "both should come up over 40 draws");
    }

    #[test]
    fn fixed_ignores_the_pool() {
        let mut random = 1;
        assert_eq!(
            choose(
                Selection::Fixed,
                Effect::Wipe,
                &[Effect::Portal],
                7,
                &mut random
            ),
            Effect::Wipe
        );
    }

    #[test]
    fn none_and_off_have_nothing_to_blend() {
        // `selection = "fixed"` with `effect = "none"` is the documented way to
        // ask for a hard cut.
        let mut off = settings(Selection::Fixed, Vec::new());
        off.effect = Effect::None;
        let mut transition = Transition::new(off, 1.0);
        assert!(transition.is_off());
        assert!(transition.sample(Instant::now()).is_none());

        // With a pool to draw from, `none` is just one of the effects and the
        // transition runs.
        let mut pool = settings(Selection::Rotate, vec![Effect::None, Effect::Fade]);
        pool.effect = Effect::None;
        let transition = Transition::new(pool, 1.0);
        assert!(!transition.is_off(), "the pool decides, not `effect`");
    }

    /// A settled transition must not hand the shader a snapshot to sample: that
    /// is the bug where an unbound `u_previous` reads undefined memory.
    #[test]
    fn a_fresh_transition_has_nothing_to_blend() {
        let mut transition = Transition::new(settings(Selection::Fixed, Vec::new()), 1.0);
        assert!(transition.sample(Instant::now()).is_none());
    }
}
