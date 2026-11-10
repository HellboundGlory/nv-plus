//! The NV+ launcher's logic: Play.exe's work without any window of its own. The screens supply a
//! [`Front`] and call [`launch`].
mod desktop;
mod find;
mod front;
mod game_ini;
mod mods;
mod play;
mod settings;
mod tables;
mod win;

pub use front::Front;
pub use play::launch;
