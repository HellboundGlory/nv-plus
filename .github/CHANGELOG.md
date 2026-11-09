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
