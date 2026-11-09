# NV+ changelog

What NV+ adds or changes on top of [nv-rs](https://github.com/slaterain/nv-rs). Upstream's own
changes come in with each merge and are listed in its pull requests, not here.

## Unreleased

Based on nv-rs `d0d1f375` (2026-11-09).

### Added

- **Mouse smoothing and acceleration for looking around**, both off by default. Settings.cmd has
  a row for each: Off, Light, Medium or Strong. Smoothing spreads each frame's mouse movement over
  a short window (20, 35 or 50 ms), so it feels the same at any frame rate; acceleration turns
  faster hand movement further, rising smoothly to 1.25x, 1.5x or 2x. Menus keep the game's own
  cursor. For fine tuning, `settings.ini` takes `NV_MOUSE_SMOOTHING=<ms>` (0 to 100),
  `NV_MOUSE_ACCEL_LIMIT` (1 to 4) and `NV_MOUSE_ACCEL_RAMP` (above 0, at most 10; 0.1 by default).
  The mouse options can also be set in the game's own `Fallout.ini`, in its `[nv-rs Mouse]` section
  (`sSmoothing`, `sAcceleration`, `fAccelerationLimit`, `fAccelerationRamp`; see
  `distribution/nv-plus-sections.ini`), which wins over `settings.ini` when the game is started
  from `Play.exe`.

### Fixed

- **Settings changed in the game's menus are kept.** The game saves its settings through
  Windows' profile-file call, which nv-rs answered without writing, so `FalloutPrefs.ini` came out
  unchanged and every change was lost at exit. Reported upstream with a fix:
  [nv-rs#370](https://github.com/slaterain/nv-rs/issues/370).
- **Runs under Wine and Proton without closing on the first save.** nv-rs makes the old DOS-style
  short names of the files the game creates with a Windows call that Wine has only as a stub, which
  closes the game. Under Wine NV+ now gives those files no short name, as Windows does on a drive
  with short names turned off. ([#1](https://github.com/HellboundGlory/nv-rs/pull/1))
- **Sound, and the opening, under Wine and Proton.** Wine's DirectSound makes some buffers Windows
  refuses, answers `DS_OK` where Windows says `DS_NO_VIRTUALIZATION`, accepts the reverb Windows 11
  refuses and can't set the distortion's gain, so the game's sound layer, which models Windows,
  refused the table captured there. Without a sound device the game is silent, and a new game
  stalls at Doc Mitchell's. `dsound-capture` now writes those answers as Windows gives them when
  it runs under Wine.
- **Muffled voices no longer stop the game under Wine and Proton.** Wine's `dsdmo.dll` can't run
  the distortion, so the game couldn't measure its edge correction table and stopped at the first
  muffled voice. `nv-plus/dsfx-edge`, a 32-bit tool run under Wine in a `winetricks dsdmo` prefix,
  measures the table from Microsoft's DLL and prints it; the port reads that file when it can't
  measure for itself.
