//! What the launcher shows while it works. The screens the player sees implement this; the logic
//! calls it and never talks to Windows itself.

pub trait Front: Sync {
    fn error(&self, text: &str);
    fn warning(&self, text: &str);
    fn info(&self, text: &str);
    /// Yes or no; with no windows (`NV_RS_NO_WINDOWS=1`), yes, as Play.exe answers.
    fn ask(&self, text: &str) -> bool;
    /// `None` when cancelled, or with no windows.
    fn pick_folder(&self, title: &str) -> Option<std::path::PathBuf>;
    /// One step of the start, shown while the launcher works ("Reading your graphics card...").
    fn step(&self, text: &str);
    /// The steps are over: the game's window is up, or the start stopped. Called again after
    /// that, and possibly with no step before it.
    fn steps_done(&self);
}
