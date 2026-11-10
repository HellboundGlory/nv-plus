//! The launcher's plain Windows dialogs, copied from upstream's Play.exe `ui.rs` for the parts the
//! screen needs: message boxes and the folder picker. They run on the thread that calls them.

use std::ffi::c_void;
use std::path::PathBuf;

#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(owner: *mut c_void, text: *const u16, title: *const u16, kind: u32) -> i32;
}
#[link(name = "shell32")]
extern "system" {
    fn SHBrowseForFolderW(info: *mut BrowseInfoW) -> *mut c_void;
    fn SHGetPathFromIDListW(list: *mut c_void, path: *mut u16) -> i32;
}
#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, flags: u32) -> i32;
    fn CoTaskMemFree(memory: *mut c_void);
}

#[repr(C)]
struct BrowseInfoW {
    owner: *mut c_void,
    root: *mut c_void,
    display_name: *mut u16,
    title: *const u16,
    flags: u32,
    callback: *mut c_void,
    param: isize,
    image: i32,
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(text: &[u16]) -> String {
    let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    String::from_utf16_lossy(&text[..end])
}

/// Scripted runs must not take the focus; the launcher's log says the same either way.
fn windows_allowed() -> bool {
    std::env::var_os("NV_RS_NO_WINDOWS").is_none_or(|v| v != "1")
}

const TITLE: &str = "NV+";
const MB_OK: u32 = 0;
const MB_YESNO: u32 = 4;
const MB_ICONERROR: u32 = 0x10;
const MB_ICONWARNING: u32 = 0x30;
const MB_ICONINFORMATION: u32 = 0x40;
const MB_SETFOREGROUND: u32 = 0x1_0000;
const IDYES: i32 = 6;

fn message_box(text: &str, kind: u32) -> i32 {
    if !windows_allowed() {
        return IDYES;
    }
    let (text, title) = (wide(text), wide(TITLE));
    // SAFETY: both strings live through the call; no owner window, so the box raises itself.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            kind | MB_SETFOREGROUND,
        )
    }
}

pub fn error(text: &str) {
    message_box(text, MB_OK | MB_ICONERROR);
}

pub fn warning(text: &str) {
    message_box(text, MB_OK | MB_ICONWARNING);
}

pub fn info(text: &str) {
    message_box(text, MB_OK | MB_ICONINFORMATION);
}

pub fn ask(text: &str) -> bool {
    message_box(text, MB_YESNO | MB_ICONINFORMATION) == IDYES
}

pub fn pick_folder(title: &str) -> Option<PathBuf> {
    if !windows_allowed() {
        return None;
    }
    let title = wide(title);
    let mut name = vec![0u16; 1024];
    let mut info = BrowseInfoW {
        owner: std::ptr::null_mut(),
        root: std::ptr::null_mut(),
        display_name: name.as_mut_ptr(),
        title: title.as_ptr(),
        // BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE.
        flags: 0x1 | 0x40,
        callback: std::ptr::null_mut(),
        param: 0,
        image: 0,
    };
    // SAFETY: the dialog wants COM on this thread; `info`'s strings live through the calls; the
    // list it hands back is freed here.
    unsafe {
        CoInitializeEx(std::ptr::null_mut(), 2);
        let list = SHBrowseForFolderW(&mut info);
        if list.is_null() {
            return None;
        }
        let mut path = vec![0u16; 1024];
        let ok = SHGetPathFromIDListW(list, path.as_mut_ptr());
        CoTaskMemFree(list);
        (ok != 0).then(|| PathBuf::from(from_wide(&path)))
    }
}
