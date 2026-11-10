//! The plain queries the screens need before they exist, and opening the logs folder.
use crate::win::{wide, wide_str};
use std::ffi::c_void;

#[link(name = "user32")]
extern "system" {
    fn GetSystemMetrics(index: i32) -> i32;
    fn FindWindowW(class: *const u16, title: *const u16) -> *mut c_void;
}
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        owner: *mut c_void,
        verb: *const u16,
        file: *const u16,
        parameters: *const u16,
        folder: *const u16,
        show: i32,
    ) -> isize;
}

/// Whether windows may be shown.
pub fn windows_allowed() -> bool {
    std::env::var_os("NV_RS_NO_WINDOWS").is_none_or(|v| v != "1")
}

/// Opens a folder in Explorer (after an error, so the player finds the log).
pub fn open_folder(path: &std::path::Path) {
    if !windows_allowed() {
        return;
    }
    let (verb, file) = (wide_str("open"), wide(path.as_os_str()));
    // SAFETY: the strings live through the call; 1 is SW_SHOWNORMAL.
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        );
    }
}

/// Whether the game's own window exists yet (its class in `d3d9real`).
pub fn game_window_is_there() -> bool {
    let class = wide_str("nv-rs-d3d9");
    // The class is registered with the A call; the name compares the same.
    // SAFETY: the string lives through the call.
    unsafe { !FindWindowW(class.as_ptr(), std::ptr::null()).is_null() }
}

/// The primary monitor's size as a program without DPI awareness sees it, as the game does.
pub fn desktop_size() -> (i32, i32) {
    // SAFETY: no pointers.
    unsafe { (GetSystemMetrics(0), GetSystemMetrics(1)) }
}
