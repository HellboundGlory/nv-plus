//! What the library tells the window: native dialogs on the start's thread, and the loading bar's
//! step and progress through the event loop.

use crate::dialogs;
use slint::ComponentHandle;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::LauncherWindow;

/// The bar's fill after `steps` steps: 0, 0.2, 0.4, 0.6, 0.8, then 0.8 again.
fn progress_after(steps: usize) -> f32 {
    steps.min(4) as f32 / 5.0
}

/// With no window (the runs that start with arguments or `NV_RS_NO_WINDOWS=1`), only the dialogs
/// show, as Play.exe's do.
pub struct WindowFront {
    window: Option<slint::Weak<LauncherWindow>>,
    steps: AtomicUsize,
}

impl WindowFront {
    pub fn new(window: Option<slint::Weak<LauncherWindow>>) -> WindowFront {
        WindowFront {
            window,
            steps: AtomicUsize::new(0),
        }
    }
}

impl nv_plus_launcher::Front for WindowFront {
    fn error(&self, text: &str) {
        dialogs::error(text);
    }

    fn warning(&self, text: &str) {
        dialogs::warning(text);
    }

    fn info(&self, text: &str) {
        dialogs::info(text);
    }

    fn ask(&self, text: &str) -> bool {
        dialogs::ask(text)
    }

    fn pick_folder(&self, title: &str) -> Option<PathBuf> {
        dialogs::pick_folder(title)
    }

    fn step(&self, text: &str) {
        let count = self.steps.fetch_add(1, Ordering::SeqCst) + 1;
        let progress = progress_after(count);
        let Some(window) = &self.window else {
            return;
        };
        let text = text.to_string();
        let _ = window.upgrade_in_event_loop(move |window| {
            window.set_step(text.as_str().into());
            window.set_progress(progress);
        });
    }

    fn steps_done(&self) {
        let Some(window) = &self.window else {
            return;
        };
        let hidden = nv_plus_launcher::game_window_is_there();
        let _ = window.upgrade_in_event_loop(move |window| {
            window.set_progress(1.0);
            if hidden {
                window.hide().ok();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_after_counts_of_steps() {
        for (steps, want) in [(0, 0.0), (1, 0.2), (4, 0.8), (6, 0.8)] {
            assert_eq!(progress_after(steps), want, "{steps} steps");
        }
    }
}
