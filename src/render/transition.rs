//! The transition when the wallpaper changes (`HANDOFF.md` §2, M2's last piece).
//!
//! The snapshot is of the **rendered** frame, not of the source: that is what
//! makes any pair of sources blend identically (still ↔ video ↔ still), keeps
//! only one decoder alive, and means the parallax and overview zoom are already
//! baked into what transitions out — no special cases for any of them.
//!
//! That is also what shapes every effect here: **the old side is a frozen
//! photograph and the new side is live**. Nothing may animate the old side, and
//! nothing may assume the new side holds still. What the effects are for is
//! exactly that asymmetry — a video arriving through a portal that is already
//! moving is something a plain cross-fade cannot show.
//!
//! Every effect is one fragment pass: a mask, a pair of lookups, or both. The
//! fanciest costs the same as the plainest, which is what makes 1500 ms of it
//! affordable.

use std::time::{Duration, Instant};

use crate::render::anim::{Animation, Animator, Curve};
use crate::render::gl;

/// What the transition does.
///
/// The order is the shader's `u_effect` numbering — changing it means changing
/// `transition_mask` in `gl.rs` too, which is why `index` is written out rather
/// than derived from the discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// No transition at all: the new wallpaper simply appears.
    None,
    /// A plain cross-fade, with no shape to it.
    Fade,
    /// Grains of the new image appear all over at once.
    Dissolve,
    /// A straight edge sweeps across.
    Wipe,
    /// The same edge, broken into bands that go one after another.
    Stripes,
    /// A circle growing from `center`.
    Iris,
    /// Like an iris, but the old frame is pushed outward as the hole opens: you
    /// move *through* it rather than watch it get cut away.
    Portal,
    /// Hexagonal cells, each opening in its own order.
    Honeycomb,
    /// The new image arrives slightly magnified and settles.
    Zoom,
    /// The new image slides in, the old one slides out.
    Slide,
}

impl Effect {
    /// Every effect, in shader order. This is the list a panel offers.
    pub const NAMES: [&'static str; 10] = [
        "none",
        "fade",
        "dissolve",
        "wipe",
        "stripes",
        "iris",
        "portal",
        "honeycomb",
        "zoom",
        "slide",
    ];

    pub fn name(self) -> &'static str {
        Self::NAMES[self.index() as usize]
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        Ok(match name {
            "none" => Self::None,
            "fade" => Self::Fade,
            "dissolve" => Self::Dissolve,
            "wipe" => Self::Wipe,
            "stripes" => Self::Stripes,
            "iris" => Self::Iris,
            "portal" => Self::Portal,
            "honeycomb" => Self::Honeycomb,
            "zoom" => Self::Zoom,
            "slide" => Self::Slide,
            other => {
                return Err(format!(
                    "unknown effect {other:?}; the effects are: {}",
                    Self::NAMES.join(", ")
                ))
            }
        })
    }

    /// The number the shader switches on.
    pub fn index(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Fade => 1,
            Self::Dissolve => 2,
            Self::Wipe => 3,
            Self::Stripes => 4,
            Self::Iris => 5,
            Self::Portal => 6,
            Self::Honeycomb => 7,
            Self::Zoom => 8,
            Self::Slide => 9,
        }
    }
}

/// How the effect is chosen for each change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    /// Always `effect`.
    Fixed,
    /// Round-robin through `effects`, so consecutive changes never repeat.
    Rotate,
    /// Pick from `effects` at random.
    Random,
}

impl Selection {
    pub const NAMES: [&'static str; 3] = ["fixed", "rotate", "random"];

    pub fn name(self) -> &'static str {
        match self {
            Self::Fixed => Self::NAMES[0],
            Self::Rotate => Self::NAMES[1],
            Self::Random => Self::NAMES[2],
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "fixed" => Ok(Self::Fixed),
            "rotate" => Ok(Self::Rotate),
            "random" => Ok(Self::Random),
            other => Err(format!(
                "unknown selection {other:?}; the choices are: {}",
                Self::NAMES.join(", ")
            )),
        }
    }
}

