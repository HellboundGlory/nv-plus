#!/usr/bin/env bash
# Builds the NV+ package and plays it under Wine, for a Linux player with Fallout: New Vegas
# from Steam.
#
#   scripts/linux/play-wine.sh [options] [-- Launcher.exe's own options]
#
#   --game DIR     the game's folder; NV_RS_GAME is used when this is not given, and the folder is
#                  looked for in your Steam libraries otherwise
#   --no-build     play the programs built last time
#   --build-only   build and lay out the package, don't start Wine
#   --dry-run      print the game's folder, the package, the prefix and the wine command line;
#                  build nothing and start nothing
#   -h, --help     this
#
# Needs Rust from rustup with the x86_64-pc-windows-gnu and i686-pc-windows-gnu targets, MinGW-w64
# for both, and Wine. Everything it writes stays in ${XDG_DATA_HOME:-$HOME/.local/share}/nv-plus
# (NV_PLUS_HOME): the package in package/, Cargo's output in target/, Wine's prefix in wine/.

set -euo pipefail

usage() {
    awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"
}

die() {
    printf '%s\n' "$*" >&2
    exit 2
}

game=${NV_RS_GAME:-}
build=1
start=1
dry_run=0
extra=()

while [ $# -gt 0 ]; do
    case $1 in
        --game)
            [ $# -ge 2 ] || die "--game needs the game's folder"
            game=$2
            shift 2
            ;;
        --no-build) build=0; shift ;;
        --build-only) start=0; shift ;;
        --dry-run) dry_run=1; shift ;;
        -h | --help) usage; exit 0 ;;
        --) shift; extra=("$@"); break ;;
        *) die "unknown option: $1 (Launcher.exe's own options go after --)" ;;
    esac
done

repo=$(CDPATH= cd -- "$(dirname -- "$(readlink -f -- "${BASH_SOURCE[0]}")")/../.." && pwd)
# Absolute, because the script changes folder before using them.
NV_PLUS_HOME=$(realpath -m -- "${NV_PLUS_HOME:-${XDG_DATA_HOME:-$HOME/.local/share}/nv-plus}")
package=$NV_PLUS_HOME/package
target=$NV_PLUS_HOME/target
WINEPREFIX=$(realpath -m -- "${WINEPREFIX:-$NV_PLUS_HOME/wine}")
cargo_home=${CARGO_HOME:-$HOME/.cargo}
rustup_home=${RUSTUP_HOME:-$HOME/.rustup}

