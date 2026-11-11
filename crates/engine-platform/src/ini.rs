//! The profile (INI) file imports, `GetPrivateProfileIntA`, `GetPrivateProfileStringA` and
//! `WritePrivateProfileStringA`: they read and change the files the game made in this run
//! ([`files`]) and the Windows directory of the player's table ([`d3d9::windows_directory`]), so
//! they are in the platform layer rather than the core (moved from `engine-core`'s
//! `units/platform.rs`).

#[allow(unused_imports)]
use crate::prelude::*;

use super::{d3d9, files};

/// ASCII white space taken off both ends (`<[u8]>::trim_ascii` is newer than the MSRV).
trait TrimSpaces {
    fn trim_spaces(&self) -> &Self;
}

impl TrimSpaces for [u8] {
    fn trim_spaces(&self) -> &[u8] {
        let start = self
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .unwrap_or(self.len());
        let end = self
            .iter()
            .rposition(|b| !b.is_ascii_whitespace())
            .map_or(start, |i| i + 1);
        &self[start..end.max(start)]
    }
}

/// The value of `key` in section `section` of the INI text `text` (names compared without case,
/// spaces around names and values trimmed, one pair of surrounding quotes taken off), as
/// `GetPrivateProfileStringA` finds it: only in the first section of that name, as Windows reads
/// a file with a repeated section name (measured on scratch files, 2026-11-09; the test
/// `only_the_first_section_of_a_name_is_read` holds the answers).
fn ini_value(text: &[u8], section: &[u8], key: &[u8]) -> Option<Vec<u8>> {
    let section = section.trim_spaces();
    let mut inside = false;
    for line in text.split(|&b| b == b'\n') {
        let line = line.trim_spaces();
        if let Some(rest) = line.strip_prefix(b"[") {
            // The first section of the name ends here: Windows reads no later one.
            if inside {
                return None;
            }
            let name = rest.split(|&b| b == b']').next().unwrap_or(rest);
            inside = name.trim_spaces().eq_ignore_ascii_case(section);
            continue;
        }
        if !inside {
            continue;
        }
        let Some(at) = line.iter().position(|&b| b == b'=') else {
            continue;
        };
        if line[..at].trim_spaces().eq_ignore_ascii_case(key) {
            let value = line[at + 1..].trim_spaces();
            let value = match value {
                [b'"', middle @ .., b'"'] => middle,
                _ => value,
            };
            return Some(value.to_vec());
        }
    }
    None
}

/// The text of the profile file `file` names: a name without a full path is in the Windows
/// directory, as the profile functions look for it (`d3d9::windows_directory`, from the
/// player's table); the file as the game made it in this run when it did (`files`), else the
/// player's file, read only. `None` when there is none.
fn profile_text(e: &mut Engine, file: Ptr) -> Option<Vec<u8>> {
    let path = profile_path(e, file);
    files::read_whole(e, &path)
}

/// The full path of the profile file `file` names: a name without a full path is in the Windows
/// directory (`d3d9::windows_directory`, from the player's table).
fn profile_path(e: &mut Engine, file: Ptr) -> Vec<u8> {
    let name = e.mem.cstr(file.addr());
    if files::is_full_path(&name) {
        return name;
    }
    let mut path = d3d9::windows_directory(e);
    path.push(b'\\');
    path.extend_from_slice(&name);
    path
}

// Platform: replaces KERNEL32.DLL GetPrivateProfileIntA (import)
/// `GetPrivateProfileIntA(section, key, default, file)`: the key's value as an integer, or
/// `default` when the file, the section or the key isn't there ([`profile_text`]). The value is
/// read as [`profile_int`] does, which follows Windows' answers measured on the player's machine
/// (2026-10-21, 38 values).
pub fn get_private_profile_int_a(
    e: &mut Engine,
    section: Ptr,
    key: Ptr,
    default: u32,
    file: Ptr,
) -> u32 {
    let section = e.mem.cstr(section.addr());
    let key = e.mem.cstr(key.addr());
    let Some(value) = profile_text(e, file).and_then(|text| ini_value(&text, &section, &key))
    else {
        return default;
    };
    profile_int(&value)
}

