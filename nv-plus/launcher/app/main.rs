#![windows_subsystem = "windows"]

slint::include_modules!();

mod dialogs;
mod front;
mod native;
mod notes;

use front::WindowFront;
use slint::winit_030::winit::dpi::PhysicalSize;
use slint::winit_030::winit::platform::windows::{IconExtWindows, WindowAttributesExtWindows};
use slint::winit_030::winit::window::Icon;
use slint::ComponentHandle;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The package's own folder: where the game and everything it writes live.
fn package_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_default()
}

/// `--game FOLDER` alone still opens the window, so Wine, with no Steam in its registry, can be
/// told where the game is. `NV_RS_NO_WINDOWS=1` or any other argument runs quiet, through the library.
fn quiet_run(args: &[String]) -> bool {
    if std::env::var_os("NV_RS_NO_WINDOWS").is_some_and(|v| v == "1") {
        return true;
    }
    !(args.is_empty() || matches!(args, [flag, _] if flag == "--game"))
}

/// Puts the window back when a start ends, also when it panics, so Play never stays disabled and
/// the window never stays hidden.
struct StartDone {
    window: slint::Weak<LauncherWindow>,
    starting: Arc<AtomicBool>,
}

impl Drop for StartDone {
    fn drop(&mut self) {
        self.starting.store(false, Ordering::SeqCst);
        let _ = self.window.upgrade_in_event_loop(|window| {
            window.set_starting(false);
            window.set_progress(0.0);
            window.set_step("".into());
            window.show().ok();
        });
    }
}

fn main() {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let root = package_root();

    if quiet_run(&args) {
        let front = WindowFront::new(None);
        std::process::exit(nv_plus_launcher::launch(&front, root, &args));
    }

    if nv_plus_launcher::another_is_running() {
        dialogs::error("NV+ is already running.");
        return;
    }

    // The big icon has to be on the window before it is first shown: Wine and Proton build the
    // taskbar's icon from it, and KDE only picks it up when the window appears.
    let icon = Icon::from_resource(native::ICON_GROUP, Some(PhysicalSize::new(256, 256))).ok();
    let backend = slint::BackendSelector::new()
        .with_winit_window_attributes_hook(move |attributes| {
            attributes.with_taskbar_icon(icon.clone())
        })
        .select();
    if let Err(e) = backend {
        dialogs::error(&format!("The launcher's window could not be made: {e}"));
        return;
    }

    let window = match LauncherWindow::new() {
        Ok(window) => window,
        Err(e) => {
            dialogs::error(&format!("The launcher's window could not be made: {e}"));
            return;
        }
    };

    window.set_version(env!("NV_PLUS_VERSION").into());
    let items: Vec<NoteItem> = notes::shipped_notes()
        .into_iter()
        .map(|note| NoteItem {
            title: note.title.into(),
            label: note.label.into(),
            summary: note.summary.into(),
        })
        .collect();
    window.set_notes(slint::ModelRc::new(slint::VecModel::from(items)));

    let starting = Arc::new(AtomicBool::new(false));
    // Quitting mid-start would stop the start between its writes, so it waits for the start to end.
    {
        let starting = starting.clone();
        window.on_quit(move || {
            if !starting.load(Ordering::SeqCst) {
                let _ = slint::quit_event_loop();
            }
        });
    }
    {
        // Alt+F4 and the taskbar's Close only hide the window by default, which would leave the
        // process running unseen and holding the single-instance lock.
        let starting = starting.clone();
        window.window().on_close_requested(move || {
            if starting.load(Ordering::SeqCst) {
                return slint::CloseRequestResponse::KeepWindowShown;
            }
            let _ = slint::quit_event_loop();
            slint::CloseRequestResponse::HideWindow
        });
    }
    {
        let weak = window.as_weak();
        window.on_minimise(move || {
            if let Some(window) = weak.upgrade() {
                window.window().set_minimized(true);
            }
        });
    }
    window.on_website(|| {
        nv_plus_launcher::open_link("https://github.com/HellboundGlory/nv-plus");
    });
    {
        let weak = window.as_weak();
        let game_args = args.clone();
        window.on_play(move || {
            if starting.swap(true, Ordering::SeqCst) {
                return;
            }
            if let Some(window) = weak.upgrade() {
                window.set_starting(true);
                window.set_progress(0.0);
                window.set_step("".into());
            }
            let front = WindowFront::new(Some(weak.clone()));
            let game_root = root.clone();
            let play_args = game_args.clone();
            let done = StartDone {
                window: weak.clone(),
                starting: starting.clone(),
            };
            std::thread::spawn(move || {
                let _done = done;
                nv_plus_launcher::launch(&front, game_root, &play_args);
            });
        });
    }

    if let Err(e) = window.show() {
        dialogs::error(&format!("The launcher's window could not be shown: {e}"));
        return;
    }
    // The native window appears some time after the event loop starts.
    let fitting = std::rc::Rc::new(slint::Timer::default());
    {
        let weak = window.as_weak();
        let timer = std::rc::Rc::downgrade(&fitting);
        fitting.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(20),
            move || {
                let Some(window) = weak.upgrade() else {
                    return;
                };
                let wiped = weak.clone();
                let fitted = native::fit(window.window(), move || {
                    if let Some(window) = wiped.upgrade() {
                        window.set_repaint(!window.get_repaint());
                    }
                });
                if let Some(timer) = timer.upgrade().filter(|_| fitted) {
                    timer.stop();
                }
            },
        );
    }
    let _ = slint::run_event_loop_until_quit();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_window_opens_for_the_game_folder_alone() {
        let cases = [
            (vec![], false),
            (vec!["--game", "C:\\Games\\FalloutNV"], false),
            (vec!["--game", "C:\\Games\\FalloutNV", "--mods"], true),
            (vec!["--check"], true),
            (vec!["--game"], true),
        ];
        for (raw, quiet) in cases {
            assert_eq!(quiet_run(&args(&raw)), quiet, "{raw:?}");
        }
    }
}