# The value of each "key" "value" pair in a Steam .vdf or .acf, one per line; with a third argument
# only the first.
vdf_values() {
    awk -v key="$2" -v first="${3:-0}" '
        {
            n = split($0, field, "\"")
            # Older libraryfolders.vdf files list libraries under "1", "2", ...
            if (n >= 4 && (field[2] == key || (key == "path" && field[2] ~ /^[0-9]+$/ && field[4] ~ /^\//))) { print field[4]; if (first == 1) exit }
        }' "$1"
}

libraries=()
declare -A seen_library=()

# A Steam library, once: a root is often a symlink of another.
add_library() {
    local real
    [ -d "$1" ] || return 0
    real=$(realpath -- "$1") || return 0
    [ -z "${seen_library[$real]+x}" ] || return 0
    seen_library[$real]=1
    libraries+=("$real")
}

find_game() {
    local root vdf library id manifest installdir folder
    for root in "$HOME/.steam/steam" \
        "$HOME/.local/share/Steam" \
        "$HOME/.var/app/com.valvesoftware.Steam/.local/share/Steam"; do
        [ -d "$root" ] || continue
        add_library "$root"
        vdf=$root/steamapps/libraryfolders.vdf
        if [ -f "$vdf" ]; then
            while IFS= read -r library; do
                add_library "$library"
            done < <(vdf_values "$vdf" path)
        fi
    done
    for library in "${libraries[@]}"; do
        for id in 22380 22490; do
            manifest=$library/steamapps/appmanifest_$id.acf
            [ -f "$manifest" ] || continue
            installdir=$(vdf_values "$manifest" installdir 1)
            [ -n "$installdir" ] || continue
            folder=$library/steamapps/common/$installdir
            if [ -f "$folder/FalloutNV.exe" ]; then
                printf '%s\n' "$folder"
                return 0
            fi
        done
    done
    return 1
}

if [ "$start" = 1 ] || [ "$dry_run" = 1 ]; then
    if [ -z "$game" ]; then
        game=$(find_game) || game=
    fi
    if [ -z "$game" ]; then
        die "no Fallout: New Vegas found in your Steam libraries. Pass --game with the folder that has FalloutNV.exe in it, or set NV_RS_GAME."
    fi
    [ -f "$game/FalloutNV.exe" ] ||
        die "there is no FalloutNV.exe in $game. Pass --game with the folder that has it."
    game=$(realpath -- "$game")
    # Launcher.exe looks for Steam in Windows' registry, which a new Wine prefix does not have, so
    # the folder goes to it as Wine's own name for it.
    zpath=Z:${game//\//\\}
fi

if [ "$dry_run" = 1 ]; then
    printf 'game: %s\npackage: %s\nprefix: %s\n' "$game" "$package" "$WINEPREFIX"
    printf 'wine: cd %q && wine ./Launcher.exe --game %q' "$package" "$zpath"
    [ "${#extra[@]}" -eq 0 ] || printf ' %q' ${extra[@]+"${extra[@]}"}
    printf '\n'
    exit 0
fi

# rustup answers for the toolchain rust-toolchain.toml pins only from inside the repo.
cd "$repo"

check_tools() {
    local missing=0 cc t targets
    if [ "$build" = 1 ]; then
        if ! command -v cargo >/dev/null; then
            printf 'cargo is not installed: install Rust from https://rustup.rs\n' >&2
            missing=1
        fi
        if ! command -v rustup >/dev/null; then
            printf 'rustup is not installed: install Rust from https://rustup.rs\n' >&2
            missing=1
        else
            targets=$(rustup target list --installed) || targets=
            for t in x86_64-pc-windows-gnu i686-pc-windows-gnu; do
                grep -qx -- "$t" <<<"$targets" ||
                    { printf 'the Rust target %s is not installed: rustup target add %s\n' "$t" "$t" >&2; missing=1; }
            done
        fi
        for cc in x86_64-w64-mingw32-gcc i686-w64-mingw32-gcc; do
            command -v "$cc" >/dev/null ||
                { printf '%s is not installed: install mingw-w64 (Debian and Ubuntu: sudo apt install gcc-mingw-w64-x86-64 gcc-mingw-w64-i686)\n' "$cc" >&2; missing=1; }
        done
    fi
    if [ "$start" = 1 ] && ! command -v wine >/dev/null; then
        printf 'wine is not installed: install Wine from https://winehq.org\n' >&2
        missing=1
    fi
    return "$missing"
}

check_tools || exit 2

x64=$target/x86_64-pc-windows-gnu/release
x86=$target/i686-pc-windows-gnu/release
programs=("$x64/Launcher.exe" "$x64/nv-viewer.exe" "$x64/d3d9-capture.exe" "$x86/nvapi-capture.exe" "$x86/dsound-capture.exe")

if [ "$build" = 1 ]; then
    # The launcher's own licences must match its crates, or the package ships the wrong notices.
    cargo run -q --manifest-path nv-plus/notices/Cargo.toml -- --manifest nv-plus/launcher/Cargo.toml --package nv-plus-launcher --target x86_64-pc-windows-gnu --target x86_64-pc-windows-msvc --out nv-plus/distribution --extra "Barlow-Condensed=nv-plus/launcher/assets/fonts/OFL-Barlow.txt" --check ||
        die "The launcher's licence notices are stale, or nv-plus/notices did not run: see nv-plus/notices/README.md."
    # A static C runtime, so the programs don't need MinGW's DLLs, and the build machine's folders
    # kept out of them. Cargo's output goes to NV_PLUS_HOME's own target/, so a developer's is not
    # rebuilt with these flags.
    sep=$'\x1f'
    export CARGO_TARGET_DIR=$target
    export CARGO_ENCODED_RUSTFLAGS="-C${sep}target-feature=+crt-static${sep}--remap-path-prefix=${repo}=nv-rs${sep}--remap-path-prefix=${cargo_home}/registry/src=cargo${sep}--remap-path-prefix=${rustup_home}=rustup${sep}--remap-path-prefix=${target}=target"
    cargo build --release --locked --target x86_64-pc-windows-gnu -p nv-viewer
    cargo build --release --locked --target x86_64-pc-windows-gnu \
        --manifest-path nv-plus/launcher/Cargo.toml --bin Launcher
    cargo build --release --locked --target x86_64-pc-windows-gnu \
        --manifest-path scripts/d3d9-capture/Cargo.toml
    cargo build --release --locked --target i686-pc-windows-gnu \
        --manifest-path scripts/nvapi-capture/Cargo.toml
    cargo build --release --locked --target i686-pc-windows-gnu \
        --manifest-path scripts/dsound-capture/Cargo.toml
    unset CARGO_ENCODED_RUSTFLAGS CARGO_TARGET_DIR
fi
for program in "${programs[@]}"; do
    [ -f "$program" ] || die "$program is not built yet: run without --no-build once."
done

# The package as the Windows ZIP has it, except userdata (Launcher.exe's profile, tables, saves and
# logs) and the player's mods, which must survive a rebuild.
rm -rf "$package/app" "$package/licenses"
rm -f "$package/Play.exe" "$package/Play with mods.cmd" "$package/Import my saves.cmd"
mkdir -p "$package/app/tools" "$package/mods"
cp "$x64/Launcher.exe" "$package/Launcher.exe"
cp "$x64/nv-viewer.exe" "$package/app/FalloutNV.exe"
cp "$x64/d3d9-capture.exe" "$package/app/tools/d3d9-capture.exe"
cp "$x86/nvapi-capture.exe" "$package/app/tools/nvapi-capture.exe"
cp "$x86/dsound-capture.exe" "$package/app/tools/dsound-capture.exe"
for file in Settings.cmd Settings.ps1 THIRD-PARTY-NOTICES.txt; do
    cp "$repo/distribution/$file" "$package/$file"
done
for file in 'READ ME.txt' LAUNCHER-NOTICES.txt; do
    cp "$repo/nv-plus/distribution/$file" "$package/$file"
done
cp "$repo/nv-plus/distribution/mods/READ ME.txt" "$package/mods/READ ME.txt"
cp -R "$repo/distribution/licenses" "$package/licenses"
cp -R "$repo/nv-plus/distribution/licenses/." "$package/licenses"
cp "$repo/LICENSE" "$repo/LEGAL.md" "$package"
cp "$repo/crates/vorbis/LICENSE-libvorbis" "$repo/crates/nvse/LICENSE-msvc-stl" \
    "$package/licenses"

if [ "$start" = 0 ]; then
    printf 'package: %s\n' "$package"
    exit 0
fi

export WINEPREFIX
export WINEDEBUG=-all
if [ ! -e "$WINEPREFIX/system.reg" ]; then
    printf 'first run: making the Wine prefix %s\n' "$WINEPREFIX"
    wineboot -u
    wineserver -w
fi

cd "$package"
status=0
wine ./Launcher.exe --game "$zpath" ${extra[@]+"${extra[@]}"} || status=$?
printf 'logs: %s\n' "$package/userdata/logs"
exit "$status"