/// A profile value as Windows' `GetPrivateProfileIntA` reads it (measured): leading spaces and
/// tabs, then one optional `+` or `-`, then an optional lower-case base prefix `0x` (16), `0o`
/// (8) or `0b` (2), else base 10; digits of that base up to the first other character, kept
/// modulo 2^32; a `-` negates the result modulo 2^32. No digits (`-`, `0x`, `0b2`, `x5`) give 0;
/// upper-case prefixes are not prefixes (`0X1F` is 0).
fn profile_int(value: &[u8]) -> u32 {
    let mut rest = value;
    while let [b' ' | b'\t', tail @ ..] = rest {
        rest = tail;
    }
    let negative = match rest {
        [b'-', tail @ ..] => {
            rest = tail;
            true
        }
        [b'+', tail @ ..] => {
            rest = tail;
            false
        }
        _ => false,
    };
    let base = match rest {
        [b'0', b'x', tail @ ..] => {
            rest = tail;
            16
        }
        [b'0', b'o', tail @ ..] => {
            rest = tail;
            8
        }
        [b'0', b'b', tail @ ..] => {
            rest = tail;
            2
        }
        _ => 10,
    };
    let mut n = 0u32;
    for &c in rest {
        let Some(digit) = char::from(c).to_digit(base) else {
            break;
        };
        n = n.wrapping_mul(base).wrapping_add(digit);
    }
    if negative {
        n.wrapping_neg()
    } else {
        n
    }
}

// Platform: replaces KERNEL32.DLL GetPrivateProfileStringA (import)
/// The value of `key` in `section` of the INI file `file` ([`profile_text`]), or
/// `default` (none: empty) when the file, the section or the key isn't there: copied into
/// `buffer`, cut to `size - 1` bytes and zero-terminated; answers the bytes copied. Listing a
/// file's sections or keys (a null section or key) is not provided.
pub fn get_private_profile_string_a(
    e: &mut Engine,
    section: Ptr,
    key: Ptr,
    default: Ptr,
    buffer: Ptr,
    size: u32,
    file: Ptr,
) -> u32 {
    if section.is_null() || key.is_null() {
        panic!("GetPrivateProfileStringA with a null section or key is not provided");
    }
    let section = e.mem.cstr(section.addr());
    let key = e.mem.cstr(key.addr());
    let found = profile_text(e, file).and_then(|text| ini_value(&text, &section, &key));
    let value = match found {
        Some(value) => value,
        None if default.is_null() => Vec::new(),
        None => e.mem.cstr(default.addr()),
    };
    if size == 0 {
        return 0;
    }
    let copied = value.len().min(size as usize - 1);
    e.mem.set_cstr(buffer.addr(), &value[..copied]);
    copied as u32
}

// Platform: replaces KERNEL32.DLL WritePrivateProfileStringA (import)
/// `WritePrivateProfileStringA(section, key, value, file)`: the value is set in the run's copy
/// of the file ([`files::write_whole`]), never the player's, and the answer is 1. A new key goes
/// after its section's last non-blank line and a missing section at the end of the file (not
/// measured); a null argument (a deletion, or `win.ini`) is not provided.
pub fn write_private_profile_string_a(
    e: &mut Engine,
    section: Ptr,
    key: Ptr,
    value: Ptr,
    file: Ptr,
) -> u32 {
    if section.is_null() || key.is_null() || value.is_null() || file.is_null() {
        panic!("WritePrivateProfileStringA with a null argument is not provided");
    }
    let section = e.mem.cstr(section.addr());
    let key = e.mem.cstr(key.addr());
    let value = e.mem.cstr(value.addr());
    let path = profile_path(e, file);
    let text = match files::read_whole(e, &path) {
        Some(text) => text,
        None => {
            assert!(
                files::directory_exists(e, &path),
                "WritePrivateProfileStringA(\"{}\"): a file in a directory that does not exist \
                 is not provided",
                String::from_utf8_lossy(&path)
            );
            Vec::new()
        }
    };
    let text = set_profile_value(&text, &section, &key, &value);
    files::write_whole(e, &path, text);
    1
}

