//! Wine and Proton detection.

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
}
