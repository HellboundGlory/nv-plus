//! The DirectSound distortion's edge correction table, read from the file `NV_DSFX_EDGE` names.

/// The table in the file `NV_DSFX_EDGE` names; a set variable with a bad file is an error.
pub fn from_file() -> Option<[f64; 32]> {
    let path = std::env::var_os("NV_DSFX_EDGE")?;
    let path = std::path::Path::new(&path);
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("NV_DSFX_EDGE {}: {e}", path.display()));
    Some(parse_edge_table(&text).unwrap_or_else(|e| panic!("NV_DSFX_EDGE {}: {e}", path.display())))
}

/// A table of 32 whitespace-separated finite positive decimals.
pub fn parse_edge_table(text: &str) -> Result<[f64; 32], String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() != 32 {
        return Err(format!("expected 32 values, found {}", words.len()));
    }
    let mut table = [0.0f64; 32];
    for (slot, word) in table.iter_mut().zip(words) {
        let v: f64 = word.parse().map_err(|_| format!("not a number: {word}"))?;
        if !v.is_finite() || v <= 0.0 {
            return Err(format!("not a positive number: {word}"));
        }
        *slot = v;
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edge_table_parses_from_whitespace() {
        let words: Vec<String> = (0..32).map(|i| format!("{}.5", i % 3)).collect();
        let text = format!("{}\t \n", words.join("\t "));
        let want: [f64; 32] = std::array::from_fn(|i| (i % 3) as f64 + 0.5);
        assert_eq!(parse_edge_table(&text), Ok(want));
    }

    #[test]
    fn a_bad_edge_table_is_refused() {
        let words = |count: usize| -> Vec<String> {
            (0..count).map(|i| format!("{}", i as f64 + 0.5)).collect()
        };
        let with = |at: usize, word: &str| -> String {
            let mut w = words(32);
            w[at] = word.to_string();
            w.join(" ")
        };
        let cases = [
            (words(31).join(" "), "found 31"),
            (words(33).join(" "), "found 33"),
            (with(5, "two"), "two"),
            (with(6, "0"), "not a positive number: 0"),
            (with(7, "-1.5"), "-1.5"),
            (with(8, "NaN"), "NaN"),
        ];
        for (text, reason) in cases {
            let error = parse_edge_table(&text).unwrap_err();
            assert!(error.contains(reason), "{error:?} does not name {reason:?}");
        }
    }
}
