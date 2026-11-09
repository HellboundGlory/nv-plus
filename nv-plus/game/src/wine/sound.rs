//! The answers Wine gives differently, written as Windows gives them, so a capture taken under
//! Wine describes the device as the game's sound layer models it.

/// Result codes and flags (dsound.h).
const DS_OK: u32 = 0;
const E_NOINTERFACE: u32 = 0x8000_4002;
const DSERR_INVALIDPARAM: u32 = 0x8007_0057;
const DSERR_CONTROLUNAVAIL: u32 = 0x8878_001e;
const DSERR_BUFFERTOOSMALL: u32 = 0x8878_00b4;
const DS_NO_VIRTUALIZATION: u32 = 0x0878_000a;
const DSBCAPS_LOCSOFTWARE: u32 = 0x8;
const DSBCAPS_CTRL3D: u32 = 0x10;
const DSBCAPS_CTRLPAN: u32 = 0x40;
const DSBCAPS_CTRLFX: u32 = 0x200;
const DSBCAPS_SIZE: u32 = 0x14;
const MIN_BUFFER_BYTES: u32 = 4;

/// `table` with every answer the layer models rewritten to Windows' answer, and the number of
/// lines that changed.
pub fn windows_answers(table: &str) -> (String, usize) {
    let mut changed = 0;
    let mut out: Vec<String> = Vec::new();
    for line in table.split('\n') {
        match rewrite(line) {
            Some(new) => {
                changed += 1;
                out.push(new);
            }
            None => out.push(line.to_string()),
        }
    }
    (out.join("\n"), changed)
}

/// The line as Windows answers it, when the capture's answer differs.
fn rewrite(line: &str) -> Option<String> {
    let f: Vec<&str> = line.split('\t').collect();
    match *f.first()? {
        "DsBuffer" => buffer(&f),
        "DsFx" => effect(&f),
        "DsSet" => setting(&f),
        _ => None,
    }
}

fn word(text: &str) -> u32 {
    u32::from_str_radix(text, 16).unwrap_or_else(|_| panic!("a hex word: {text}"))
}

fn words(text: &str) -> Vec<u32> {
    text.split(' ')
        .filter(|w| !w.is_empty())
        .map(word)
        .collect()
}

/// The answer the layer gives a secondary `CreateSoundBuffer` (Windows' rule).
fn create_result(flags: u32, hrtf: bool, channels: u32, rate: u32, bits: u32, bytes: u32) -> u32 {
    let align = channels * bits / 8;
    if bytes < MIN_BUFFER_BYTES {
        DSERR_INVALIDPARAM
    } else if flags & DSBCAPS_CTRLFX != 0 && bytes / align < rate * 150 / 1000 {
        DSERR_BUFFERTOOSMALL
    } else if flags & DSBCAPS_CTRL3D != 0 && channels != 1 {
        DSERR_INVALIDPARAM
    } else if flags & DSBCAPS_CTRL3D != 0 && hrtf {
        DS_NO_VIRTUALIZATION
    } else {
        DS_OK
    }
}

