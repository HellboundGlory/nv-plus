//! The patch notes the home screen shows, read from the repository's CHANGELOG.md.

pub struct Note {
    pub title: String,
    pub label: String,
    pub summary: String,
}

const CHANGELOG: &str = include_str!("../../../.github/CHANGELOG.md");

pub fn shipped_notes() -> Vec<Note> {
    patch_notes(CHANGELOG)
}

pub fn patch_notes(markdown: &str) -> Vec<Note> {
    let mut notes = Vec::new();
    let mut label = String::new();
    let mut lines = markdown.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(section) = line.strip_prefix("## ") {
            let section = section.trim();
            label = if section == "Unreleased" {
                "Next release".into()
            } else {
                section.into()
            };
            continue;
        }
        let Some(rest) = line.strip_prefix("- **") else {
            continue;
        };
        let Some((title, body)) = rest.split_once("**") else {
            continue;
        };
        let mut body = body.to_string();
        while let Some(next) = lines.next_if(|l| l.starts_with(' ') || l.starts_with('\t')) {
            body.push(' ');
            body.push_str(next);
        }
        notes.push(Note {
            title: title.strip_suffix('.').unwrap_or(title).trim().to_string(),
            label: label.clone(),
            summary: first_sentence(&plain(&body)),
        });
    }
    notes
}

/// The bullet's body as the player reads it: its continuation lines joined, links as their text,
/// no backticks for code.
fn plain(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    loop {
        let Some(open) = rest.find('[') else {
            out.push_str(rest);
            break;
        };
        let Some(close) = rest[open..].find(']') else {
            out.push_str(rest);
            break;
        };
        let text = &rest[open + 1..open + close];
        match rest[open + close + 1..].strip_prefix('(') {
            Some(tail) => match tail.find(')') {
                Some(end) => {
                    out.push_str(&rest[..open]);
                    out.push_str(text);
                    rest = &tail[end + 1..];
                }
                None => {
                    out.push_str(rest);
                    break;
                }
            },
            None => {
                out.push_str(&rest[..=open + close]);
                rest = &rest[open + close + 1..];
            }
        }
    }
    out.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('`', "")
}

/// Up to and including the body's first full stop, or the whole body when it has none.
fn first_sentence(text: &str) -> String {
    let text = text.trim();
    match text.find(". ") {
        Some(end) => text[..=end].to_string(),
        None => text.trim_end_matches(' ').to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(notes: &[Note]) -> Vec<(&str, &str, &str)> {
        notes
            .iter()
            .map(|n| (n.title.as_str(), n.label.as_str(), n.summary.as_str()))
            .collect()
    }

    #[test]
    fn bullets_and_their_sections() {
        let cases = [
            (
                "## Unreleased\n\n### Fixed\n\n- **Sound works.** It plays. The opening runs too,\n  joined into one line.\n",
                vec![(
                    "Sound works",
                    "Next release",
                    "It plays.",
                )],
            ),
            (
                "## Unreleased\n\n- **Links and code.** See [the report](https://example.com) and `FalloutPrefs.ini`.\n",
                vec![(
                    "Links and code",
                    "Next release",
                    "See the report and FalloutPrefs.ini.",
                )],
            ),
            (
                "## 1.0\n\n- **First.** Older.\n\n## 1.1\n\n- **Second.** Newer.\n",
                vec![("First", "1.0", "Older."), ("Second", "1.1", "Newer.")],
            ),
            (
                "Intro text.\n\n## Unreleased\n\nBased on nv-rs `abc`.\n\n### Fixed\n\n- **No body.**\n",
                vec![("No body", "Next release", "")],
            ),
            (
                "Intro text.\n\n## Unreleased\n\nBased on nv-rs `abc`.\n\n### Fixed\n\nProse only.\n",
                vec![],
            ),
        ];
        for (markdown, want) in cases {
            assert_eq!(rows(&patch_notes(markdown)), want, "{markdown:?}");
        }
    }

    #[test]
    fn the_shipped_changelog_gives_notes() {
        let notes = shipped_notes();
        assert!(!notes.is_empty());
        assert!(notes.iter().all(|n| !n.title.is_empty()));
    }
}