/// `text` with `key=value` set in `section` ([`ini_value`]'s matching): an existing key keeps the
/// key text and line ending it has in the file, a new key goes after its section's last non-blank
/// line, and a missing section is appended at the end.
fn set_profile_value(text: &[u8], section: &[u8], key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut in_section = false;
    let mut found_section = false;
    let mut insert_at = None;
    let mut end = 0;
    for line in text.split_inclusive(|&b| b == b'\n') {
        let start = end;
        end += line.len();
        let body = line.strip_suffix(b"\n").unwrap_or(line);
        let body = body.strip_suffix(b"\r").unwrap_or(body);
        let trimmed = body.trim_spaces();
        if let Some(rest) = trimmed.strip_prefix(b"[") {
            let name = rest.split(|&b| b == b']').next().unwrap_or(rest);
            in_section = !found_section && name.trim_spaces().eq_ignore_ascii_case(section);
            found_section |= in_section;
            if in_section {
                insert_at = Some(end);
            }
            continue;
        }
        if !in_section || trimmed.is_empty() {
            continue;
        }
        insert_at = Some(end);
        let Some(at) = body.iter().position(|&b| b == b'=') else {
            continue;
        };
        if body[..at].trim_spaces().eq_ignore_ascii_case(key) {
            let mut out = text[..start].to_vec();
            out.extend_from_slice(&body[..=at]);
            out.extend_from_slice(value);
            out.extend_from_slice(&text[start + body.len()..]);
            return out;
        }
    }
    // A new key goes at `insert_at`; without its section, a new section goes at the end.
    let at = insert_at.unwrap_or(text.len());
    let mut out = text[..at].to_vec();
    if !out.is_empty() && !out.ends_with(b"\n") {
        out.extend_from_slice(b"\r\n");
    }
    if insert_at.is_none() {
        out.push(b'[');
        out.extend_from_slice(section);
        out.extend_from_slice(b"]\r\n");
    }
    out.extend_from_slice(key);
    out.push(b'=');
    out.extend_from_slice(value);
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(&text[at..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> Engine {
        let mut e = Engine::new();
        e.mem.map(0x1000, 0x1000);
        e
    }

    const PREFS: &[u8] = br"Z:\Data\FalloutPrefs.ini";

    fn put(e: &mut Engine, s: &[u8]) -> Ptr {
        let at = e.mem.alloc(s.len() as u32 + 1);
        e.mem.set_cstr(at, s);
        Ptr::new(at)
    }

    fn write(e: &mut Engine, section: &[u8], key: &[u8], value: &[u8], path: &[u8]) -> u32 {
        let (s, k, v, f) = (put(e, section), put(e, key), put(e, value), put(e, path));
        write_private_profile_string_a(e, s, k, v, f)
    }

    fn read(e: &mut Engine, section: &[u8], key: &[u8], path: &[u8]) -> Vec<u8> {
        let buffer = e.mem.alloc(0x100);
        let (s, k, f) = (put(e, section), put(e, key), put(e, path));
        let n = get_private_profile_string_a(e, s, k, Ptr::NULL, Ptr::new(buffer), 0x100, f);
        e.mem.bytes(buffer, n)
    }

    /// An existing key's value is replaced, keeping the key text and the line's ending; every
    /// other byte of the file stays.
    #[test]
    fn a_written_value_replaces_the_key_in_place() {
        const TEXT: &[u8] = b"; comment\r\n[General]\r\n sLanguage = ENGLISH \r\n[Controls]\r\nForward=\"0011FF00\"\r\nBack=1F\r\n";
        let cases: &[(&[u8], &[u8], &[u8], &[u8], &[u8])] = &[
            (
                TEXT,
                b"controls",
                b"forward",
                b"0022FF11",
                b"; comment\r\n[General]\r\n sLanguage = ENGLISH \r\n[Controls]\r\nForward=0022FF11\r\nBack=1F\r\n",
            ),
            (
                TEXT,
                b"General",
                b"slanguage",
                b"FRENCH",
                b"; comment\r\n[General]\r\n sLanguage =FRENCH\r\n[Controls]\r\nForward=\"0011FF00\"\r\nBack=1F\r\n",
            ),
            (b"[A]\nK=1\nL=2\n", b"A", b"K", b"9", b"[A]\nK=9\nL=2\n"),
            (b"[A]\nK=1", b"A", b"K", b"9", b"[A]\nK=9"),
        ];
        for &(before, section, key, value, after) in cases {
            let mut e = engine();
            files::write_whole(&mut e, PREFS, before.to_vec());
            assert_eq!(write(&mut e, section, key, value, PREFS), 1);
            assert_eq!(files::read_whole(&e, PREFS).unwrap(), after.to_vec());
            assert_eq!(read(&mut e, section, key, PREFS), value);
        }
    }

    /// A write gives the file's entry its new size, the time as last access and last write,
    /// and the archive bit, as a closed write does.
    #[test]
    fn a_write_updates_the_files_entry() {
        let mut e = engine();
        let dir = put(&mut e, br"Z:\Ini");
        assert_eq!(files::create_directory_a(&mut e, dir, 0), 1);
        let name = br"Z:\Ini\FalloutPrefs.ini";
        state!(e).disk.clock = Some(0x1234);
        assert_eq!(write(&mut e, b"Controls", b"Back", b"1F", name), 1);
        state!(e).disk.clock = Some(0x5678);
        assert_eq!(write(&mut e, b"Controls", b"Back", b"2EE", name), 1);
        let entry = &state!(e).disk.entries[&br"z:\ini\falloutprefs.ini".to_vec()];
        assert_eq!(
            (entry.size, entry.created, entry.accessed, entry.written),
            (22, 0x1234, 0x5678, 0x5678)
        );
        assert_ne!(entry.attributes & files::FILE_ATTRIBUTE_ARCHIVE, 0);
    }

    /// Each write stamps the file as changed, so the settings copy-out (`NV_PREFS_OUT`) sees it.
    #[test]
    fn a_write_stamps_the_file_as_changed() {
        let mut e = engine();
        files::write_whole(&mut e, PREFS, b"[Controls]\r\nBack=1F\r\n".to_vec());
        let k = br"z:\data\falloutprefs.ini".to_vec();
        let first = state!(e).disk.stamps.get(&k).copied();
        assert!(first.is_some());
        assert_eq!(write(&mut e, b"Controls", b"Back", b"2EE", PREFS), 1);
        assert!(state!(e).disk.stamps.get(&k).copied() > first);
    }

    /// A new key in an existing section goes after that section's last non-blank line.
    #[test]
    fn a_new_key_follows_its_sections_last_line() {
        let mut e = engine();
        let text = b"[General]\r\nsLanguage=ENGLISH\r\n\r\n[Controls]\r\nForward=0011FF00\r\n\r\n[Save]\r\n";
        files::write_whole(&mut e, PREFS, text.to_vec());
        assert_eq!(write(&mut e, b"controls", b"Back", b"1F", PREFS), 1);
        assert_eq!(
            files::read_whole(&e, PREFS).unwrap(),
            b"[General]\r\nsLanguage=ENGLISH\r\n\r\n[Controls]\r\nForward=0011FF00\r\nBack=1F\r\n\r\n[Save]\r\n".to_vec()
        );
        assert_eq!(read(&mut e, b"Controls", b"Back", PREFS), b"1F");
    }

    /// A missing section is appended, with a line ending first when the text does not end in one.
    #[test]
    fn a_new_section_is_appended() {
        let cases: &[(&[u8], &[u8])] = &[
            (
                b"[General]\r\nsLanguage=ENGLISH\r\n",
                b"[General]\r\nsLanguage=ENGLISH\r\n[Controls]\r\nBack=1F\r\n",
            ),
            (
                b"[General]\r\nsLanguage=ENGLISH",
                b"[General]\r\nsLanguage=ENGLISH\r\n[Controls]\r\nBack=1F\r\n",
            ),
        ];
        for &(before, want) in cases {
            let mut e = engine();
            files::write_whole(&mut e, PREFS, before.to_vec());
            assert_eq!(write(&mut e, b"Controls", b"Back", b"1F", PREFS), 1);
            assert_eq!(files::read_whole(&e, PREFS).unwrap(), want.to_vec());
            assert_eq!(read(&mut e, b"Controls", b"Back", PREFS), b"1F");
        }
    }

    /// A missing file in a directory that exists is made with its section; the player's disk
    /// keeps nothing.
    #[test]
    fn a_missing_file_in_an_existing_directory_is_made() {
        let mut e = engine();
        let dir = put(&mut e, br"Z:\Ini");
        assert_eq!(files::create_directory_a(&mut e, dir, 0), 1);
        let name = br"Z:\Ini\FalloutPrefs.ini";
        assert_eq!(write(&mut e, b"Controls", b"Back", b"1F", name), 1);
        assert_eq!(
            files::read_whole(&e, name).unwrap(),
            b"[Controls]\r\nBack=1F\r\n".to_vec()
        );
        assert_eq!(read(&mut e, b"Controls", b"Back", name), b"1F");
        assert!(
            !std::path::Path::new("Z:\\Ini\\FalloutPrefs.ini").exists(),
            "nothing reaches the player's disk"
        );
    }

    /// A name without a full path writes into the Windows directory, as reads find it there.
    #[test]
    fn a_bare_name_writes_into_the_windows_directory() {
        let dir = std::env::temp_dir().join(format!("nv-rs-ini-win-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let hex: String = dir
            .to_string_lossy()
            .into_owned()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let mut e = engine();
        d3d9::load_adapter_table(&mut e, &format!("GetWindowsDirectoryA\t\t1\t{hex}\n"));
        let mut path = dir.to_string_lossy().into_owned().into_bytes();
        path.extend_from_slice(b"\\FalloutPrefs.ini");
        assert_eq!(
            write(&mut e, b"Controls", b"Back", b"1F", b"FalloutPrefs.ini"),
            1
        );
        assert_eq!(
            files::read_whole(&e, &path).unwrap(),
            b"[Controls]\r\nBack=1F\r\n".to_vec()
        );
        assert_eq!(
            read(&mut e, b"Controls", b"Back", b"FalloutPrefs.ini"),
            b"1F"
        );
        let made_inside = std::fs::read_dir(&dir).unwrap().count();
        let made_beside = std::fs::metadata(String::from_utf8_lossy(&path).as_ref()).is_ok();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            (made_inside, made_beside),
            (0, false),
            "nothing reaches the disk"
        );
    }

    /// A null section, key, value or file (a deletion, or `win.ini`) stops the run.
    #[test]
    fn a_null_argument_stops() {
        for null in 0..4 {
            let mut e = engine();
            let mut args = [
                put(&mut e, b"Controls"),
                put(&mut e, b"Back"),
                put(&mut e, b"1F"),
                put(&mut e, PREFS),
            ];
            args[null] = Ptr::NULL;
            let [s, k, v, f] = args;
            let stop = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                write_private_profile_string_a(&mut e, s, k, v, f)
            }))
            .expect_err("a null argument must stop");
            let text = stop
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| stop.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            assert!(text.contains("not provided"), "argument {null}: {text}");
        }
    }

    /// A file in a directory that does not exist stops the run.
    #[test]
    #[should_panic(expected = "not provided")]
    fn a_missing_directory_stops() {
        let mut e = engine();
        write(
            &mut e,
            b"Controls",
            b"Back",
            b"1F",
            br"Z:\NoSuch\FalloutPrefs.ini",
        );
    }

    #[test]
    fn ini_values_are_found_by_section_and_key() {
        let text = b"; comment\r\n[General]\r\n sLanguage = ENGLISH \r\n[Controls]\r\nForward=\"0011FF00\"\r\nBack=1F\r\n";
        assert_eq!(
            ini_value(text, b"controls", b"forward").unwrap(),
            b"0011FF00"
        );
        assert_eq!(ini_value(text, b"Controls", b"Back").unwrap(), b"1F");
        assert_eq!(
            ini_value(text, b"General", b"sLanguage").unwrap(),
            b"ENGLISH"
        );
        assert_eq!(ini_value(text, b"General", b"Back"), None);
    }

    /// A `;` line is never found: a key nv-rs's `Fallout.ini` writes commented out
    /// (`distribution/nv-rs-sections.ini`) or a player commented out is not the key, and the
    /// `[nv-rs ...]` sections are sections like any other.
    #[test]
    fn a_commented_key_is_never_found() {
        let text = b"[nv-rs Fixes]\r\n; Uncapped frame rate: a note\r\n;bUncap=1\r\n\
                     [Display]\r\n;iPresentInterval=0\r\niPresentInterval=1\r\n";
        assert_eq!(ini_value(text, b"nv-rs Fixes", b"bUncap"), None);
        assert_eq!(
            ini_value(text, b"Display", b"iPresentInterval").unwrap(),
            b"1"
        );
    }

    /// A file that starts with a UTF-8 byte order mark, as Windows' `GetPrivateProfileStringA`
    /// reads it (measured on scratch files, 2026-11-09, measure/ini/DESIGN.md): a heading glued
    /// to the mark is no heading, so the keys under it are in no section and not found, and the
    /// sections after it read as usual. Play.exe puts nv-rs's block at the end of such a file
    /// (scripts/launcher `game_ini::with_sections`), where it changes none of the player's keys.
    #[test]
    fn a_heading_glued_to_a_byte_order_mark_is_no_heading() {
        let marked = b"\xef\xbb\xbf[General]\r\nsA=1\r\nsB=two\r\n[Display]\r\nfB=2\r\n\
                       \r\n[nv-rs General]\r\nsPreset=fixes\r\n";
        assert_eq!(ini_value(marked, b"General", b"sA"), None);
        assert_eq!(ini_value(marked, b"General", b"sB"), None);
        assert_eq!(ini_value(marked, b"Display", b"fB").unwrap(), b"2");
        assert_eq!(
            ini_value(marked, b"nv-rs General", b"sPreset").unwrap(),
            b"fixes"
        );
        // The mark alone on the first line: the next line is a heading.
        let alone = b"\xef\xbb\xbf\r\n[General]\r\nsA=1\r\n";
        assert_eq!(ini_value(alone, b"General", b"sA").unwrap(), b"1");
    }

    /// Windows' answers on a file with a repeated section name, measured on scratch files on the
    /// player's machine (2026-11-09; `GetPrivateProfileStringA` and `GetPrivateProfileIntA` alike):
    /// only the first section of a name is read, the name compared without case and with the
    /// spaces in the brackets and in the caller's name trimmed. An empty first section hides a
    /// later one; an empty key in the first one is found, empty.
    #[test]
    fn only_the_first_section_of_a_name_is_read() {
        let text = b"[A]\r\nx=1\r\n[B]\r\nz=3\r\n[a]\r\ny=2\r\nx=9\r\n[ A ]\r\nw=6\r\n";
        assert_eq!(ini_value(text, b"A", b"x").unwrap(), b"1");
        assert_eq!(ini_value(text, b"A", b"y"), None);
        assert_eq!(ini_value(text, b"A", b"w"), None);
        assert_eq!(ini_value(text, b"B", b"z").unwrap(), b"3");
        assert_eq!(ini_value(text, b" A ", b"x").unwrap(), b"1");
        let text = b"[a]\r\nx=1\r\n[A]\r\ny=2\r\n[C]\r\nq=1\r\n[C]\r\nq=2\r\nr=3\r\n";
        assert_eq!(ini_value(text, b"A", b"x").unwrap(), b"1");
        assert_eq!(ini_value(text, b"A", b"y"), None);
        assert_eq!(ini_value(text, b"C", b"q").unwrap(), b"1");
        assert_eq!(ini_value(text, b"C", b"r"), None);
        assert_eq!(ini_value(b"[A]\r\n[A]\r\nx=1\r\n", b"A", b"x"), None);
        assert_eq!(
            ini_value(b"[A]\r\n; note\r\n\r\n[A]\r\nx=1\r\n", b"A", b"x"),
            None
        );
        assert_eq!(
            ini_value(b"[A]\r\nx=\r\n[A]\r\nx=1\r\n", b"A", b"x").unwrap(),
            b""
        );
    }

    /// Three more of Windows' `GetPrivateProfileStringA` answers, measured on scratch files on
    /// the player's machine (2026-11-10): a key repeated in one section gives its first line,
    /// whatever its case and spaces; a heading is the text up to its first `]` (text after it, or
    /// no `]` at all, changes nothing, and `[A]x=9` is a heading, not a key); a key before any
    /// heading is in no section, not even the one named "", which `[]` opens.
    #[test]
    fn repeated_keys_headings_and_keys_before_any_section_read_as_windows_does() {
        assert_eq!(
            ini_value(b"[A]\r\nx=1\r\nx=2\r\n", b"A", b"x").unwrap(),
            b"1"
        );
        assert_eq!(
            ini_value(b"[A]\r\nX=1\r\n x =2\r\n", b"A", b"x").unwrap(),
            b"1"
        );
        let headings: [&[u8]; 6] = [
            b"[A] trailing\r\nx=1\r\n",
            b"[A]; note\r\nx=1\r\n",
            b"[A]]\r\nx=1\r\n",
            b"[A\r\nx=1\r\n",
            b"[A]x=9\r\nx=1\r\n",
            b" [A] \r\nx=1\r\n",
        ];
        for text in headings {
            assert_eq!(ini_value(text, b"A", b"x").unwrap(), b"1");
        }
        assert_eq!(ini_value(b"[A]]\r\nx=1\r\n", b"A]", b"x"), None);
        assert_eq!(ini_value(b"x=1\r\n[A]\r\ny=2\r\n", b"", b"x"), None);
        assert_eq!(ini_value(b"x=1\r\n[A]\r\ny=2\r\n", b"A", b"x"), None);
        assert_eq!(ini_value(b"x=1\r\n[]\r\nz=3\r\n", b"", b"z").unwrap(), b"3");
        assert_eq!(ini_value(b"x=1\r\n[]\r\nz=3\r\n", b"", b"x"), None);
    }

    /// Windows' `GetPrivateProfileIntA` answers, measured on the player's machine (2026-10-21).
    #[test]
    fn profile_ints_read_as_windows_does() {
        let measured: &[(&[u8], u32)] = &[
            (b"-5", 0xffff_fffb),
            (b"-0", 0),
            (b"0x10", 16),
            (b"0X1F", 0),
            (b"-0x10", 0xffff_fff0),
            (b"12abc", 12),
            (b"  7", 7),
            (b"+3", 3),
            (b"4294967295", 0xffff_ffff),
            (b"4294967296", 0),
            (b"99999999999", 1_215_752_191),
            (b"1e3", 1),
            (b"0", 0),
            (b"08", 8),
            (b"010", 10),
            (b"-", 0),
            (b"x5", 0),
            (b"0b101", 5),
            (b"0o17", 15),
            (b"0x", 0),
            (b"0xg", 0),
            (b"-  5", 0),
            (b" -5", 0xffff_fffb),
            (b"0b2", 0),
            (b"0x10abc", 0x10abc),
            (b"0xFFFFFFFF1", 0xffff_fff1),
            (b"0B11", 0),
            (b"0O7", 0),
            (b"\t9", 9),
            (b"--5", 0),
            (b"+-5", 0),
            (b"- 5", 0),
            (b"00x10", 0),
            (b"0x-5", 0),
            (b"0b", 0),
            (b"0o8", 0),
            (b"5 6", 5),
        ];
        for &(value, want) in measured {
            assert_eq!(profile_int(value), want, "{:?}", std::str::from_utf8(value));
        }
    }

    /// A missing file gives the default, cut to the buffer.
    #[test]
    fn profile_strings_fall_back_to_the_default() {
        let mut e = engine();
        e.mem.set_cstr(0x1000, b"Controls");
        e.mem.set_cstr(0x1020, b"Forward");
        e.mem.set_cstr(0x1040, b"default");
        e.mem.set_cstr(0x1060, br"Z:\no\such\file.ini");
        let n = get_private_profile_string_a(
            &mut e,
            Ptr::new(0x1000),
            Ptr::new(0x1020),
            Ptr::new(0x1040),
            Ptr::new(0x1100),
            4,
            Ptr::new(0x1060),
        );
        assert_eq!(n, 3);
        assert_eq!(e.mem.cstr(0x1100), b"def");
        let n = get_private_profile_string_a(
            &mut e,
            Ptr::new(0x1000),
            Ptr::new(0x1020),
            Ptr::NULL,
            Ptr::new(0x1100),
            0x40,
            Ptr::new(0x1060),
        );
        assert_eq!((n, e.mem.u8(0x1100)), (0, 0));
    }
}
