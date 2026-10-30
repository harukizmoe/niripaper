//! niri's animation vocabulary: curves, springs, and the animation types a
//! config file spells (`off` / easing / spring). §4.2.6.
//!
//! The frame-callback-driven [`Animator` state machine](#) lives in the daemon
//! crate (`render::anim`); only the value types and their maths are shared,
//! because config files name them and the schema reports them.

use std::time::Duration;

/// §4.2.6: `durations.large` = 600 ms.
pub const DEFAULT_DURATION: Duration = Duration::from_millis(600);

/// niri's easing curves (`curve "…"`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Curve {
    Linear,
    EaseOutQuad,
    EaseOutCubic,
    EaseOutExpo,
    /// CSS `cubic-bezier(x1, y1, x2, y2)`.
    CubicBezier([f64; 4]),
}

impl Curve {
    /// niri's five names; the last one takes four control points.
    pub const NAMES: [&'static str; 5] = [
        "linear",
        "ease-out-quad",
        "ease-out-cubic",
        "ease-out-expo",
        "cubic-bezier",
    ];

    pub fn parse(name: &str, control_points: Option<[f64; 4]>) -> Result<Self, String> {
        Ok(match name {
            "linear" => Self::Linear,
            "ease-out-quad" => Self::EaseOutQuad,
            "ease-out-cubic" => Self::EaseOutCubic,
            "ease-out-expo" => Self::EaseOutExpo,
            "cubic-bezier" => {
                let points = control_points.ok_or(
                    "curve \"cubic-bezier\" needs four control points, e.g. [0.05, 0.7, 0.1, 1]",
                )?;
                if points.iter().any(|p| !p.is_finite()) {
                    return Err(format!(
                        "cubic-bezier control points must be finite: {points:?}"
                    ));
                }
                Self::CubicBezier(points)
            }
            other => {
                return Err(format!(
                    "unknown curve {other:?}; niri's curves are: {}",
                    Self::NAMES.join(", ")
                ))
            }
        })
    }

    /// The config-file spelling — the inverse of `parse`, indexed off the same
    /// table so the two cannot drift (a bezier is always spelled `cubic-bezier`;
    /// its control points live beside it).
    pub fn name(&self) -> &'static str {
        match self {
            Self::Linear => Self::NAMES[0],
            Self::EaseOutQuad => Self::NAMES[1],
            Self::EaseOutCubic => Self::NAMES[2],
            Self::EaseOutExpo => Self::NAMES[3],
            Self::CubicBezier(_) => Self::NAMES[4],
        }
    }

    pub fn eval(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseOutQuad => 1.0 - (1.0 - t).powi(2),
            Self::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Self::EaseOutExpo => {
                if t >= 1.0 {
                    1.0
                } else {
                    1.0 - 2f64.powf(-10.0 * t)
                }
            }
            Self::CubicBezier(points) => cubic_bezier(*points, t),
        }
    }
}

/// `Easing.OutCubic` — kept as a named helper because §4.2.6 specifies it.
pub fn out_cubic(t: f64) -> f64 {
    Curve::EaseOutCubic.eval(t)
}

/// CSS `cubic-bezier(x1, y1, x2, y2)`: solve `x(u) = t` for `u`, then return
/// `y(u)`. The curve is defined by the two control points with (0,0) and (1,1)
/// fixed, exactly like the CSS timing function niri copies.
fn cubic_bezier(points: [f64; 4], t: f64) -> f64 {
    let [x1, y1, x2, y2] = points;
    let bezier = |a: f64, b: f64, u: f64| {
        // Bernstein form of a cubic with P0 = 0 and P3 = 1.
        let v = 1.0 - u;
        3.0 * v * v * u * a + 3.0 * v * u * u * b + u * u * u
    };
    let slope = |a: f64, b: f64, u: f64| {
        let v = 1.0 - u;
        3.0 * v * v * a + 6.0 * v * u * (b - a) + 3.0 * u * u * (1.0 - b)
    };

    // Newton first (fast), bisection as a fallback for the flat parts.
    let mut u = t;
    for _ in 0..8 {
        let error = bezier(x1, x2, u) - t;
        if error.abs() < 1e-9 {
            return bezier(y1, y2, u);
        }
        let derivative = slope(x1, x2, u);
        if derivative.abs() < 1e-6 {
            break;
        }
        u -= error / derivative;
    }
    let (mut low, mut high) = (0.0, 1.0);
    let mut u = t;
    for _ in 0..32 {
        let x = bezier(x1, x2, u);
        if (x - t).abs() < 1e-9 {
            break;
        }
        if x < t {
            low = u;
        } else {
            high = u;
        }
        u = 0.5 * (low + high);
    }
    bezier(y1, y2, u)
}

