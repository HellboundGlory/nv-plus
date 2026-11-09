//! NV+ features the upstream crates call through one-line hooks: the mouse feel, the Wine check
//! and the distortion effect's edge correction table read from a file or built in for Wine.

use nv_plus_options::Options;

pub mod edge;
pub mod mouse;
pub mod wine;

/// Binds the look feel and says what is on, one line on stderr.
pub fn install(options: &Options) {
    mouse::set(options.mouse);
    if let Some(rest) = options.describe().strip_prefix("; ") {
        eprintln!("NV+: {rest}");
    }
    if let Some(note) = &options.note {
        eprintln!("{note}");
    }
}
