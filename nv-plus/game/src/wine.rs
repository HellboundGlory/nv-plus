//! Wine and Proton detection, and the sound table's answers written as Windows gives them.

pub mod sound;

use std::io::Write;
use std::path::Path;

/// Wine and Proton: their ntdll exports `wine_get_version`, and their `RtlGenerate8dot3Name`
/// is a stub that aborts the process.
#[cfg(windows)]
pub fn is_wine() -> bool {
    static WINE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *WINE.get_or_init(|| {
        // SAFETY: both names are NUL-terminated; ntdll is loaded in every process.
        unsafe {
            let module = GetModuleHandleA(b"ntdll.dll\0".as_ptr());
            !module.is_null() && !GetProcAddress(module, b"wine_get_version\0".as_ptr()).is_null()
        }
    })
}

#[cfg(not(windows))]
pub fn is_wine() -> bool {
    false
}

/// `bytes` as a sound table, unchanged on Windows; under Wine or Proton, [`windows_table`].
pub fn sound_table(bytes: Vec<u8>, log: &Path) -> Result<Vec<u8>, String> {
    if !is_wine() {
        return Ok(bytes);
    }
    windows_table(&bytes, log)
}

/// The table with the answers Wine gives differently written as Windows gives them, and one line
/// in `log` saying how many changed. A table it can't read is an error, as the capture tool's own
/// failure is, so the launcher falls back the same way.
pub fn windows_table(bytes: &[u8], log: &Path) -> Result<Vec<u8>, String> {
    let text = String::from_utf8_lossy(bytes);
    let (text, changed) =
        std::panic::catch_unwind(|| sound::windows_answers(&text)).map_err(|_| {
            "the sound table has a line Windows' answers can't be written for".to_string()
        })?;
    let line = format!(
        "Wine: {changed} answers changed to Windows' (the game's sound layer models Windows)\n"
    );
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .and_then(|mut file| file.write_all(line.as_bytes()));
    Ok(text.into_bytes())
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleA(name: *const u8) -> *mut std::ffi::c_void;
    fn GetProcAddress(module: *mut std::ffi::c_void, name: *const u8) -> *mut std::ffi::c_void;
}

#[cfg(test)]
mod tests {
    #[cfg(not(windows))]
    #[test]
    fn it_is_not_wine_off_windows() {
        assert!(!super::is_wine());
    }

    #[cfg(not(windows))]
    #[test]
    fn the_table_is_unchanged_off_windows() {
        let table = b"DsSet\t2e0 volume 1\t0\t0\nDsEnd";
        assert!(super::sound::windows_answers(&String::from_utf8_lossy(table)).1 > 0);
        let log = std::env::temp_dir().join("nv-plus-unchanged-off-windows.log");
        assert_eq!(super::sound_table(table.to_vec(), &log), Ok(table.to_vec()));
        assert!(!log.exists());
    }

    #[test]
    fn a_rewritten_table_names_its_count_in_the_log() {
        let log = std::env::temp_dir().join(format!("nv-plus-count-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&log);
        let table = b"DsSet\t2e0 volume 1\t0\t0\nDsEnd";
        let (want, changed) = super::sound::windows_answers(&String::from_utf8_lossy(table));
        assert_eq!(super::windows_table(table, &log), Ok(want.into_bytes()));
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(
            text.contains(&format!("Wine: {changed} answers changed")),
            "{text}"
        );
        std::fs::remove_file(&log).unwrap();
    }

    #[test]
    fn a_table_that_cant_be_read_is_an_error() {
        let log = std::env::temp_dir().join("nv-plus-unreadable.log");
        assert!(super::windows_table(b"DsBuffer", &log).is_err());
    }
}