fn buffer(f: &[&str]) -> Option<String> {
    let w = words(f[1]);
    let (flags, hrtf, tag, channels, rate, bits) = (w[0], w[1] != 0, w[2], w[3], w[4], w[5]);
    let bytes = w[6];
    let result = create_result(flags, hrtf, channels, rate, bits, bytes);
    let want = if (result as i32) < 0 {
        format!("DsBuffer\t{}\t{result:x}\t\t\t\t", f[1])
    } else {
        let mut caps = Vec::new();
        for x in [DSBCAPS_SIZE, flags | DSBCAPS_LOCSOFTWARE, bytes, 0, 0] {
            caps.extend_from_slice(&x.to_le_bytes());
        }
        let three_d = if flags & DSBCAPS_CTRL3D != 0 {
            DS_OK
        } else {
            E_NOINTERFACE
        };
        format!(
            "DsBuffer\t{}\t{result:x}\t{}\t{} 12\t{rate:x} 0 0 0\t0 {three_d:x}",
            f[1],
            hex(&caps),
            hex(&format_bytes(tag, channels, rate, bits))
        )
    };
    (want != f.join("\t")).then_some(want)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// `SetFX` and `GetObjectInPath` for the distortion and the I3DL2 reverb.
fn effect(f: &[&str]) -> Option<String> {
    if f.get(2)?.starts_with("create") {
        return None;
    }
    let mut w = f[1].split(' ');
    let name = w.next()?;
    let flags = word(w.next()?);
    let want: (u32, u32, u32) = if flags & DSBCAPS_CTRLFX == 0 {
        (DSERR_CONTROLUNAVAIL, 0xffff_ffff, DSERR_CONTROLUNAVAIL)
    } else if name == "distortion" {
        (DS_OK, 2, DS_OK)
    } else {
        (0x8004_0154, 5, 0x8878_1161)
    };
    let line = format!(
        "DsFx\t{}\t{:x}\t{:x}\t{:x}\t{}",
        f[1],
        want.0,
        want.1,
        want.2,
        f.get(5).copied().unwrap_or("")
    );
    (line != f.join("\t")).then_some(line)
}

/// `SetAllParameters` on the distortion, and the buffer's volume, pan, frequency and 3D
/// distances.
fn setting(f: &[&str]) -> Option<String> {
    let w: Vec<&str> = f[1].split(' ').collect();
    let ok = |good: bool| if good { DS_OK } else { DSERR_INVALIDPARAM };
    let want = match w.as_slice() {
        ["distortion", gain] => ok((-60.0..=0.0).contains(&f32::from_bits(word(gain)))),
        [_, "volume", v] => ok((-10000..=0).contains(&(word(v) as i32))),
        [flags, "pan", v] => {
            let has_pan = word(flags) & DSBCAPS_CTRLPAN != 0;
            let v = word(v) as i32;
            match ((-10000..=10000).contains(&v), has_pan) {
                (false, _) => DSERR_INVALIDPARAM,
                (true, false) => DSERR_CONTROLUNAVAIL,
                (true, true) => DS_OK,
            }
        }
        [_, "frequency", v] => {
            let v = word(v);
            ok(v == 0 || (100..=200_000).contains(&v))
        }
        ["3dbuffer", "mindistance" | "maxdistance", v] => ok(f32::from_bits(word(v)) > 0.0),
        _ => return None,
    };
    let mut out: Vec<String> = f.iter().map(|s| s.to_string()).collect();
    out[2] = format!("{want:x}");
    let line = out.join("\t");
    (line != f.join("\t")).then_some(line)
}

fn format_bytes(tag: u32, channels: u32, rate: u32, bits: u32) -> Vec<u8> {
    let align = (channels * bits / 8) as u16;
    let mut b = Vec::new();
    b.extend_from_slice(&(tag as u16).to_le_bytes());
    b.extend_from_slice(&(channels as u16).to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * u32::from(align)).to_le_bytes());
    b.extend_from_slice(&align.to_le_bytes());
    b.extend_from_slice(&(bits as u16).to_le_bytes());
    b.extend_from_slice(&0u16.to_le_bytes());
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_rewritten(cases: &[(&str, &str)], changed: usize) {
        let mut table = String::new();
        let mut want = String::new();
        for (wine, windows) in cases {
            table.push_str(wine);
            table.push('\n');
            want.push_str(windows);
            want.push('\n');
        }
        let (out, count) = windows_answers(&table);
        assert_eq!(out, want);
        assert_eq!(count, changed);
    }

    #[test]
    fn effects_get_windows_answers() {
        assert_rewritten(
            &[
                (
                    "DsFx\tdistortion 2e0 1 5622 ac44\t0\t2\t0\t00",
                    "DsFx\tdistortion 2e0 1 5622 ac44\t0\t2\t0\t00",
                ),
                (
                    "DsFx\tdistortion e0 1 5622 ac44\t8878001e\t5\t88781161\t00",
                    "DsFx\tdistortion e0 1 5622 ac44\t8878001e\tffffffff\t8878001e\t00",
                ),
                (
                    "DsFx\ti3dl2 2e0 1 5622 ac44\t0\t2\t0\t18fcffff",
                    "DsFx\ti3dl2 2e0 1 5622 ac44\t80040154\t5\t88781161\t18fcffff",
                ),
                (
                    "DsFx\ti3dl2 e0 1 5622 ac44\t8878001e\t5\t88781161\t18fcffff",
                    "DsFx\ti3dl2 e0 1 5622 ac44\t8878001e\tffffffff\t8878001e\t18fcffff",
                ),
                (
                    "DsFx\tdistortion 302b0 2 ac44 2b110\tcreate 80070057\t\t\t",
                    "DsFx\tdistortion 302b0 2 ac44 2b110\tcreate 80070057\t\t\t",
                ),
            ],
            3,
        );
    }

    #[test]
    fn settings_get_windows_answers() {
        assert_rewritten(
            &[
                (
                    "DsSet\tdistortion c2740000\t80004001\t00",
                    "DsSet\tdistortion c2740000\t80070057\t00",
                ),
                (
                    "DsSet\tdistortion c2700000\t80004001\t00",
                    "DsSet\tdistortion c2700000\t0\t00",
                ),
                (
                    "DsSet\tdistortion 00000000\t80004001\t00",
                    "DsSet\tdistortion 00000000\t0\t00",
                ),
                (
                    "DsSet\tdistortion 3f000000\t80004001\t00",
                    "DsSet\tdistortion 3f000000\t80070057\t00",
                ),
                (
                    "DsSet\t2e0 volume ffffd8ef\t0\t0",
                    "DsSet\t2e0 volume ffffd8ef\t80070057\t0",
                ),
                (
                    "DsSet\t2e0 volume ffffd8f0\t0\t0",
                    "DsSet\t2e0 volume ffffd8f0\t0\t0",
                ),
                (
                    "DsSet\t2e0 volume 1\t0\t0",
                    "DsSet\t2e0 volume 1\t80070057\t0",
                ),
                (
                    "DsSet\t2e0 pan 2711\t0\t0",
                    "DsSet\t2e0 pan 2711\t80070057\t0",
                ),
                (
                    "DsSet\t2e0 pan 2710\t80004001\t0",
                    "DsSet\t2e0 pan 2710\t0\t0",
                ),
                (
                    "DsSet\t302b0 pan 2711\t0\t0",
                    "DsSet\t302b0 pan 2711\t80070057\t0",
                ),
                (
                    "DsSet\t302b0 pan 0\t0\t0",
                    "DsSet\t302b0 pan 0\t8878001e\t0",
                ),
                (
                    "DsSet\t2e0 frequency 0\t80070057\t5622",
                    "DsSet\t2e0 frequency 0\t0\t5622",
                ),
                (
                    "DsSet\t2e0 frequency 63\t0\t63",
                    "DsSet\t2e0 frequency 63\t80070057\t63",
                ),
                (
                    "DsSet\t2e0 frequency 64\t0\t64",
                    "DsSet\t2e0 frequency 64\t0\t64",
                ),
                (
                    "DsSet\t2e0 frequency 30d40\t0\t30d40",
                    "DsSet\t2e0 frequency 30d40\t0\t30d40",
                ),
                (
                    "DsSet\t2e0 frequency 30d41\t0\t30d40",
                    "DsSet\t2e0 frequency 30d41\t80070057\t30d40",
                ),
                (
                    "DsSet\t3dbuffer mindistance 00000000\t0\t",
                    "DsSet\t3dbuffer mindistance 00000000\t80070057\t",
                ),
                (
                    "DsSet\t3dbuffer maxdistance 4e6e6b28\t80070057\t",
                    "DsSet\t3dbuffer maxdistance 4e6e6b28\t0\t",
                ),
            ],
            15,
        );
    }

    #[test]
    fn other_lines_are_unchanged() {
        let table = "DsCaps\t\t0\t00\n\
                     DsCursor\t5622 1 10 ac44 1 0\t0:0:0\n\
                     DsEnd";
        let (out, changed) = windows_answers(table);
        assert_eq!(out, table);
        assert_eq!(changed, 0);
    }

    #[test]
    fn buffers_get_windows_answers() {
        assert_rewritten(
            &[
                (
                    "DsBuffer\t2e0 0 1 1 1f40 8 4\t0\t14000000e8020000040000000000000000000000\t01000100401f0000401f0000010008000000 12\t1f40 0 0 0\t0 80004002",
                    "DsBuffer\t2e0 0 1 1 1f40 8 4\t887800b4\t\t\t\t",
                ),
                (
                    "DsBuffer\t2e0 0 1 2 ac44 10 2b110\t80070057\t\t\t\t",
                    "DsBuffer\t2e0 0 1 2 ac44 10 2b110\t0\t14000000e802000010b102000000000000000000\t0100020044ac000010b10200040010000000 12\tac44 0 0 0\t0 80004002",
                ),
                (
                    "DsBuffer\t302b0 1 1 1 1f40 8 4b0\t0\t14000000b8020300b00400000000000000000000\t01000100401f0000401f0000010008000000 12\t1f40 0 0 0\t0 0",
                    "DsBuffer\t302b0 1 1 1 1f40 8 4b0\t878000a\t14000000b8020300b00400000000000000000000\t01000100401f0000401f0000010008000000 12\t1f40 0 0 0\t0 0",
                ),
                (
                    "DsBuffer\t302b0 0 1 2 1f40 8 2000\t0\t14000000b8020300002000000000000000000000\t01000100401f0000401f0000010008000000 12\t1f40 0 0 0\t0 0",
                    "DsBuffer\t302b0 0 1 2 1f40 8 2000\t80070057\t\t\t\t",
                ),
                (
                    "DsBuffer\te0 0 1 1 1f40 10 3\t0\t14000000e8000000030000000000000000000000\t01000100401f0000803e0000020010000000 12\t1f40 0 0 0\t0 80004002",
                    "DsBuffer\te0 0 1 1 1f40 10 3\t80070057\t\t\t\t",
                ),
                (
                    "DsBuffer\te0 0 1 1 1f40 8 4\t0\t14000000e8000000040000000000000000000000\t01000100401f0000401f0000010008000000 12\t1f40 0 0 0\t0 80004002",
                    "DsBuffer\te0 0 1 1 1f40 8 4\t0\t14000000e8000000040000000000000000000000\t01000100401f0000401f0000010008000000 12\t1f40 0 0 0\t0 80004002",
                ),
                (
                    "DsBuffer\t2e0 0 1 1 1f40 8 4af\t0\t14000000e8020000af0400000000000000000000\t01000100401f0000401f0000010008000000 12\t1f40 0 0 0\t0 80004002",
                    "DsBuffer\t2e0 0 1 1 1f40 8 4af\t887800b4\t\t\t\t",
                ),
            ],
            6,
        );
    }
}