/// Which way a wipe, a set of stripes or a slide goes: the direction the edge
/// travels, and the direction the incoming image comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    pub const NAMES: [&'static str; 4] = ["left", "right", "up", "down"];

    pub fn name(self) -> &'static str {
        match self {
            Self::Left => Self::NAMES[0],
            Self::Right => Self::NAMES[1],
            Self::Up => Self::NAMES[2],
            Self::Down => Self::NAMES[3],
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            "up" => Ok(Self::Up),
            "down" => Ok(Self::Down),
            other => Err(format!(
                "unknown direction {other:?}; the choices are: {}",
                Self::NAMES.join(", ")
            )),
        }
    }

    /// The unit vector the shader projects onto: one component is always ±1 and
    /// the other 0, which is what keeps the sweep exactly `0..1` across the
    /// screen.
    pub fn vector(self) -> (f32, f32) {
        match self {
            Self::Left => (-1.0, 0.0),
            Self::Right => (1.0, 0.0),
            Self::Up => (0.0, -1.0),
            Self::Down => (0.0, 1.0),
        }
    }
}

/// The settings a transition runs with, as validated from the config.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub selection: Selection,
    /// Used when `selection = "fixed"`.
    pub effect: Effect,
    /// The pool `rotate` and `random` draw from.
    pub effects: Vec<Effect>,
    pub duration: Duration,
    pub curve: Curve,
    /// Whether the curve may go past 1 (and below 0). Off by default: a bounce
    /// is a decision, not a side effect of a bezier's control points.
    pub allow_overshoot: bool,
    /// How wide the moving edge is, as a fraction of the screen. 0 is a hard
    /// edge, 1 is very soft.
    pub softness: f64,
    /// Where radial effects start, as a fraction of the screen.
    pub center: (f64, f64),
    pub direction: Direction,
    /// How many bands `stripes` breaks the edge into.
    pub stripes: u32,
    /// How big a `honeycomb` cell is, as a fraction of the screen's height.
    pub cell: f64,
    /// How far `portal` pushes the old frame outward as the hole opens, as a
    /// multiple of its distance from the centre. `0` makes it an iris.
    pub push: f64,
    /// Play one when the daemon starts, so logging in is not a hard cut.
    pub on_start: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // The default has to have some character of its own — a plain fade is
            // what every other wallpaper tool does. Portal and iris speak the
            // compositor's own depth language, and dissolve is the quiet one.
            selection: Selection::Rotate,
            effect: Effect::Fade,
            effects: vec![Effect::Portal, Effect::Iris, Effect::Dissolve],
            duration: Duration::from_millis(1500),
            curve: Curve::EaseOutCubic,
            allow_overshoot: false,
            softness: 0.3,
            center: (0.5, 0.5),
            direction: Direction::Right,
            stripes: 12,
            // A hexagon radius, as a fraction of the screen height: 0.05 is about
            // thirteen rows on a 1440-tall screen, which reads as a honeycomb
            // rather than as a handful of tiles.
            cell: 0.05,
            // Enough that portal and iris are told apart at a glance. Below about
            // 0.3 the push reads as "a slight zoom" on a photograph.
            push: 0.55,
            on_start: true,
        }
    }
}

/// Runs transitions: picks the effect, animates the progress, owns the snapshot.
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
        let animation = Animation::easing(settings.curve, settings.duration);
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
        self.animator.retarget(1.0, now);
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
            params: (self.settings.stripes as f32, self.settings.cell as f32),
            push: self.settings.push as f32,
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
/// Free of the player so the choice can be tested without a GL context: it is
/// the only part of a transition that is worth testing in a unit test.
fn choose(
    selection: Selection,
    fixed: Effect,
    pool: &[Effect],
    played: u64,
    random: &mut u64,
) -> Effect {
    match selection {
        Selection::Fixed => fixed,
        // The pool is never empty: the config refuses an empty one when the
        // selection needs it.
        Selection::Rotate => pool[played as usize % pool.len()],
        Selection::Random => pool[(next_random(random) % pool.len() as u64) as usize],
    }
}

/// xorshift64*, so `random` needs no dependency and is reproducible from the
/// seed when something needs debugging.
fn next_random(state: &mut u64) -> u64 {
    let mut value = *state;
    value ^= value >> 12;
    value ^= value << 25;
    value ^= value >> 27;
    *state = value;
    value.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            cell: 0.12,
            push: 0.55,
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
