use std::path::PathBuf;
use std::process::Command;

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const LANG_EN_US: u16 = 0x0409;

fn main() {
    slint_build::compile("ui/launcher.slint").expect("ui/launcher.slint");
    icon_resource();
    println!("cargo:rerun-if-changed=../../.github/CHANGELOG.md");
    println!("cargo:rerun-if-env-changed=NV_PLUS_VERSION");
    // HEAD's reflog changes on every commit and checkout, so the sha below stays current.
    for file in ["HEAD", "logs/HEAD"] {
        if let Some(path) = git(&["rev-parse", "--git-path", file]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }

    let version = std::env::var("NV_PLUS_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| git(&["rev-parse", "--short", "HEAD"]))
        .unwrap_or_else(|| "dev".into());
    println!("cargo:rustc-env=NV_PLUS_VERSION={version}");
}

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (out.status.success() && !text.is_empty()).then_some(text)
}

/// Launcher.exe's icon in Explorer and on the taskbar: `assets/nv-plus.ico` (the NV+ logo, 16 to
/// 256 pixels) written as a `.res` file. The MSVC linker takes it as it is; MinGW's needs it turned
/// into an object file by `windres`. The same writer as `viewer/build.rs`: an empty first entry, one `RT_ICON` per image and an
/// `RT_GROUP_ICON` (name 1) listing them.
fn icon_resource() {
    println!("cargo:rerun-if-changed=assets/nv-plus.ico");
    println!("cargo:rerun-if-env-changed=WINDRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let ico = std::fs::read("assets/nv-plus.ico").expect("assets/nv-plus.ico");
    assert!(
        ico.len() >= 6 && u16_at(&ico, 0) == 0 && u16_at(&ico, 2) == 1,
        "nv-plus.ico is not an icon file"
    );
    let count = usize::from(u16_at(&ico, 4));
    assert!(
        ico.len() >= 6 + 16 * count,
        "nv-plus.ico's directory is cut short"
    );
    let mut res = Vec::new();
    entry(&mut res, 0, 0, 0, &[]);
    // The group: the .ico's directory with each image's file offset replaced by its resource id.
    let mut group = Vec::with_capacity(6 + 14 * count);
    group.extend_from_slice(&ico[0..6]);
    for k in 0..count {
        let dir = 6 + 16 * k;
        let size = u32_at(&ico, dir + 8) as usize;
        let offset = u32_at(&ico, dir + 12) as usize;
        let image = offset
            .checked_add(size)
            .and_then(|end| ico.get(offset..end))
            .expect("nv-plus.ico: an image runs past the end of the file");
        let id = u16::try_from(k + 1).expect("fewer than 65535 images");
        entry(&mut res, RT_ICON, id, 0x1010, image);
        group.extend_from_slice(&ico[dir..dir + 12]);
        group.extend_from_slice(&id.to_le_bytes());
    }
    entry(&mut res, RT_GROUP_ICON, 1, 0x1030, &group);
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("nv-plus.res");
    std::fs::write(&out, res).expect("write nv-plus.res");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins={}", out.display());
        return;
    }
    let object = out.with_extension("o");
    let windres = std::env::var("WINDRES").unwrap_or_else(|_| {
        let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
        format!("{arch}-w64-mingw32-windres")
    });
    let made = [windres.as_str(), "windres"].iter().any(|tool| {
        Command::new(tool)
            .args(["--input-format=res", "--output-format=coff", "-i"])
            .arg(&out)
            .arg("-o")
            .arg(&object)
            .status()
            .is_ok_and(|s| s.success())
    });
    if made {
        println!("cargo:rustc-link-arg-bins={}", object.display());
    } else {
        println!("cargo:warning=windres not found: Launcher.exe is built without its icon");
    }
}

/// One resource: the 32-byte header (numeric type and name), then the data padded to 4 bytes. The
/// empty first entry has language 0, as resource compilers write it.
fn entry(res: &mut Vec<u8>, kind: u16, name: u16, flags: u16, data: &[u8]) {
    let language = if kind == 0 { 0 } else { LANG_EN_US };
    let size = u32::try_from(data.len()).expect("an icon image under 4 GB");
    res.extend_from_slice(&size.to_le_bytes());
    res.extend_from_slice(&32u32.to_le_bytes());
    res.extend_from_slice(&[0xff, 0xff]);
    res.extend_from_slice(&kind.to_le_bytes());
    res.extend_from_slice(&[0xff, 0xff]);
    res.extend_from_slice(&name.to_le_bytes());
    res.extend_from_slice(&0u32.to_le_bytes()); // data version
    res.extend_from_slice(&flags.to_le_bytes());
    res.extend_from_slice(&language.to_le_bytes());
    res.extend_from_slice(&0u32.to_le_bytes()); // version
    res.extend_from_slice(&0u32.to_le_bytes()); // characteristics
    res.extend_from_slice(data);
    while !res.len().is_multiple_of(4) {
        res.push(0);
    }
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
