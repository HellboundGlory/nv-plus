# NV+ changelog

What NV+ adds or changes on top of [nv-rs](https://github.com/slaterain/nv-rs). Upstream's own
changes come in with each merge and are listed in its pull requests, not here.

## Unreleased

Based on nv-rs `d0d1f375` (2026-11-09).

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
  stalls at Doc Mitchell's. Play.exe writes the captured answers as Windows gives them when it
  runs under Wine or Proton.
- **Muffled voices no longer stop the game under Wine and Proton.** Wine's `dsdmo.dll` can't run
  the distortion, so the game couldn't measure its edge correction table and stopped at the first
  muffled voice. `nv-plus/dsfx-edge`, a 32-bit tool run under Wine in a `winetricks dsdmo` prefix,
  measures the table from Microsoft's DLL and prints it; the port reads that file when it can't
  measure for itself. Play.exe needs no setup now: under Wine and Proton the game uses a built-in
  copy of the table measured from Microsoft's 2002 DLL when it can't measure one and no
  `NV_DSFX_EDGE` file is set.
