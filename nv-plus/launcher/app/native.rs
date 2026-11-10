// The native window's outline, its big icon, and full repaints after the system throws the
// window's pixels away.
//
// Slint hands its icon to the window as the small icon only; Wine builds the taskbar's icon from
// the big one and falls back to its own logo, so the big icon is set from Launcher.exe's resource.
//
// The renderer redraws only what changed since its last frame, so after Wine drops a minimised
// window's contents and asks for a repaint, nothing changed and nothing is drawn. A WM_PAINT
// carrying an update region comes from the system; the renderer's own requests carry none
// (RDW_INTERNALPAINT), so only the system's ones call `on_wiped`.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, Ordering};

type Hwnd = *mut c_void;
type WndProc = unsafe extern "system" fn(Hwnd, u32, usize, isize) -> isize;

const GWLP_WNDPROC: i32 = -4;
const WM_SIZE: u32 = 0x0005;
const WM_PAINT: u32 = 0x000F;
const WM_SETICON: u32 = 0x0080;
const SIZE_MINIMIZED: usize = 1;
const WINDING: i32 = 2;
const ICON_BIG: usize = 1;
const IMAGE_ICON: u32 = 1;
// The icon group `app/build.rs` writes.
const ICON_GROUP: usize = 1;

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[link(name = "user32")]
extern "system" {
    fn GetUpdateRect(window: Hwnd, rect: *mut Rect, erase: i32) -> i32;
    fn GetClientRect(window: Hwnd, rect: *mut Rect) -> i32;
    fn GetDpiForWindow(window: Hwnd) -> u32;
    fn SetWindowRgn(window: Hwnd, region: *mut c_void, redraw: i32) -> i32;
    fn SetWindowLongPtrW(window: Hwnd, index: i32, value: isize) -> isize;
    fn CallWindowProcW(previous: isize, window: Hwnd, msg: u32, w: usize, l: isize) -> isize;
    fn SendMessageW(window: Hwnd, msg: u32, w: usize, l: isize) -> isize;
    fn LoadImageW(
        instance: *mut c_void,
        name: *const u16,
        kind: u32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> *mut c_void;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreatePolygonRgn(points: *const Point, count: i32, mode: i32) -> *mut c_void;
    fn CreateRectRgn(left: i32, top: i32, right: i32, bottom: i32) -> *mut c_void;
    fn DeleteObject(object: *mut c_void) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
}

static PREVIOUS: AtomicIsize = AtomicIsize::new(0);

thread_local! {
    static ON_WIPED: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
}

/// The gold line's outer edge in `frame-22.png` drawn on a `width` x `height` window, in logical
/// pixels: the art's black rim and its corners fall outside. The art is hand drawn, so the
/// bottom-right corner is cut deeper than the others. A region leaves out its right and bottom
/// edges, so those sit one pixel past the last pixel kept.
pub fn outline(width: i32, height: i32) -> [(i32, i32); 8] {
    let (w, h) = (width, height);
    [
        (16, 4),
        (w - 16, 4),
        (w - 4, 16),
        (w - 4, h - 18),
        (w - 17, h - 4),
        (16, h - 4),
        (4, h - 16),
        (4, 16),
    ]
}

fn hwnd(window: &slint::Window) -> Option<Hwnd> {
    match window.window_handle().window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as Hwnd),
        _ => None,
    }
}

/// Cuts the window to the outline at its present size and DPI. While minimised under Wine it is
/// cut to nothing: Wine shrinks a minimising window to a strip at its top-left, which the renderer
/// fills with the frame's corner. Windows draws a minimised window nowhere, and its taskbar
/// preview needs the region.
unsafe fn cut(window: Hwnd, minimised: bool) {
    let region = if minimised {
        if !nv_plus_game::wine::is_wine() {
            return;
        }
        CreateRectRgn(0, 0, 0, 0)
    } else {
        let mut client = Rect::default();
        if GetClientRect(window, &mut client) == 0 || client.right == 0 || client.bottom == 0 {
            return;
        }
        let scale = match GetDpiForWindow(window) {
            0 => 1.0,
            dpi => dpi as f32 / 96.0,
        };
        let logical = |v: i32| (v as f32 / scale).round() as i32;
        let points: Vec<Point> = outline(logical(client.right), logical(client.bottom))
            .iter()
            .map(|&(x, y)| Point {
                x: (x as f32 * scale).round() as i32,
                y: (y as f32 * scale).round() as i32,
            })
            .collect();
        CreatePolygonRgn(points.as_ptr(), points.len() as i32, WINDING)
    };
    // The system owns the region once it is set.
    if !region.is_null() && SetWindowRgn(window, region, 1) == 0 {
        DeleteObject(region);
    }
}

unsafe extern "system" fn proc(window: Hwnd, msg: u32, w: usize, l: isize) -> isize {
    if msg == WM_SIZE {
        cut(window, w == SIZE_MINIMIZED);
    }
    if msg == WM_PAINT && GetUpdateRect(window, std::ptr::null_mut(), 0) != 0 {
        // A panic can't unwind out of a window procedure; it would abort the process.
        let _ = std::panic::catch_unwind(|| {
            ON_WIPED.with(|f| {
                if let Some(f) = f.borrow().as_ref() {
                    f();
                }
            })
        });
    }
    CallWindowProcW(PREVIOUS.load(Ordering::SeqCst), window, msg, w, l)
}

/// Cuts the window to the frame's outline, gives it the big icon and calls `on_wiped` before each
/// repaint the system asks for. Call on the event loop's thread; false while the native window
/// doesn't exist yet.
pub fn fit(window: &slint::Window, on_wiped: impl Fn() + 'static) -> bool {
    if PREVIOUS.load(Ordering::SeqCst) != 0 {
        return true;
    }
    let Some(handle) = hwnd(window) else {
        return false;
    };
    ON_WIPED.with(|f| *f.borrow_mut() = Some(Box::new(on_wiped)));
    unsafe {
        cut(handle, false);
        let icon = LoadImageW(
            GetModuleHandleW(std::ptr::null()),
            ICON_GROUP as *const u16,
            IMAGE_ICON,
            256,
            256,
            0,
        );
        if !icon.is_null() {
            SendMessageW(handle, WM_SETICON, ICON_BIG, icon as isize);
        }
        let ours: WndProc = proc;
        PREVIOUS.store(
            SetWindowLongPtrW(handle, GWLP_WNDPROC, ours as usize as isize),
            Ordering::SeqCst,
        );
    }
    true
}
