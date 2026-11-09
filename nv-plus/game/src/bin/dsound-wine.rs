//! Rewrites a sound table captured under Wine as Windows answers it: the table on stdin, the
//! rewritten table on stdout and one line on stderr saying how many answers changed.

use std::io::{Read, Write};

fn main() {
    let mut table = Vec::new();
    std::io::stdin().read_to_end(&mut table).unwrap();
    let (text, changed) =
        nv_plus_game::wine::sound::windows_answers(&String::from_utf8_lossy(&table));
    std::io::stdout().write_all(text.as_bytes()).unwrap();
    eprintln!(
        "Wine: {changed} answers changed to Windows' (the game's sound layer models Windows)"
    );
}
