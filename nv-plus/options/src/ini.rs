//! The run's `Fallout.ini`, read the way the game's own profile call finds a key.

/// The value of `key` in `section` of the INI `text`: every section of that name is looked in,
/// names compared without case, spaces around names and values trimmed, one pair of surrounding
/// quotes taken off; the first match wins. A `;` line never matches.
pub fn value<'a>(text: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix('[') {
            let name = rest.split(']').next().unwrap_or(rest);
            inside = name.trim().eq_ignore_ascii_case(section);
            continue;
        }
        if !inside {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case(key) {
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value);
            return Some(value);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_found_as_the_games_reader_finds_them() {
        let text =
            "[nv-rs Mouse]\n;sSmoothing=Light\n  SSMOOTHING = \"Medium\" \nsSmoothing=Strong\n\
                    [General]\nsSmoothing=Off\n[ NV-RS MOUSE ]\nsAcceleration=Light\n";
        assert_eq!(value(text, "nv-rs Mouse", "sSmoothing"), Some("Medium"));
        assert_eq!(value(text, "nv-rs Mouse", "sAcceleration"), Some("Light"));
        assert_eq!(value(text, "General", "sSmoothing"), Some("Off"));
        assert_eq!(value(text, "nv-rs Mouse", ";sSmoothing"), Some("Light"));
        assert_eq!(value(text, "nv-rs Threads", "sSmoothing"), None);
        assert_eq!(value("[nv-rs Mouse]\n", "nv-rs Mouse", "sSmoothing"), None);
    }
}
