//! NV+'s own player settings: the look feel, from the `NV_MOUSE_*` variables and the run's
//! `Fallout.ini`.

pub mod ini;
pub mod mouse;

use mouse::Feel;

/// The settings bound before the game's first frame.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Options {
    pub mouse: Feel,
    /// One line naming `Fallout.ini` values that couldn't be used; the default stood for each.
    pub note: Option<String>,
}

// Every value is finite, checked on read.
impl Eq for Options {}

impl Options {
    /// The options the run asked for: each setting is its variable when set, else `game_ini`'s
    /// `[nv-rs Mouse]` key, else the default.
    pub fn read(
        game_ini: Option<&str>,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Options, String> {
        let (mouse, bad) = mouse::read(&env, game_ini)?;
        let note = (!bad.is_empty()).then(|| {
            format!(
                "Fallout.ini's [{}]: not used, the default instead: {}",
                mouse::SECTION,
                bad.join(", ")
            )
        });
        Ok(Options { mouse, note })
    }

    /// The start-up line's mouse part: `; ` clauses for what is on, empty when off.
    pub fn describe(&self) -> String {
        let mut line = String::new();
        if self.mouse.smoothing_ms > 0.0 {
            line.push_str(&format!("; mouse smoothing {} ms", self.mouse.smoothing_ms));
        }
        if let Some(natural) = self.mouse.accel {
            line.push_str(&format!(
                "; mouse acceleration limit {} ramp {}",
                natural.limit, natural.ramp
            ));
        }
        line
    }
}

/// The mouse variables `fallout_ini`'s `[nv-rs Mouse]` keys set to a value the game takes, so a
/// bad file value never becomes a variable that stops the start.
pub fn file_switches(fallout_ini: &str) -> Vec<(&'static str, Option<String>)> {
    let get = |key: &str| {
        ini::value(fallout_ini, mouse::SECTION, key)
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };
    let checks: [fn(&str) -> bool; 4] = [
        |v| mouse::smoothing(v).is_some(),
        |v| mouse::accel(v).is_some(),
        |v| mouse::accel_limit(v).is_some(),
        |v| mouse::accel_ramp(v).is_some(),
    ];
    let mut out = Vec::new();
    for ((key, switch, _), check) in mouse::MOUSE_KEYS.iter().zip(checks) {
        if let Some(value) = get(key).filter(|v| check(v)) {
            out.push((*switch, Some(value.to_string())));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mouse::Natural;

    fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| {
            pairs
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn nothing_set_describes_nothing() {
        let none = |_: &str| None;
        let options = Options::read(None, none).unwrap();
        assert_eq!(options.describe(), "");
        assert_eq!(options.note, None);
    }

    /// The variable wins over the file's key, key by key, so the file's preset can take its ramp
    /// from the environment; with no file the variables alone decide.
    #[test]
    fn a_variable_wins_over_the_files_key() {
        let ini = "[nv-rs Mouse]\nsSmoothing=Light\nsAcceleration=Medium\n";
        let from_file = Options::read(Some(ini), env_of(&[])).unwrap();
        assert_eq!(
            from_file.mouse,
            Feel {
                smoothing_ms: 20.0,
                accel: Some(Natural {
                    limit: 1.5,
                    ramp: 0.1
                }),
            }
        );
        let both = Options::read(Some(ini), env_of(&[("NV_MOUSE_SMOOTHING", "Strong")])).unwrap();
        assert_eq!(both.mouse.smoothing_ms, 50.0);
        assert_eq!(
            both.mouse.accel,
            Some(Natural {
                limit: 1.5,
                ramp: 0.1
            })
        );
        let tuned = Options::read(Some(ini), env_of(&[("NV_MOUSE_ACCEL_RAMP", "0.5")])).unwrap();
        assert_eq!(
            tuned.mouse.accel,
            Some(Natural {
                limit: 1.5,
                ramp: 0.5
            })
        );
        let limited =
            Options::read(Some(&format!("{ini}fAccelerationLimit=3.5\n")), env_of(&[])).unwrap();
        assert_eq!(limited.mouse.accel.map(|n| n.limit), Some(3.5));
        let variable_only = Options::read(None, env_of(&[("NV_MOUSE_ACCEL", "Light")])).unwrap();
        assert_eq!(
            variable_only.mouse.accel,
            Some(Natural {
                limit: 1.25,
                ramp: 0.1
            })
        );
    }

    /// A file value that can't be used leaves the default and is named once; the start goes on.
    #[test]
    fn a_bad_file_value_keeps_the_default_and_is_named() {
        let options = Options::read(Some("[nv-rs Mouse]\nsSmoothing=fast\n"), env_of(&[])).unwrap();
        assert_eq!(options.mouse, Feel::default());
        assert_eq!(
            options.note.as_deref(),
            Some("Fallout.ini's [nv-rs Mouse]: not used, the default instead: sSmoothing=fast")
        );
        let two = Options::read(
            Some("[nv-rs Mouse]\nsAcceleration=hard\nfAccelerationRamp=0\n"),
            env_of(&[]),
        )
        .unwrap();
        assert_eq!(
            two.note.as_deref(),
            Some(
                "Fallout.ini's [nv-rs Mouse]: not used, the default instead: \
                 sAcceleration=hard, fAccelerationRamp=0"
            )
        );
    }

    #[test]
    fn no_file_means_the_file_is_ignored() {
        let options = Options::read(None, env_of(&[])).unwrap();
        assert_eq!(options.mouse, Feel::default());
        assert_eq!(options.note, None);
    }

    /// `distribution/nv-plus-sections.ini` holds exactly `MOUSE_KEYS`: each key once, its comment
    /// on the line above it, an empty value, nothing else.
    #[test]
    fn the_distribution_file_is_the_mouse_keys() {
        let text = include_str!("../../../distribution/nv-plus-sections.ini");
        let lines: Vec<&str> = text.lines().collect();
        let mut keys = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if line.is_empty() || line.starts_with(';') || line.starts_with('[') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (_, _, comment) = mouse::MOUSE_KEYS
                .iter()
                .find(|(k, _, _)| *k == key)
                .unwrap_or_else(|| panic!("line {}: {line} is not in MOUSE_KEYS", i + 1));
            assert_eq!(value, "", "{line}");
            assert_eq!(
                lines[i - 1],
                format!("; {comment}"),
                "the comment above {key}"
            );
            keys.push(key);
        }
        assert_eq!(
            keys,
            mouse::MOUSE_KEYS
                .iter()
                .map(|(k, _, _)| *k)
                .collect::<Vec<_>>()
        );
        let header = format!("[{}]", mouse::SECTION);
        assert_eq!(
            lines.iter().filter(|l| **l == header).count(),
            1,
            "{header}"
        );
    }

    #[test]
    fn file_switches_pass_the_values_the_game_takes_and_drop_the_rest() {
        let some = |v: &str| Some(v.to_string());
        let ini = "[nv-rs Mouse]\nsSmoothing=Medium\nsAcceleration=light\n\
                   fAccelerationLimit=3.5\nfAccelerationRamp=0.25\n";
        assert_eq!(
            file_switches(ini),
            [
                ("NV_MOUSE_SMOOTHING", some("Medium")),
                ("NV_MOUSE_ACCEL", some("light")),
                ("NV_MOUSE_ACCEL_LIMIT", some("3.5")),
                ("NV_MOUSE_ACCEL_RAMP", some("0.25")),
            ]
        );
        let bad = "[nv-rs Mouse]\nsSmoothing=fast\nsAcceleration=hard\n\
                   fAccelerationLimit=5\nfAccelerationRamp=0\n";
        assert!(file_switches(bad).is_empty());
        let empty = "[nv-rs Mouse]\nsSmoothing=\nsAcceleration=\n\
                     fAccelerationLimit=\nfAccelerationRamp=\n";
        assert!(file_switches(empty).is_empty());
    }

    #[test]
    fn the_line_names_the_mouse_feel_when_it_is_on() {
        let smooth = Options::read(None, env_of(&[("NV_MOUSE_SMOOTHING", "Medium")])).unwrap();
        assert_eq!(smooth.describe(), "; mouse smoothing 35 ms");
        let accel = Options::read(None, env_of(&[("NV_MOUSE_ACCEL", "Light")])).unwrap();
        assert_eq!(accel.describe(), "; mouse acceleration limit 1.25 ramp 0.1");
        let both = Options::read(
            None,
            env_of(&[("NV_MOUSE_SMOOTHING", "7"), ("NV_MOUSE_ACCEL", "Strong")]),
        )
        .unwrap();
        assert_eq!(
            both.describe(),
            "; mouse smoothing 7 ms; mouse acceleration limit 2 ramp 0.1"
        );
        assert_eq!(
            both.mouse,
            Feel {
                smoothing_ms: 7.0,
                accel: Some(Natural {
                    limit: 2.0,
                    ramp: 0.1
                }),
            }
        );
    }
}
