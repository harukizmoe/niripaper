//! Wallpaper-transition settings vocabulary: which effect, how it is picked,
//! and the settings block a config file spells. §5 of the shader's contract
//! (the `u_effect` numbering) belongs to these names.
//!
//! The running state machine ([`Transition`](#), effect choice per change,
//! snapshot ownership) stays in the daemon crate; only the value types and the
//! pool choice are shared, because config files name them and the schema and
//! `state` report them.

use std::time::Duration;

use crate::animation::Curve;

/// What the transition does.
///
/// The order is the shader's `u_effect` numbering — changing it means changing
/// `transition_mask` in the daemon's GL module too, which is why `index` is
/// written out rather than derived from the discriminant.
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
    /// The new image arrives slightly magnified and settles.
    Zoom,
    /// The new image slides in, the old one slides out.
    Slide,
}

impl Effect {
    /// Every effect, in shader order. This is the list a panel offers.
    pub const NAMES: [&'static str; 9] = [
        "none", "fade", "dissolve", "wipe", "stripes", "iris", "portal", "zoom", "slide",
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
            Self::Zoom => 7,
            Self::Slide => 8,
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
    /// How far `portal` pushes the old frame outward as the hole opens, as a
    /// multiple of its distance from the centre. `0` makes it an iris.
    pub push: f64,
    /// How big the hole already is when the transition starts, as a fraction of
    /// the screen's height. `0` (the default) begins as a point; anything larger
    /// begins as a complete circle, which is what makes the shape readable from
    /// the first frame.
    pub start_radius: f64,
    /// How long the transition sits on its first frame before it starts moving.
    ///
    /// This is what makes a small circle *readable*: a circle that begins at
    /// `start_radius` and immediately expands is gone before the eye has decided
    /// what it is looking at. Holding it still for a moment is the difference
    /// between "something grew" and "a circle opened". It is part of
    /// `duration_ms`, not extra: the whole transition still lasts that long.
    pub hold: Duration,
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
            // Enough that portal and iris are told apart at a glance. Below about
            // 0.3 the push reads as "a slight zoom" on a photograph.
            push: 0.55,
            // A point, which is what a circle growing from nothing looks like.
            start_radius: 0.0,
            // Nothing: move as soon as it starts.
            hold: Duration::ZERO,
            on_start: true,
        }
    }
}

/// Which effect change number `played` gets.
///
/// Free of the player so the choice can be tested without a GL context: it is
/// the only part of a transition that is worth testing in a unit test.
pub fn choose(
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
    fn settings_default_has_character() {
        let defaults = Settings::default();
        assert_eq!(defaults.selection, Selection::Rotate);
        assert!(defaults.effects.contains(&Effect::Portal));
    }

    #[test]
    fn a_pool_can_list_none_without_breaking_the_round_trip() {
        // `selection = "fixed"` with `effect = "none"` is the documented way to
        // ask for a hard cut; a pool containing `none` is one of the effects.
        let pool = settings(Selection::Rotate, vec![Effect::None, Effect::Fade]);
        assert_eq!(pool.effects.len(), 2);
    }
}
