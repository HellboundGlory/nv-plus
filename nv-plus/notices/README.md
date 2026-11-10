# nv-plus-notices

Writes the third-party notices and licence files for the NV+ launcher, in the
format of upstream's `distribution/THIRD-PARTY-NOTICES.txt`.

It reads `cargo metadata` for the launcher's own `Cargo.toml` and lock, walks
the dependency graph of `nv-plus-launcher` across normal and build edges (dev
edges and path packages left out), and for each crate copies the licence files
from that exact version in the local Cargo registry into
`licenses/<package>-<version>/`. A crate that ships no licence file gets the
standard text of its licence expression, marked in its entry.

```
cargo run -q --manifest-path nv-plus/notices/Cargo.toml -- \
    --manifest nv-plus/launcher/Cargo.toml \
    --package nv-plus-launcher \
    --target x86_64-pc-windows-gnu --target x86_64-pc-windows-msvc \
    --out nv-plus/distribution \
    --extra "Barlow-Condensed=nv-plus/launcher/assets/fonts/OFL-Barlow.txt"
```

Run it again whenever `nv-plus/launcher/Cargo.lock` changes. The packaging
scripts pass `--check` on the same arguments, which writes nothing and fails
when the committed files under `nv-plus/distribution/` are not what this run
would write. A write replaces `licenses/` whole, so a crate the lock dropped
goes with it; the check ignores line ends, which git may change on checkout.
