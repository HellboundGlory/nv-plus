//! The look feel: smoothing and acceleration, an NV+ option and not behaviour of the original
//! game (no exe address).
//!
//! Smoothing spreads a frame's hand motion evenly over a short window, so the turn is the same at
//! any frame rate. Acceleration turns faster hand movement further, rising smoothly to a limit.

use crate::ini;

/// The feel the player asked for; both parts off is the original's behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Feel {
    pub smoothing_ms: f64,
    pub accel: Option<Natural>,
}

impl Feel {
    /// Both parts off: the original's behaviour.
    pub fn is_off(&self) -> bool {
        self.smoothing_ms == 0.0 && self.accel.is_none()
    }
}

/// A turn that grows with the hand's speed towards `limit` counts per count, `ramp` where it rises.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Natural {
    pub limit: f64,
    pub ramp: f64,
}

/// The `Fallout.ini` section the mouse keys live in.
pub const SECTION: &str = "nv-rs Mouse";

/// The mouse keys: key, the variable that wins over it, and the one `;` line above it in the
/// generated file.
pub const MOUSE_KEYS: &[(&str, &str, &str)] = &[
    (
        "sSmoothing",
        "NV_MOUSE_SMOOTHING",
        "Off, Light (20 ms), Medium (35 ms), Strong (50 ms), or milliseconds from 0 to 100: each frame's look movement is spread over that time.",
    ),
    (
        "sAcceleration",
        "NV_MOUSE_ACCEL",
        "Off, Light, Medium or Strong: faster hand movement turns further, up to 1.25, 1.5 or 2 times.",
    ),
    (
        "fAccelerationLimit",
        "NV_MOUSE_ACCEL_LIMIT",
        "Empty = sAcceleration's; else 1 to 4 times at most.",
    ),
    (
        "fAccelerationRamp",
        "NV_MOUSE_ACCEL_RAMP",
        "Empty = 0.1; else above 0 and at most 10: how soon the turn rises to its limit.",
    ),
];

/// The smoothing a word or a number of milliseconds names: Off, Light (20 ms), Medium (35 ms),
/// Strong (50 ms), or 0 to 100. Compared without case and trimmed.
pub fn smoothing(word: &str) -> Option<f64> {
    match word.trim().to_ascii_lowercase().as_str() {
        "off" => Some(0.0),
        "light" => Some(20.0),
        "medium" => Some(35.0),
        "strong" => Some(50.0),
        _ => word
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|ms| (0.0..=100.0).contains(ms)),
    }
}

/// The acceleration limit a word names: Off (none), Light (1.25), Medium (1.5), Strong (2).
pub fn accel(word: &str) -> Option<Option<f64>> {
    match word.trim().to_ascii_lowercase().as_str() {
        "off" => Some(None),
        "light" => Some(Some(1.25)),
        "medium" => Some(Some(1.5)),
        "strong" => Some(Some(2.0)),
        _ => None,
    }
}

/// An acceleration limit, 1 to 4 times at most.
pub fn accel_limit(word: &str) -> Option<f64> {
    word.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| (1.0..=4.0).contains(n))
}

/// An acceleration ramp, above 0 and at most 10.
pub fn accel_ramp(word: &str) -> Option<f64> {
    word.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| *n > 0.0 && *n <= 10.0)
}

/// The look feel of the `NV_MOUSE_*` variables, each winning over `Fallout.ini`'s `[nv-rs Mouse]`
/// key of the same name. An empty value is unset.
pub fn from_env(env: &impl Fn(&str) -> Option<String>) -> Result<Feel, String> {
    read(env, None).map(|(feel, _)| feel)
}