/// niri's spring: mass is hardcoded to 1, so the angular frequency is
/// `sqrt(stiffness)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    pub damping_ratio: f64,
    pub stiffness: f64,
    pub epsilon: f64,
}

impl Spring {
    /// Normalized progress of a unit spring released from rest: 0 at `t = 0`,
    /// tending to 1. Closed form, so nothing accumulates per frame.
    pub fn eval(&self, seconds: f64) -> f64 {
        let zeta = self.damping_ratio;
        let omega = self.stiffness.sqrt();
        let t = seconds.max(0.0);
        if zeta < 1.0 {
            let damped = omega * (1.0 - zeta * zeta).sqrt();
            let decay = (-zeta * omega * t).exp();
            1.0 - decay * ((damped * t).cos() + (zeta * omega / damped) * (damped * t).sin())
        } else if (zeta - 1.0).abs() < 1e-9 {
            let decay = (-omega * t).exp();
            1.0 - decay * (1.0 + omega * t)
        } else {
            // Overdamped: two real roots. niri warns against damping-ratio > 1.
            let root = omega * (zeta * zeta - 1.0).sqrt();
            let (r1, r2) = (-zeta * omega + root, -zeta * omega - root);
            1.0 - (r2 * (r1 * t).exp() - r1 * (r2 * t).exp()) / (r2 - r1)
        }
    }

    /// The decaying envelope of `|value - target|`, normalized to a unit step.
    ///
    /// The instantaneous error is **not** usable here: an underdamped spring
    /// crosses its target every half period, so `|1 - eval(t)|` dips to zero at
    /// the first crossing and the animation would be declared finished before
    /// the overshoot it exists to produce. The envelope decays monotonically.
    pub fn envelope(&self, seconds: f64) -> f64 {
        let zeta = self.damping_ratio;
        let omega = self.stiffness.sqrt();
        let t = seconds.max(0.0);
        if zeta < 1.0 {
            (-zeta * omega * t).exp() / (1.0 - zeta * zeta).sqrt()
        } else if (zeta - 1.0).abs() < 1e-9 {
            (-omega * t).exp() * (1.0 + omega * t)
        } else {
            // Overdamped: `eval` approaches 1 monotonically, so it is already
            // an envelope.
            (1.0 - self.eval(t)).abs()
        }
    }

    /// Settled once the remaining distance is within `epsilon` — niri's
    /// "set epsilon to a lower value if the animation jumps at the end".
    pub fn settled(&self, seconds: f64, distance: f64) -> bool {
        self.envelope(seconds) * distance <= self.epsilon
    }
}

/// One of niri's animation types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Animation {
    /// niri's `off`: jump straight to the target.
    Off,
    Easing {
        duration: Duration,
        curve: Curve,
    },
    Spring(Spring),
}

impl Animation {
    pub fn easing(curve: Curve, duration: Duration) -> Self {
        Self::Easing { duration, curve }
    }

    pub fn spring(damping_ratio: f64, stiffness: f64, epsilon: f64) -> Self {
        Self::Spring(Spring {
            damping_ratio,
            stiffness,
            epsilon,
        })
    }

    /// One-line summary for the startup log: users tune these by feel, so they
    /// need to see what is actually in effect.
    pub fn describe(&self) -> String {
        match self {
            Self::Off => "off".to_owned(),
            Self::Easing { duration, curve } => {
                format!("{} {} ms", curve.name(), duration.as_millis())
            }
            Self::Spring(spring) => format!(
                "spring {}/{}/{}",
                spring.damping_ratio, spring.stiffness, spring.epsilon
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
