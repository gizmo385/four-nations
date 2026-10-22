#!/usr/bin/env bash
# Builds and runs fn_client inside the flake devShell (flake.nix), which supplies the Rust toolchain
# and every native library Bevy loads (X11, xkbcommon, Vulkan loader, Mesa drivers). That is what
# makes Nix-built binaries work on Ubuntu/WSL2, which can't use the system's /usr/lib.
#
#   scripts/run-client.sh [seed] [nation] [width] [height]
#   FN_SCREENSHOT=/tmp/shot.png scripts/run-client.sh 42 earth   # headless: no window, save, exit
#
# Prefer `cargo xtask screenshot SEED NATION OUT.png [WIDTH HEIGHT]` for headless screenshots; it
# calls this script. Already in `nix develop`? Then `cargo run -p fn_client` works directly too.
# Without Nix the script just builds and runs with whatever toolchain and system libraries exist.
#
# macOS (Darwin): flake.nix only defines devShells for Linux, and Bevy needs no extra native
# libraries there, so the script skips `nix develop` and builds with the host toolchain. Setting
# FN_DEVSHELL=1 by hand is no longer needed (it still skips Nix on any platform).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
say() { echo "run-client: $*" >&2; }

if [[ -z "${FN_DEVSHELL:-}" && "$(uname -s)" == "Darwin" ]]; then
  say "macOS: the flake has no devShell for this system, using the host toolchain"
elif [[ -z "${FN_DEVSHELL:-}" ]] && command -v nix >/dev/null; then
  say "entering the flake devShell (first run downloads the toolchain and libraries)"
  exec nix develop "$root" -c "$0" "$@"
fi

# Not -q: after pulling changes the rebuild can take a minute, and silence looks like a hang.
say "building fn_client"
cargo build -p fn_client --manifest-path "$root/Cargo.toml"
if [[ -n "${FN_SCREENSHOT:-}" ]]; then
  say "rendering headless screenshot to $FN_SCREENSHOT"
else
  say "starting (the window can take a few seconds to appear with software rendering)"
fi
exec "$root/target/debug/fn_client" "$@"