/// The feel and the `key=value` lines of the file it couldn't use. A variable that can't be read
/// is an error (the start stops); a file value that can't be used leaves the default.
pub fn read(
    env: &impl Fn(&str) -> Option<String>,
    ini: Option<&str>,
) -> Result<(Feel, Vec<String>), String> {
    let variable = |name: &str| {
        env(name)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let file = |key: &str| {
        ini.and_then(|text| ini::value(text, SECTION, key))
            .filter(|v| !v.is_empty())
    };
    let mut bad = Vec::new();

    let smoothing_ms = match variable("NV_MOUSE_SMOOTHING") {
        Some(value) => smoothing(&value).ok_or_else(|| {
            format!(
                "NV_MOUSE_SMOOTHING={value}: use Off, Light, Medium, Strong or milliseconds from 0 to 100"
            )
        })?,
        None => match file("sSmoothing") {
            Some(value) => smoothing(value).unwrap_or_else(|| {
                bad.push(format!("sSmoothing={value}"));
                0.0
            }),
            None => 0.0,
        },
    };
    let preset_limit = match variable("NV_MOUSE_ACCEL") {
        Some(value) => accel(&value)
            .ok_or_else(|| format!("NV_MOUSE_ACCEL={value}: use Off, Light, Medium or Strong"))?,
        None => match file("sAcceleration") {
            Some(value) => accel(value).unwrap_or_else(|| {
                bad.push(format!("sAcceleration={value}"));
                None
            }),
            None => None,
        },
    };
    // Checked even with acceleration off: Settings.cmd keeps these lines when it is turned off there.
    let limit = match variable("NV_MOUSE_ACCEL_LIMIT") {
        Some(value) => Some(
            accel_limit(&value)
                .ok_or_else(|| format!("NV_MOUSE_ACCEL_LIMIT={value}: use a number from 1 to 4"))?,
        ),
        None => match file("fAccelerationLimit") {
            Some(value) => accel_limit(value).or_else(|| {
                bad.push(format!("fAccelerationLimit={value}"));
                None
            }),
            None => None,
        },
    };
    let ramp = match variable("NV_MOUSE_ACCEL_RAMP") {
        Some(value) => Some(accel_ramp(&value).ok_or_else(|| {
            format!("NV_MOUSE_ACCEL_RAMP={value}: use a number above 0 and at most 10")
        })?),
        None => match file("fAccelerationRamp") {
            Some(value) => accel_ramp(value).or_else(|| {
                bad.push(format!("fAccelerationRamp={value}"));
                None
            }),
            None => None,
        },
    };
    let accel = preset_limit.map(|preset| Natural {
        limit: limit.unwrap_or(preset),
        ramp: ramp.unwrap_or(0.1),
    });
    Ok((
        Feel {
            smoothing_ms,
            accel,
        },
        bad,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| {
            pairs
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn smoothing_words_and_numbers_are_milliseconds() {
        for (word, ms) in [
            ("Off", Some(0.0)),
            ("light", Some(20.0)),
            (" Medium ", Some(35.0)),
            ("STRONG", Some(50.0)),
            ("7", Some(7.0)),
            ("0", Some(0.0)),
            ("100", Some(100.0)),
            ("2.5", Some(2.5)),
            ("fast", None),
            ("-1", None),
            ("101", None),
            ("", None),
        ] {
            assert_eq!(smoothing(word), ms, "{word:?}");
        }
    }

    #[test]
    fn an_acceleration_word_is_an_optional_limit() {
        for (word, limit) in [
            ("Off", Some(None)),
            ("Light", Some(Some(1.25))),
            ("medium", Some(Some(1.5))),
            ("STRONG", Some(Some(2.0))),
            ("hard", None),
            ("3", None),
            ("", None),
        ] {
            assert_eq!(accel(word), limit, "{word:?}");
        }
    }

    #[test]
    fn the_acceleration_limit_and_ramp_ranges_hold() {
        for (word, limit) in [
            ("1", Some(1.0)),
            ("4", Some(4.0)),
            ("3.5", Some(3.5)),
            ("0.5", None),
            ("5", None),
            ("wide", None),
        ] {
            assert_eq!(accel_limit(word), limit, "{word:?}");
        }
        for (word, ramp) in [
            ("0.1", Some(0.1)),
            ("10", Some(10.0)),
            ("0", None),
            ("11", None),
            ("fast", None),
        ] {
            assert_eq!(accel_ramp(word), ramp, "{word:?}");
        }
    }

    #[test]
    fn an_empty_or_missing_variable_leaves_the_default() {
        let none = |_: &str| None;
        assert_eq!(from_env(&none).unwrap(), Feel::default());
        assert_eq!(
            from_env(&env_of(&[("NV_MOUSE_SMOOTHING", "")])).unwrap(),
            Feel::default()
        );
    }

    #[test]
    fn the_acceleration_numbers_tune_the_preset() {
        let tuned = from_env(&env_of(&[
            ("NV_MOUSE_ACCEL", "Strong"),
            ("NV_MOUSE_ACCEL_LIMIT", "3.5"),
            ("NV_MOUSE_ACCEL_RAMP", "0.25"),
        ]))
        .unwrap();
        assert_eq!(
            tuned.accel,
            Some(Natural {
                limit: 3.5,
                ramp: 0.25
            })
        );
        let preset = from_env(&env_of(&[("NV_MOUSE_ACCEL", "Light")])).unwrap();
        assert_eq!(
            preset.accel,
            Some(Natural {
                limit: 1.25,
                ramp: 0.1
            })
        );
    }

    #[test]
    fn a_mouse_switch_that_cant_be_read_stops_the_start() {
        for (name, value) in [("NV_MOUSE_SMOOTHING", "fast"), ("NV_MOUSE_ACCEL", "hard")] {
            let pairs: &'static [(&'static str, &'static str)] =
                Box::leak(Box::new([(name, value)]));
            let err = from_env(&env_of(pairs)).unwrap_err();
            assert!(err.starts_with(&format!("{name}={value}")), "{err}");
        }
        for (name, value) in [("NV_MOUSE_ACCEL_LIMIT", "5"), ("NV_MOUSE_ACCEL_RAMP", "0")] {
            let pairs: &'static [(&'static str, &'static str)] =
                Box::leak(Box::new([("NV_MOUSE_ACCEL", "Medium"), (name, value)]));
            let err = from_env(&env_of(pairs)).unwrap_err();
            assert!(err.starts_with(&format!("{name}={value}")), "{err}");
        }
    }

    /// The limit and ramp are checked even with acceleration off: Settings.cmd keeps hand-written
    /// lines when it is turned off there.
    #[test]
    fn the_accel_numbers_do_nothing_while_accel_is_off() {
        for pairs in [
            vec![("NV_MOUSE_ACCEL_LIMIT", "1.8")],
            vec![("NV_MOUSE_ACCEL_RAMP", "0.2")],
            vec![("NV_MOUSE_ACCEL", "Off"), ("NV_MOUSE_ACCEL_LIMIT", "1.8")],
            vec![("NV_MOUSE_ACCEL", ""), ("NV_MOUSE_ACCEL_RAMP", "0.2")],
        ] {
            let pairs: &'static [(&'static str, &'static str)] =
                Box::leak(pairs.into_boxed_slice());
            assert_eq!(from_env(&env_of(pairs)).unwrap().accel, None, "{pairs:?}");
        }
        let err = from_env(&env_of(&[("NV_MOUSE_ACCEL_LIMIT", "9")])).unwrap_err();
        assert!(
            err.contains("NV_MOUSE_ACCEL_LIMIT=9: use a number from 1 to 4"),
            "{err}"
        );
    }
}
