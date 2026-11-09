//! dsfx-edge: the DirectSound distortion's edge correction table, measured from this machine's
//! own `dsdmo.dll` and written to stdout as 32 values on one line, separated by single spaces.
//! 32-bit, because Microsoft's 32-bit `dsdmo.dll` (winetricks) runs the distortion under Wine
//! while the 64-bit game can't load it, so the game can't measure the table itself.
//!
//! `cargo build --release --target i686-pc-windows-gnu`, then run the release binary under Wine
//! in a prefix with `winetricks dsdmo` (for example `~/.cache/nv-rs-wine`), with stdout into a
//! file: that file stays on the player's machine, and `NV_DSFX_EDGE` names it for the port.

use std::process::exit;

#[path = "../../../crates/engine-platform/src/dsfx.rs"]
#[allow(dead_code)]
mod dsfx;

/// One table entry from the same shift at the two impulses, when both agree to two decimals.
fn entry(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    let (a, b) = (a?, b?);
    let v = (a * 100.0).round() / 100.0;
    if !v.is_finite() || v <= 0.0 {
        return None;
    }
    if (b * 100.0).round() / 100.0 != v {
        return None;
    }
    if (a - v).abs() > 0.002 || (b - v).abs() > 0.002 {
        return None;
    }
    Some(v)
}

fn main() {
    let ratios: Vec<(usize, Option<f64>, Option<f64>)> = (2..=31)
        .map(|n| (n, dsfx::edge_ratio(n, 0.5), dsfx::edge_ratio(n, 0.9)))
        .collect();
    if ratios.iter().all(|&(_, a, b)| a.is_none() && b.is_none()) {
        eprintln!("this machine's dsdmo.dll gives no distortion to measure");
        exit(1);
    }
    let mut table = [1.0f64; 32];
    let mut refused = Vec::new();
    for (n, a, b) in ratios {
        match entry(a, b) {
            Some(v) => table[n] = v,
            None => refused.push(n),
        }
    }
    if refused.contains(&2) {
        eprintln!("entry 2, the only one the game reads, could not be measured");
        exit(1);
    }
    if !refused.is_empty() {
        let names: Vec<String> = refused.iter().map(usize::to_string).collect();
        eprintln!(
            "entries {} could not be measured; written as 1 (the game reads only entry 2)",
            names.join(" ")
        );
    }
    let values: Vec<String> = table.iter().map(f64::to_string).collect();
    println!("{}", values.join(" "));
}

#[cfg(test)]
mod tests {
    use super::entry;

    #[test]
    fn an_entry_needs_both_impulses_at_the_same_value() {
        let cases = [
            (Some(1.5), Some(1.5001), Some(1.5)),
            (Some(2.14), Some(2.1402), Some(2.14)),
            (Some(2.1058), Some(1.9794), None),
            (Some(1.4800184), Some(1.4595), None),
            (Some(1.5), None, None),
            (None, Some(1.5), None),
            (Some(1.5), Some(1.503), None),
            (Some(0.0), Some(0.0), None),
            (Some(f64::INFINITY), Some(f64::INFINITY), None),
        ];
        for (a, b, want) in cases {
            assert_eq!(entry(a, b), want, "{a:?}, {b:?}");
        }
    }
}
